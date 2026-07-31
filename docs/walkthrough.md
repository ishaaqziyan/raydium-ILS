# Walkthrough: replaying real impermanent loss for a Raydium pool

Most IL explainers plot a formula over a made-up price range. This project
instead pulls the actual reserve history of a live Raydium pool from
on-chain data and replays it - same math, real path. This walks through how
the pipeline works end to end, including the parts that didn't work on the
first attempt, because that's where the actual engineering was.

## The shape of the problem

Given a pool and a time range, produce a time series of
`{ timestamp, holdValue, lpValue, ilPercent }` and chart it. Three pieces:

1. **Reserve history** - what were the pool's actual token reserves at N
   points over the last week/month? (`backend/src/history.rs`)
2. **IL math** - given that history and a hypothetical deposit, what would
   holding vs. LPing have been worth at each point? (`backend/src/il_calc.rs`,
   see [il-math-deep-dive.md](il-math-deep-dive.md) for the formulas)
3. **Serve it** - one HTTP endpoint tying the two together, and a frontend
   chart. (`backend/src/main.rs`, `frontend/`)

Piece 2 was built and unit-tested first, against known textbook cases,
before touching any real data - so when the numbers looked wrong later, the
question was always "is the *data* wrong" and never "is the *math* wrong."
That separation paid for itself repeatedly.

## Picking a pool, for real

`backend/src/pools.rs` hardcodes SOL/USDC's pool account, vault addresses,
and mints. Those weren't typed in from memory - Raydium's public v3 API
(`api-v3.raydium.io/pools/info/mint`) confirmed the pool ID, and the vault
addresses were decoded live from the pool account's raw on-chain layout
(fetched via Helius `getAccountInfo`, parsed by hand against the known
Raydium AMM v4 `LiquidityStateV4` byte layout) and cross-checked against
the API's reported mints. Both sources agreed, which is the actual bar for
"trust this address" when it's going into a fixed list - a wrong pool
address here would silently produce a chart for some other pool's data with
no error anywhere.

## Attempt 1: walk every signature back from now

The obvious approach: `getSignaturesForAddress` on the pool's vault,
paginate backward with `before`, bucket into fixed intervals, done. This is
what most Solana history tooling does, and it's what the architecture doc
originally specified.

It doesn't scale for a pool this busy. SOL/USDC's base vault sees on the
order of 10+ transactions per second - reference-only, arbitrage, and
routed swaps included, not just direct LP-visible swaps. Five hundred pages
of 1,000 signatures each (500K signatures, the practical ceiling for an
on-demand request) covered about **12 hours** of real time, not the 30 days
needed. Popular-pool volume, not rare-edge-case volume - this is the
default case for exactly the pool you'd pick to demo.

## Attempt 2: binary-search the slot, scan the block

Fix: don't walk every signature. For each target timestamp, estimate the
Solana slot (average ~0.45s/slot, corrected with a few `getBlockTime`
probes), then pull that block and scan its transactions for one touching
the vaults, reading `postTokenBalances` directly - no swap-instruction
decoding needed, which sidesteps the AMM v4 vs. CLMM account-layout
differences entirely.

This is *correct* - a real end-to-end 30-day run produced 121 clean
snapshots with sane, smooth prices - but slow in a specific way that only
showed up under load: a Solana block can hold 2,000–3,000+ transactions,
and in a spot-check of one real block, exactly **one** of 2,610 transactions
even referenced the vault, and that one didn't touch its balance (it was a
routing reference inside a larger aggregator swap). Most of every
multi-megabyte block download was wasted. First-hit latency for a 30-day
pull ranged from about a minute to over ten, depending on how many nearby
slots needed scanning before hitting a real balance-changing transaction.

## Attempt 3: seed the address index instead of scanning blocks

The fix wasn't a new idea, it was combining the first two correctly.
`getSignaturesForAddress` is fast and precise *if* you don't ask it to walk
from "now" - Helius maintains a per-account index, so a `before`-bounded
query near the right point in time resolves instantly. So:

1. Binary-search the slot nearest the target timestamp (same as attempt 2 -
   this part was always cheap and correct).
2. Pull that block in `transactionDetails: "signatures"` mode - a tiny
   payload, just a signature list, used only to anchor a point in time.
3. Feed that signature to `getSignaturesForAddress(vault, { before: seed })`
   - the account-scoped index - to jump straight to real vault-touching
   transactions near that moment.
4. `getTransaction` on a handful of those candidates (some are
   reference-only, same as before, so a few candidates may miss) until one
   actually carries both vaults' post-balances.

Each step downloads a signature list or a single transaction - never a
whole block. The same 30-day, 121-bucket pull that took 1–10+ minutes with
block-scanning dropped to **54 seconds**, with 120 of 121 buckets found
(one bucket landed in a gap with no balance-changing transaction within the
search window, which is an acceptable miss rate for a sampled series, not a
correctness bug). This is what actually makes "no running indexer, computed
on demand" (`ray.md` §3.2, §5) true in practice rather than in theory.

The lesson that generalizes: for any high-volume Solana account, "walk
history from now" and "scan blocks for relevance" both degrade badly with
volume. Address-scoped indexes exist for a reason - the trick is anchoring
them at the right point in time instead of at either extreme.

## Serving it

`backend/src/main.rs` is one Axum endpoint,
`GET /api/il-series?pool=&days=&interval_hours=`, backed by an in-memory
cache keyed on `(pool, days, interval_hours)` - no database, matching the
architecture doc's MVP scope. A repeat request for the same range returns
in single-digit milliseconds; a fresh one costs whatever the Helius round
trips above cost.

## The frontend

Astro (static shell) + Tailwind + Chart.js, plain `<script type="module">`
tags - no component framework, per the architecture doc's explicit choice
to keep review weight on the data pipeline and IL math rather than
frontend plumbing. `frontend/src/lib/` holds three small modules
(`api.ts`, `poolSelector.ts`, `chart.ts`); `pages/index.astro` wires them
together: pick a pool and range, fetch, render the chart (hold value, LP
value, and IL% on a secondary axis), and print a one-line plain-English
summary - "providing liquidity lost 0.003% versus holding" - because the
chart is the credible version and the sentence is the shareable one.

## What to take away

The interesting failure here wasn't a bug in the usual sense - every
attempt above returned *correct* numbers. The problem was always
throughput: how many round trips, and how big each payload, for a query
shape (dense recent history on a high-volume account) that's the norm for
any pool worth demoing. That's also why this made a better portfolio piece
than a clean first-try implementation would have: the real constraint
wasn't the IL formula, which is well-understood and takes an afternoon to
implement and test - it was building a data pipeline that stays honest
(real on-chain reserves, not synthetic prices) while staying fast enough to
be a live demo instead of a batch job.
