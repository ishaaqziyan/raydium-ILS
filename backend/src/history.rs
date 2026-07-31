//! Helius RPC historical pull + reserve reconstruction.
//!
//! Approach: for each target timestamp, binary-search (via slot-time
//! estimation + `getBlockTime` correction) the Solana slot nearest that
//! moment. Pull that block in `transactionDetails: "signatures"` mode
//! (tiny payload — just a signature list) purely to get *any* signature
//! anchored at that point in time, then hand that signature to
//! `getSignaturesForAddress(vault, { before: seed })` — Helius's own
//! per-account index — to jump straight to the nearest real transaction
//! that actually touches the vault. `getTransaction` on that single
//! signature gives `meta.preTokenBalances`/`postTokenBalances`; the *change*
//! in each vault's balance during that one transaction is that swap's
//! executed price, which works as a real point-in-time price sample for
//! both AMM v4 and CLMM pools without decoding either program's swap
//! instruction data — see `extract_snapshot_from_tx` for why the delta
//! (not the raw balance ratio) is required for CLMM correctness.
//!
//! Two earlier approaches were tried and rejected:
//! - `getSignaturesForAddress` walked back from *now* (the doc's original
//!   plan): doesn't scale for a pool this busy — SOL/USDC's vault sees on
//!   the order of 10+ tx/sec, so paginating signatures only reached ~12
//!   hours of history in 500 pages.
//! - `getBlock` with full transaction scanning, checking every tx in the
//!   block for one that references the vault: technically correct but slow
//!   in practice — a Solana block can have 2000+ transactions and only a
//!   handful (sometimes zero) actually touch any one pool's vault, so most
//!   of each multi-MB block download was wasted.
//!
//! Seeding the address-indexed lookup near the right time (rather than
//! either extreme) keeps the RPC call count per sample small and each
//! payload tiny — a signature list, then one single-transaction fetch.

use futures::stream::{self, StreamExt};
use serde::Deserialize;
use serde_json::json;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Semaphore;

use crate::pools::PoolConfig;

/// Global cap on in-flight Helius requests, regardless of how much
/// parallelism any call site above `rpc_call` uses — the one choke point
/// that lets bucket- and candidate-level concurrency scale freely without
/// blowing past Helius's rate limit.
static RPC_PERMITS: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(20));

const RPC_MAX_RETRIES: u32 = 4;
const RPC_RETRY_BASE_DELAY: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy)]
pub struct ReserveSnapshot {
    pub timestamp: i64,
    /// quote per base, e.g. USDC per SOL.
    pub price: f64,
}

#[derive(Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

/// `transactionDetails: "signatures"` — the lightest possible block payload,
/// used only to anchor a `getSignaturesForAddress` lookup at a point in time.
#[derive(Deserialize)]
struct BlockSignatures {
    #[serde(default)]
    signatures: Vec<String>,
}

#[derive(Deserialize)]
struct SignatureInfo {
    signature: String,
}

#[derive(Deserialize)]
struct TransactionResult {
    #[serde(rename = "blockTime")]
    block_time: Option<i64>,
    transaction: TransactionEnvelope,
    meta: Option<TxMeta>,
}

#[derive(Deserialize)]
struct TransactionEnvelope {
    message: TxMessage,
}

#[derive(Deserialize)]
struct TxMessage {
    #[serde(rename = "accountKeys")]
    account_keys: Vec<AccountKeyEntry>,
}

#[derive(Deserialize)]
struct AccountKeyEntry {
    pubkey: String,
}

#[derive(Deserialize)]
struct TxMeta {
    #[serde(rename = "preTokenBalances", default)]
    pre_token_balances: Vec<TokenBalance>,
    #[serde(rename = "postTokenBalances", default)]
    post_token_balances: Vec<TokenBalance>,
    #[serde(rename = "loadedAddresses")]
    loaded_addresses: Option<LoadedAddresses>,
}

#[derive(Deserialize)]
struct LoadedAddresses {
    #[serde(default)]
    writable: Vec<String>,
    #[serde(default)]
    readonly: Vec<String>,
}

#[derive(Deserialize)]
struct TokenBalance {
    #[serde(rename = "accountIndex")]
    account_index: u32,
    #[serde(rename = "uiTokenAmount")]
    ui_token_amount: UiTokenAmount,
}

#[derive(Deserialize)]
struct UiTokenAmount {
    #[serde(rename = "uiAmount")]
    ui_amount: Option<f64>,
}

/// Send `body` (a single request object or a batch array), gated by the
/// global permit pool, retrying with backoff on 429/5xx. Callers parse the
/// body themselves since single vs. batch responses deserialize differently.
async fn send_with_retry(
    client: &reqwest::Client,
    rpc_url: &str,
    body: &serde_json::Value,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    for attempt in 0..=RPC_MAX_RETRIES {
        // Acquire is scoped inside the loop so a backing-off retry doesn't
        // hold a permit (and starve other callers) while it sleeps.
        let _permit = RPC_PERMITS.acquire().await?;
        let resp = client.post(rpc_url).json(body).send().await?;
        let status = resp.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error() {
            drop(_permit);
            if attempt == RPC_MAX_RETRIES {
                return Err(format!("RPC HTTP {status} after {RPC_MAX_RETRIES} retries").into());
            }
            tokio::time::sleep(RPC_RETRY_BASE_DELAY * 2u32.pow(attempt)).await;
            continue;
        }

        return Ok(resp);
    }

    unreachable!("loop always returns via the Ok/Err arms above")
}

async fn rpc_call<T: for<'de> Deserialize<'de>>(
    client: &reqwest::Client,
    rpc_url: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<T, Box<dyn std::error::Error>> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": params,
    });
    let resp = send_with_retry(client, rpc_url, &body).await?;
    let parsed: RpcResponse<T> = resp.json().await?;
    if let Some(err) = parsed.error {
        return Err(format!("RPC error {}: {}", err.code, err.message).into());
    }
    parsed.result.ok_or_else(|| "RPC response missing result field".into())
}

/// Pre- and post-transaction balance for a vault, matched by comparing its
/// address against the transaction's full account key list (static keys
/// plus any address-lookup-table-loaded keys).
fn vault_balance_delta(meta: &TxMeta, all_keys: &[String], vault_address: &str) -> Option<(f64, f64)> {
    let idx = all_keys.iter().position(|k| k == vault_address)? as u32;
    let pre = meta.pre_token_balances.iter().find(|b| b.account_index == idx)?.ui_token_amount.ui_amount?;
    let post = meta.post_token_balances.iter().find(|b| b.account_index == idx)?.ui_token_amount.ui_amount?;
    Some((pre, post))
}

async fn try_get_block_time(client: &reqwest::Client, rpc_url: &str, slot: u64) -> Option<i64> {
    rpc_call::<Option<i64>>(client, rpc_url, "getBlockTime", json!([slot])).await.ok().flatten()
}

async fn try_get_block_signatures(
    client: &reqwest::Client,
    rpc_url: &str,
    slot: u64,
) -> Option<Vec<String>> {
    let params = json!([
        slot,
        {
            "encoding": "json",
            "maxSupportedTransactionVersion": 0,
            "transactionDetails": "signatures",
            "rewards": false
        }
    ]);
    let block = rpc_call::<Option<BlockSignatures>>(client, rpc_url, "getBlock", params).await.ok()??;
    if block.signatures.is_empty() {
        None
    } else {
        Some(block.signatures)
    }
}

async fn try_get_transaction(
    client: &reqwest::Client,
    rpc_url: &str,
    signature: &str,
) -> Option<TransactionResult> {
    let params = json!([
        signature,
        { "encoding": "jsonParsed", "maxSupportedTransactionVersion": 0 }
    ]);
    rpc_call::<Option<TransactionResult>>(client, rpc_url, "getTransaction", params).await.ok().flatten()
}

/// Average Solana slot duration, used only to seed the initial slot guess;
/// corrected below via a few `getBlockTime` probes.
const AVG_SLOT_SECONDS: f64 = 0.45;

/// Estimate the slot nearest `target_time`, refining an initial linear guess
/// with a handful of `getBlockTime` round trips (skipped slots are probed
/// forward a few positions since they have no block time of their own).
async fn find_slot_for_timestamp(
    client: &reqwest::Client,
    rpc_url: &str,
    target_time: i64,
    current_slot: u64,
    current_time: i64,
) -> u64 {
    let mut estimate =
        current_slot as i64 - ((current_time - target_time) as f64 / AVG_SLOT_SECONDS) as i64;
    estimate = estimate.max(1);

    for _ in 0..8 {
        let mut probed = None;
        for offset in 0..5i64 {
            let slot = (estimate + offset).max(1) as u64;
            if let Some(t) = try_get_block_time(client, rpc_url, slot).await {
                probed = Some((slot, t));
                break;
            }
        }
        let Some((slot, actual_time)) = probed else { break };
        let diff = actual_time - target_time;
        if diff.abs() < 30 {
            return slot;
        }
        estimate -= (diff as f64 / AVG_SLOT_SECONDS) as i64;
        estimate = estimate.max(1);
    }

    estimate.max(1) as u64
}

/// Price for this snapshot is the *executed* price of the swap found at
/// this point in time — the ratio of how much each vault's balance actually
/// moved in this transaction — not the ratio of the vaults' absolute
/// balances.
///
/// Those are the same thing for a constant-product (AMM v4) pool, since its
/// entire curve lives in those two numbers. They are *not* the same thing
/// for a CLMM pool: its vaults hold liquidity spread across many tick
/// ranges, most of it out of range at any given price, so the raw balance
/// ratio has no clean relationship to spot price. The executed-delta
/// approach sidesteps that entirely — and works identically for both curve
/// types — without needing to decode the CLMM pool's `sqrtPriceX64` state
/// (which, being current-only, can't be read historically through plain
/// RPC anyway).
fn extract_snapshot_from_tx(tx: &TransactionResult, pool: &PoolConfig) -> Option<ReserveSnapshot> {
    let block_time = tx.block_time?;
    let meta = tx.meta.as_ref()?;

    let mut all_keys: Vec<String> =
        tx.transaction.message.account_keys.iter().map(|k| k.pubkey.clone()).collect();
    if let Some(loaded) = &meta.loaded_addresses {
        all_keys.extend(loaded.writable.iter().cloned());
        all_keys.extend(loaded.readonly.iter().cloned());
    }

    let (base_pre, base_post) = vault_balance_delta(meta, &all_keys, pool.base_vault)?;
    let (quote_pre, quote_post) = vault_balance_delta(meta, &all_keys, pool.quote_vault)?;

    let base_delta = (base_post - base_pre).abs();
    let quote_delta = (quote_post - quote_pre).abs();
    if base_delta <= 0.0 || quote_delta <= 0.0 {
        // Vault was referenced (e.g. a routing hop) but didn't actually
        // trade against this pool in this transaction.
        return None;
    }

    Some(ReserveSnapshot { timestamp: block_time, price: quote_delta / base_delta })
}

/// Find a signature to anchor a `before`-scoped `getSignaturesForAddress`
/// call, by locating any signature in a block near `start_slot` (probing a
/// handful of nearby slots in case of skips — a signatures-only block fetch
/// is cheap, so this is a light preamble to the real lookup below).
async fn find_seed_signature(client: &reqwest::Client, rpc_url: &str, start_slot: u64) -> Option<String> {
    const MAX_OFFSET: i64 = 20;
    let candidates = std::iter::once(0i64).chain((1..=MAX_OFFSET).flat_map(|o| [o, -o]));
    for offset in candidates {
        let slot = start_slot as i64 + offset;
        if slot < 1 {
            continue;
        }
        if let Some(sigs) = try_get_block_signatures(client, rpc_url, slot as u64).await {
            if let Some(sig) = sigs.into_iter().next() {
                return Some(sig);
            }
        }
    }
    None
}

/// Resolve a reserve snapshot near `start_slot`: anchor a seed signature at
/// that slot, then walk `getSignaturesForAddress` on the vault (Helius's own
/// account index) backward from there, and take the *median* executed
/// price across every candidate that actually carries both vaults'
/// balances (some referencing transactions — e.g. aggregator routes —
/// mention the vault without changing its balance, and are skipped).
///
/// A single-transaction price is noisy: one thin, high-slippage swap can
/// print a wildly off-market execution price (an early version of this
/// pipeline hit exactly this — one outlier trade briefly implied a ~40%
/// price spike that doesn't appear anywhere else in the surrounding
/// history, and fed straight into an obviously-wrong IL spike). The median
/// across several nearby candidates is a standard, minimal fix for
/// single-point contamination like that, without needing any actual
/// outlier-detection logic.
async fn find_reserve_near_slot(
    client: &reqwest::Client,
    rpc_url: &str,
    pool: &PoolConfig,
    start_slot: u64,
) -> Option<ReserveSnapshot> {
    const CANDIDATE_LIMIT: u32 = 10;

    let seed = find_seed_signature(client, rpc_url, start_slot).await?;

    let params = json!([pool.base_vault, { "before": seed, "limit": CANDIDATE_LIMIT }]);
    let candidates: Vec<SignatureInfo> =
        rpc_call(client, rpc_url, "getSignaturesForAddress", params).await.ok()?;

    let mut snapshots: Vec<ReserveSnapshot> = stream::iter(candidates)
        .map(|candidate| async move { try_get_transaction(client, rpc_url, &candidate.signature).await })
        .buffer_unordered(CANDIDATE_LIMIT as usize)
        .filter_map(|tx| async move { tx.and_then(|tx| extract_snapshot_from_tx(&tx, pool)) })
        .collect()
        .await;

    if snapshots.is_empty() {
        return None;
    }
    snapshots.sort_by(|a, b| a.price.total_cmp(&b.price));
    Some(snapshots[snapshots.len() / 2])
}

/// Pull a sampled reserve/price history for `pool` over the last
/// `lookback_days`, one sample every `interval_hours`.
pub async fn fetch_reserve_history(
    client: &reqwest::Client,
    rpc_url: &str,
    pool: &PoolConfig,
    lookback_days: i64,
    interval_hours: i64,
) -> Result<Vec<ReserveSnapshot>, Box<dyn std::error::Error>> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let current_slot: u64 = rpc_call(client, rpc_url, "getSlot", json!([])).await?;
    let current_time = try_get_block_time(client, rpc_url, current_slot).await.unwrap_or(now);

    let interval_seconds = interval_hours * 3_600;
    let lookback_seconds = lookback_days * 86_400;

    let mut targets = Vec::new();
    let mut t = now;
    while t >= now - lookback_seconds {
        targets.push(t);
        t -= interval_seconds;
    }
    targets.sort_unstable();

    // Each bucket is an independent lookup — fan out with bounded
    // concurrency rather than walking targets one at a time, since wall
    // time here is network round-trip latency, not CPU work. Safe to raise
    // above the global RPC_PERMITS cap — that semaphore, not this constant,
    // is what actually bounds in-flight Helius requests.
    const CONCURRENCY: usize = 16;

    let mut snapshots: Vec<ReserveSnapshot> = stream::iter(targets)
        .map(|target_time| async move {
            let slot =
                find_slot_for_timestamp(client, rpc_url, target_time, current_slot, current_time)
                    .await;
            find_reserve_near_slot(client, rpc_url, pool, slot).await
        })
        .buffer_unordered(CONCURRENCY)
        .filter_map(|snap| async move { snap })
        .collect()
        .await;

    snapshots.sort_by_key(|s| s.timestamp);
    snapshots.dedup_by_key(|s| s.timestamp);
    Ok(snapshots)
}
