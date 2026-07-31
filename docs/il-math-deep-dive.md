# IL math deep dive

This explains the formulas implemented in `backend/src/il_calc.rs`, why the
textbook IL formula is a special case of what this project actually
computes, and why replaying real reserve history is the harder - and more
honest - version of the problem.

## 1. The two strategies

Impermanent loss compares two ways of holding the same starting capital:

- **Hold**: keep the original token split, value it at the current price.
- **LP**: deposit the tokens into the pool, let the curve rebalance them as
  price moves, value the resulting position at the current price.

Both are evaluated at the *same* price point, so the comparison isolates
exactly one thing: what the automated-market-maker's rebalancing cost you
(or made you), independent of whether the asset went up or down overall.

Throughout, `price` means "value of token0 in token1 terms" - e.g. USDC per
SOL for the SOL/USDC pool - and all dollar-style values are in token1
units.

## 2. Constant product (AMM v4)

A constant-product pool holds reserves `x` (token0) and `y` (token1) such
that `x * y = k`, a constant. At price `p`, arbitrage keeps `y / x = p`, so:

```
x = sqrt(k / p)
y = sqrt(k * p)
```

**Entry.** For a balanced deposit of value `V0` at entry price `p0`, half
the value goes to each side:

```
amount0 = V0 / (2 * p0)
amount1 = V0 / 2
k = amount0 * amount1
```

**At any later price `p`:**

```
hold_value(p) = amount0 * p + amount1
lp_value(p)   = x * p + y = 2 * sqrt(k * p)
il_percent(p) = (lp_value(p) - hold_value(p)) / hold_value(p) * 100
```

This collapses to the textbook single-hop formula when you only look at the
entry and exit price. Writing `r = p / p0` (the price ratio since entry):

```
IL(r) = 2 * sqrt(r) / (1 + r) - 1
```

Sanity checks baked into `il_calc.rs`'s test suite:

| price move | IL |
|---|---|
| none (`r = 1`) | 0% |
| 2x (`r = 2`) | −5.72% |
| 4x (`r = 4`) | **−20.00%** exactly |
| `r` and `1/r` | identical magnitude - IL is symmetric in log-price |

`lp_value(p) <= hold_value(p)` for every price, always - a constant-product
LP position can never beat holding at any single price point (it can beat
holding cumulatively via fees, which this project doesn't model - see
§4). That invariant is asserted directly in the test suite, not just
spot-checked.

## 3. Concentrated liquidity (CLMM), restricted to a fixed range

A CLMM position only provides liquidity inside `[price_lower, price_upper]`.
Standard Uniswap-v3-style liquidity math applies (Raydium's CLMM program
uses the same tick-range mechanics): with `sqrtPa = sqrt(price_lower)`,
`sqrtPb = sqrt(price_upper)`, and liquidity constant `L`:

```
in range:        amount0(p) = L * (1/sqrt(p) - 1/sqrtPb)
                  amount1(p) = L * (sqrt(p) - sqrtPa)
below range:      amount0 = L * (1/sqrtPa - 1/sqrtPb),  amount1 = 0
above range:      amount0 = 0,  amount1 = L * (sqrtPb - sqrtPa)
```

Below the range the whole position has converted to token0; above it,
entirely to token1 - the position stops "tracking" further price movement
in that direction, which is exactly what happens physically when a tick
range is fully exhausted.

**Solving for `L` from a target entry value** (rather than asking the user
to specify raw token amounts) makes this usable from a single deposit-value
input:

```
value(p0) = L * (2*sqrt(p0) - sqrtPa - p0/sqrtPb)
L = V0 / (2*sqrt(p0) - sqrtPa - p0/sqrtPb)
```

MVP scope restricts this to a *single fixed, realistic tick range* per the
architecture doc - no user-chosen ranges - since a CLMM position's
underlying amounts depend on that specific range, not on pool-wide
reserves, and letting the range vary turns this into a much bigger input
surface than an MVP needs.

**Why CLMM shows *more* IL than full-range for the same price move**: a
tighter range means the position's `L` is concentrated into less price
space, so it rebalances faster (in token terms) per unit of price change.
`il_calc.rs` has a test asserting exactly this - a ±10% range around entry
shows strictly worse IL than full-range constant-product for the same price
move. This is the standard "concentrated liquidity amplifies IL" result,
and it falls directly out of the formulas above rather than being asserted
separately.

## 4. What this doesn't model

- **Trading fees.** Real LPs earn a cut of swap volume, which can offset -
  sometimes fully - the IL shown here. This project only shows the
  rebalancing cost side of the equation, deliberately: mixing in fee
  revenue would require decoding fee accrual per LP position, which is a
  materially different (and per-position) data problem than reserve
  history. The chart's honest framing is "what did the curve cost you,"
  not "were you net profitable."
- **Real-world entry timing.** The simulator picks the earliest available
  snapshot in the requested window as the entry point. A real LP's actual
  entry could be any point along the path.

## 5. Textbook IL vs. real-path IL - the actual point of this project

The formula in §2 is usually presented as a single before/after
calculation: pick an entry price and an exit price, plug into
`IL(r) = 2*sqrt(r)/(1+r) - 1`, done. That's mathematically correct but
represents a price path that almost never happens - real prices don't move
in one clean hop from `p0` to `p1`; they wander, and IL along the way is
what the position actually earned or lost against hold at every point in
between, which the single-hop formula throws away entirely.

This project instead re-evaluates `hold_value` and `lp_value` at *every*
sampled point in the pool's real reserve history (`backend/src/history.rs`
reconstructs that history from live Helius data - see the walkthrough for
how), so the IL curve shown is the actual path, snapshot by snapshot, not
an interpolation and not a synthetic price series. That's the "meaningfully
harder version of the problem" the architecture doc set out to solve, and
it's the reason the chart can say something concrete like "SOL/USDC LPs
were down 0.12% versus holding at the worst point in the last 30 days" - a
claim about what actually happened, not what a formula predicts would
happen under an assumption that rarely holds.
