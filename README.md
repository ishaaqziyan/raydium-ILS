# Raydium Impermanent Loss Simulator

[![CI](https://github.com/ishaaqziyan/Raydium-ILS/actions/workflows/ci.yml/badge.svg)](https://github.com/ishaaqziyan/Raydium-ILS/actions/workflows/ci.yml)

Replays real impermanent loss against actual historical price and reserve
data for a Raydium pool - not a theoretical IL curve plotted over a
made-up price range.

See [`ray.md`](../ray.md) for the original architecture doc,
[`docs/il-math-deep-dive.md`](docs/il-math-deep-dive.md) for the IL formulas,
and [`docs/walkthrough.md`](docs/walkthrough.md) for how the data pipeline
actually got built (including the parts that didn't work on the first try).

## What it does

Pick a pool (fixed list of 15 well-known Raydium AMM v4 pools : SOL/USDC,
RAY/USDC, RAY/SOL, mSOL pairs, BONK, WETH, and more, see
`backend/src/pools.rs`) and a time range (7 or 30 days).

The backend pulls that pool's real on-chain reserve history from Helius,
replays a hypothetical deposit against it under the pool's actual
constant-product curve, and returns a time series of hold value, LP value,
and IL% at each sampled point. 

The frontend charts it and prints a one-line plain-English readout.

## Running it

Requires the `HELIUS_API_KEY` env var (a Helius RPC API key). 
This repo
uses [Doppler](https://doppler.com) to manage it  
project
`raydium-il-simulator`, config `dev`.

Recipes are in the [`justfile`](justfile) ([`just`](https://github.com/casey/just)):

```bash
just install   # first time only - frontend deps
just backend   # http://localhost:3001
just frontend  # http://localhost:4321
just test      # backend unit tests
just check     # frontend type-check
just lint      # cargo fmt --check + clippy
just           # list all recipes
```

Or run the underlying commands directly:

```bash
# backend - http://localhost:3001
cd backend
doppler run -- cargo run

# frontend - http://localhost:4321
cd frontend
npm install   # first time only
npx astro dev
```

Without Doppler, export `HELIUS_API_KEY` yourself before running the
backend.

**First request for any given pool/range is slow-ish** : it's pulling real
on-chain history live, no indexer running in the background (by design; see
the architecture doc's MVP scope). 
A 30-day/6-hour-bucket pull takes on the
order of a minute; repeat requests for the same pool/range hit an in-memory
cache and return in milliseconds. See the walkthrough for why this took a
few tries to get fast.

## Repo layout

```
backend/     Rust, Axum. il_calc.rs (core IL math, unit tested),
             history.rs (Helius pull + reserve reconstruction),
             pools.rs (fixed pool list), main.rs (HTTP endpoint).
frontend/    Astro + Tailwind, vanilla TypeScript modules, Chart.js.
             No component framework.
docs/        IL math deep-dive, build walkthrough.
justfile     dev/test/build recipes (just --list).
```

## Tests

```bash
just test
# or: cd backend && cargo test
```

11 unit tests in `il_calc.rs` cover the constant-product and CLMM curves
against known textbook cases (a 4x price move produces exactly −20% IL,
etc.) 
see the deep-dive doc for the full table.
