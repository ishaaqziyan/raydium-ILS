//! Core impermanent loss math. Price convention throughout: `price` is the
//! value of one unit of token0 expressed in token1 (quote) terms, e.g. USDC
//! per SOL for a SOL/USDC pool. All dollar-style values below are in token1
//! units.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    /// Uniswap/Raydium-AMM-v4 style x*y=k pool.
    ConstantProduct,
    /// Raydium CLMM position, restricted to a single fixed tick range
    /// (price_lower, price_upper), per MVP scope.
    ClmmInRange { price_lower: f64, price_upper: f64 },
}

#[derive(Debug, Clone, Copy)]
pub struct PriceSnapshot {
    pub timestamp: i64,
    pub price: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IlPoint {
    pub timestamp: i64,
    pub hold_value: f64,
    pub lp_value: f64,
    pub il_percent: f64,
}

#[derive(Debug, Clone, Copy)]
enum CurveState {
    ConstantProduct { k: f64 },
    Clmm { liquidity: f64, sqrt_pa: f64, sqrt_pb: f64 },
}

pub struct Position {
    amount0_entry: f64,
    amount1_entry: f64,
    state: CurveState,
}

impl Position {
    /// `entry_price` is price at deposit time, `initial_value` is total
    /// deposit value in token1 units, split according to the curve's
    /// balanced-deposit rule at entry.
    ///
    /// For ClmmInRange, entry_price must fall within [price_lower,
    /// price_upper] — the MVP only simulates positions opened in-range.
    pub fn new(curve: Curve, entry_price: f64, initial_value: f64) -> Self {
        assert!(entry_price > 0.0, "entry_price must be positive");
        assert!(initial_value > 0.0, "initial_value must be positive");

        match curve {
            Curve::ConstantProduct => {
                // Balanced deposit: half the value on each side.
                let amount0_entry = initial_value / (2.0 * entry_price);
                let amount1_entry = initial_value / 2.0;
                let k = amount0_entry * amount1_entry;
                Self { amount0_entry, amount1_entry, state: CurveState::ConstantProduct { k } }
            }
            Curve::ClmmInRange { price_lower, price_upper } => {
                assert!(price_lower > 0.0 && price_upper > price_lower, "invalid tick range");
                assert!(
                    entry_price >= price_lower && entry_price <= price_upper,
                    "entry_price must fall within the tick range"
                );

                let sqrt_pa = price_lower.sqrt();
                let sqrt_pb = price_upper.sqrt();
                let sqrt_p0 = entry_price.sqrt();

                // Derived from: value(p0) = L * (2*sqrtP0 - sqrtPa - p0/sqrtPb)
                let denom = 2.0 * sqrt_p0 - sqrt_pa - entry_price / sqrt_pb;
                let liquidity = initial_value / denom;

                let amount0_entry = liquidity * (1.0 / sqrt_p0 - 1.0 / sqrt_pb);
                let amount1_entry = liquidity * (sqrt_p0 - sqrt_pa);

                Self {
                    amount0_entry,
                    amount1_entry,
                    state: CurveState::Clmm { liquidity, sqrt_pa, sqrt_pb },
                }
            }
        }
    }

    fn hold_value_at(&self, price: f64) -> f64 {
        self.amount0_entry * price + self.amount1_entry
    }

    fn lp_value_at(&self, price: f64) -> f64 {
        match self.state {
            CurveState::ConstantProduct { k } => {
                // x = sqrt(k/p), y = sqrt(k*p) => value = x*p + y = 2*sqrt(k*p)
                2.0 * (k * price).sqrt()
            }
            CurveState::Clmm { liquidity, sqrt_pa, sqrt_pb } => {
                let sqrt_p = price.sqrt();
                let (amount0, amount1) = if sqrt_p <= sqrt_pa {
                    // Price below range: fully in token0.
                    (liquidity * (1.0 / sqrt_pa - 1.0 / sqrt_pb), 0.0)
                } else if sqrt_p >= sqrt_pb {
                    // Price above range: fully in token1.
                    (0.0, liquidity * (sqrt_pb - sqrt_pa))
                } else {
                    (
                        liquidity * (1.0 / sqrt_p - 1.0 / sqrt_pb),
                        liquidity * (sqrt_p - sqrt_pa),
                    )
                };
                amount0 * price + amount1
            }
        }
    }

    /// Compute the {hold, lp, ilPercent} series over a price history.
    /// Does not require the first snapshot to equal entry_price/timestamp.
    pub fn compute_series(&self, snapshots: &[PriceSnapshot]) -> Vec<IlPoint> {
        snapshots
            .iter()
            .map(|s| {
                let hold_value = self.hold_value_at(s.price);
                let lp_value = self.lp_value_at(s.price);
                let il_percent = (lp_value - hold_value) / hold_value * 100.0;
                IlPoint { timestamp: s.timestamp, hold_value, lp_value, il_percent }
            })
            .collect()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn constant_product_no_price_move_has_zero_il() {
        let pos = Position::new(Curve::ConstantProduct, 100.0, 1000.0);
        let series = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 100.0 }]);
        assert!(approx(series[0].il_percent, 0.0, 1e-9));
        assert!(approx(series[0].hold_value, 1000.0, 1e-9));
        assert!(approx(series[0].lp_value, 1000.0, 1e-9));
    }

    #[test]
    fn constant_product_2x_price_move_matches_textbook_il() {
        // Textbook: IL(r) = 2*sqrt(r)/(1+r) - 1, r = price ratio.
        // r=2 => IL ≈ -5.719%
        let pos = Position::new(Curve::ConstantProduct, 100.0, 1000.0);
        let series = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 200.0 }]);
        assert!(approx(series[0].il_percent, -5.719, 1e-2));
    }

    #[test]
    fn constant_product_4x_price_move_is_20_percent_il() {
        // Textbook fact: a 4x price move produces a 20% IL, exactly.
        let pos = Position::new(Curve::ConstantProduct, 100.0, 1000.0);
        let series = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 400.0 }]);
        assert!(approx(series[0].il_percent, -20.0, 1e-6));
    }

    #[test]
    fn constant_product_il_is_symmetric_in_log_price() {
        // r and 1/r produce identical IL magnitude.
        let pos = Position::new(Curve::ConstantProduct, 100.0, 1000.0);
        let up = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 400.0 }]);
        let down = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 25.0 }]);
        assert!(approx(up[0].il_percent, down[0].il_percent, 1e-9));
    }

    #[test]
    fn constant_product_lp_value_never_exceeds_hold_value() {
        let pos = Position::new(Curve::ConstantProduct, 50.0, 1000.0);
        let prices = [1.0, 10.0, 40.0, 50.0, 60.0, 200.0, 1000.0, 0.5];
        let snapshots: Vec<_> = prices
            .iter()
            .enumerate()
            .map(|(i, &p)| PriceSnapshot { timestamp: i as i64, price: p })
            .collect();
        for point in pos.compute_series(&snapshots) {
            assert!(point.lp_value <= point.hold_value + 1e-6, "IL must never be positive");
            assert!(point.il_percent <= 1e-6);
        }
    }

    #[test]
    fn clmm_in_range_no_price_move_has_zero_il() {
        let curve = Curve::ClmmInRange { price_lower: 80.0, price_upper: 125.0 };
        let pos = Position::new(curve, 100.0, 1000.0);
        let series = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 100.0 }]);
        assert!(approx(series[0].il_percent, 0.0, 1e-6));
        assert!(approx(series[0].hold_value, 1000.0, 1e-6));
        assert!(approx(series[0].lp_value, 1000.0, 1e-6));
    }

    #[test]
    fn clmm_in_range_amplifies_il_versus_constant_product() {
        // Same entry conditions; a tight range should show *more* IL than
        // full-range (constant product) for the same price move — this is
        // the core teaching point of concentrated liquidity.
        let cp_pos = Position::new(Curve::ConstantProduct, 100.0, 1000.0);
        let clmm_pos = Position::new(
            Curve::ClmmInRange { price_lower: 90.0, price_upper: 111.0 },
            100.0,
            1000.0,
        );
        let snap = [PriceSnapshot { timestamp: 0, price: 110.0 }];
        let cp_il = cp_pos.compute_series(&snap)[0].il_percent;
        let clmm_il = clmm_pos.compute_series(&snap)[0].il_percent;
        assert!(clmm_il < cp_il, "tight-range CLMM IL ({clmm_il}) should exceed CP IL ({cp_il})");
    }

    #[test]
    fn clmm_price_above_range_holds_only_token1() {
        let curve = Curve::ClmmInRange { price_lower: 80.0, price_upper: 120.0 };
        let pos = Position::new(curve, 100.0, 1000.0);
        let series = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 500.0 }]);
        // Fully converted to token1: lp_value should equal amount1 alone,
        // and stop tracking further price increases.
        let series2 = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 5000.0 }]);
        assert!(approx(series[0].lp_value, series2[0].lp_value, 1e-6));
    }

    #[test]
    fn clmm_price_below_range_holds_only_token0() {
        let curve = Curve::ClmmInRange { price_lower: 80.0, price_upper: 120.0 };
        let pos = Position::new(curve, 100.0, 1000.0);
        let series = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 10.0 }]);
        let series2 = pos.compute_series(&[PriceSnapshot { timestamp: 0, price: 1.0 }]);
        // lp_value scales linearly with price when fully in token0.
        assert!(approx(series[0].lp_value / 10.0, series2[0].lp_value / 1.0, 1e-6));
    }

    #[test]
    #[should_panic(expected = "entry_price must fall within the tick range")]
    fn clmm_rejects_out_of_range_entry_price() {
        let curve = Curve::ClmmInRange { price_lower: 80.0, price_upper: 120.0 };
        Position::new(curve, 200.0, 1000.0);
    }

    #[test]
    fn full_series_tracks_multiple_snapshots_in_order() {
        let pos = Position::new(Curve::ConstantProduct, 100.0, 1000.0);
        let snapshots = vec![
            PriceSnapshot { timestamp: 0, price: 100.0 },
            PriceSnapshot { timestamp: 3600, price: 120.0 },
            PriceSnapshot { timestamp: 7200, price: 90.0 },
        ];
        let series = pos.compute_series(&snapshots);
        assert_eq!(series.len(), 3);
        assert_eq!(series[0].timestamp, 0);
        assert_eq!(series[1].timestamp, 3600);
        assert_eq!(series[2].timestamp, 7200);
        assert!(approx(series[0].il_percent, 0.0, 1e-9));
    }
}
