mod history;
mod il_calc;
mod pools;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tower_http::cors::CorsLayer;

const DEFAULT_DEPOSIT_USD: f64 = 10_000.0;

/// (pool id, days, interval_hours) — deposit isn't part of the key since it
/// only affects the local IL math below, not the on-chain history fetch.
/// Keeping history cached separately from deposit means changing the
/// deposit amount recomputes instantly instead of re-hitting Helius.
type HistoryCacheKey = (String, i64, i64);

/// Deliberately takes no deposit argument — that's the entire point of the
/// split. A cache key built from just (pool, days, interval) means any
/// deposit value looked up right after reuses the same cached history.
fn history_cache_key(pool_id: &str, days: i64, interval_hours: i64) -> HistoryCacheKey {
    (pool_id.to_string(), days, interval_hours)
}

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    rpc_url: String,
    // No persistent DB — doc section 5 — history cached in memory per
    // pool/range for repeated requests during a session.
    history_cache: Arc<Mutex<HashMap<HistoryCacheKey, Arc<Vec<history::ReserveSnapshot>>>>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IlSeriesResponse {
    pool_id: &'static str,
    pool_label: &'static str,
    entry_price: f64,
    deposit_usd: f64,
    points: Vec<il_calc::IlPoint>,
}

#[derive(Debug, Deserialize)]
struct IlQuery {
    pool: String,
    days: i64,
    interval_hours: Option<i64>,
    deposit_usd: Option<f64>,
}

struct ApiError(StatusCode, String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}

async fn get_il_series(
    State(state): State<AppState>,
    Query(params): Query<IlQuery>,
) -> Result<Json<IlSeriesResponse>, ApiError> {
    let pool = pools::find_pool(&params.pool)
        .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, format!("unknown pool '{}'", params.pool)))?;

    if params.days != 7 && params.days != 30 && params.days != 90 {
        return Err(ApiError(StatusCode::BAD_REQUEST, "days must be 7, 30, or 90".into()));
    }
    let interval_hours = params.interval_hours.unwrap_or(match params.days {
        7 => 3,
        30 => 6,
        _ => 12,
    });
    let deposit_usd = params.deposit_usd.unwrap_or(DEFAULT_DEPOSIT_USD);

    let history_key = history_cache_key(pool.id, params.days, interval_hours);
    let cached_history = state.history_cache.lock().unwrap().get(&history_key).cloned();
    let reserve_history = match cached_history {
        Some(history) => history,
        None => {
            let fetched = history::fetch_reserve_history(
                &state.client,
                &state.rpc_url,
                pool,
                params.days,
                interval_hours,
            )
            .await
            .map_err(|e| ApiError(StatusCode::BAD_GATEWAY, format!("history fetch failed: {e}")))?;
            let fetched = Arc::new(fetched);
            state.history_cache.lock().unwrap().insert(history_key, fetched.clone());
            fetched
        }
    };

    let Some(first) = reserve_history.first() else {
        return Err(ApiError(StatusCode::BAD_GATEWAY, "no reserve history found for this range".into()));
    };
    let entry_price = first.price;

    let price_snapshots: Vec<il_calc::PriceSnapshot> = reserve_history
        .iter()
        .map(|s| il_calc::PriceSnapshot { timestamp: s.timestamp, price: s.price })
        .collect();

    let curve = match pool.curve {
        pools::CurveKind::AmmV4 => il_calc::Curve::ConstantProduct,
        pools::CurveKind::Clmm { price_lower, price_upper } => {
            // Position::new asserts entry_price is in-range and panics
            // otherwise — check first so a price that's drifted outside
            // this CLMM pool's fixed range (decoded at a fixed point in
            // time) returns a clear error instead of a crashed request.
            if entry_price < price_lower || entry_price > price_upper {
                return Err(ApiError(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!(
                        "entry price {entry_price:.4} at the start of this range falls outside \
                         {}'s fixed CLMM tick range [{price_lower:.4}, {price_upper:.4}] — try a \
                         shorter range",
                        pool.label
                    ),
                ));
            }
            il_calc::Curve::ClmmInRange { price_lower, price_upper }
        }
    };
    let position = il_calc::Position::new(curve, entry_price, deposit_usd);
    let points = position.compute_series(&price_snapshots);

    let response = IlSeriesResponse {
        pool_id: pool.id,
        pool_label: pool.label,
        entry_price,
        deposit_usd,
        points,
    };

    Ok(Json(response))
}

#[tokio::main]
async fn main() {
    let rpc_key = std::env::var("HELIUS_API_KEY").expect("HELIUS_API_KEY not set");
    let rpc_url = format!("https://mainnet.helius-rpc.com/?api-key={rpc_key}");

    let state = AppState {
        client: reqwest::Client::new(),
        rpc_url,
        history_cache: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/api/il-series", get(get_il_series))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod cache_split_tests {
    use super::*;
    use history::ReserveSnapshot;

    fn snapshot(price: f64) -> ReserveSnapshot {
        ReserveSnapshot { timestamp: 0, price }
    }

    // The cache key function takes no deposit argument at all — this is
    // the actual enforcement of the split, checked at compile time. These
    // tests cover the *behavioral* half: that the resulting keys collide
    // or differ the way callers rely on.

    #[test]
    fn same_pool_days_interval_produce_identical_key_regardless_of_call_site() {
        let a = history_cache_key("58oQ...pool", 7, 3);
        let b = history_cache_key("58oQ...pool", 7, 3);
        assert_eq!(a, b);
    }

    #[test]
    fn different_days_produce_different_keys() {
        let a = history_cache_key("pool", 7, 3);
        let b = history_cache_key("pool", 30, 3);
        assert_ne!(a, b);
    }

    #[test]
    fn different_interval_produces_different_keys() {
        let a = history_cache_key("pool", 7, 3);
        let b = history_cache_key("pool", 7, 6);
        assert_ne!(a, b);
    }

    #[test]
    fn different_pool_produces_different_keys() {
        let a = history_cache_key("pool-a", 7, 3);
        let b = history_cache_key("pool-b", 7, 3);
        assert_ne!(a, b);
    }

    #[test]
    fn cache_hit_on_second_lookup_with_same_key_returns_same_history_without_refetching() {
        let cache: Arc<Mutex<HashMap<HistoryCacheKey, Arc<Vec<ReserveSnapshot>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let key = history_cache_key("pool", 7, 3);
        let history = Arc::new(vec![snapshot(100.0), snapshot(110.0)]);

        cache.lock().unwrap().insert(key.clone(), history.clone());

        // Two independent "requests" with different deposit amounts both
        // look up the same history_key — deposit was never part of it —
        // and should both hit the cache and observe the identical Arc
        // (no clone of the underlying Vec, no refetch).
        let first_lookup = cache.lock().unwrap().get(&key).cloned();
        let second_lookup = cache.lock().unwrap().get(&key).cloned();

        let first_lookup = first_lookup.expect("expected cache hit");
        let second_lookup = second_lookup.expect("expected cache hit");
        assert!(Arc::ptr_eq(&first_lookup, &second_lookup));
        assert!(Arc::ptr_eq(&first_lookup, &history));
    }

    #[test]
    fn cache_miss_when_key_absent() {
        let cache: Arc<Mutex<HashMap<HistoryCacheKey, Arc<Vec<ReserveSnapshot>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let key = history_cache_key("pool", 7, 3);

        assert!(cache.lock().unwrap().get(&key).is_none());
    }
}
