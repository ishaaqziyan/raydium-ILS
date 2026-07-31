//! Fixed pool list for MVP scope — user picks from this dropdown, no
//! arbitrary pool address input. Every pool below was decoded live from its
//! on-chain Raydium AMM v4 account (`LiquidityStateV4` layout: baseVault @
//! byte 336, quoteVault @ 368, baseMint @ 400, quoteMint @ 432, decimals @
//! 32/40), verified to be owned by the AMM v4 program
//! (`675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8`) rather than Raydium's
//! Stable-swap or newer CPMM programs (which share the "standard" pool type
//! label in Raydium's API but use different account layouts entirely — two
//! candidate pools decoded as garbage before this check caught them), and
//! cross-checked against Raydium's public v3 API and Jupiter's token search
//! for mint/symbol accuracy. Selected from real AMM v4 pools with
//! meaningful on-chain liquidity (TVL > $6k at decode time) so each has
//! actual swap history to replay — a handful of very well-known pairs
//! (JitoSOL/SOL, WBTC/*, PYTH/USDC) have migrated entirely to CLMM or CPMM
//! and have no live AMM v4 pool left, so they're not in this list.
//!
//! Two CLMM pools are also included (SOL/USDC and RAY/USDC, paired with
//! their AMM v4 counterparts above for a direct full-range-vs-concentrated
//! comparison). Their account layout is Raydium's `PoolState` (program
//! `CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK`): tokenMint0 @ byte 73,
//! tokenMint1 @ 105, tokenVault0 @ 137, tokenVault1 @ 169, decimals @
//! 233/234, tickSpacing @ 235, sqrtPriceX64 @ 253, tickCurrent @ 269 —
//! verified live by byte-searching the raw account data for the known SOL
//! and USDC mint pubkeys (found at exactly 73/105) rather than trusting
//! struct-layout memory, then cross-checking the price implied by
//! `sqrtPriceX64` against the price implied by `tickCurrent`
//! (`1.0001^tick`) — two independent fields in the same account that
//! should always agree, and did (within rounding). Per the architecture
//! doc's MVP scope, each CLMM pool's tick range is fixed at decode time
//! (±20% of spot price then), not user-selectable.

#[derive(Debug, Clone, Copy)]
pub enum CurveKind {
    AmmV4,
    Clmm { price_lower: f64, price_upper: f64 },
}

#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    pub id: &'static str,
    pub label: &'static str,
    pub base_mint: &'static str,
    pub quote_mint: &'static str,
    pub base_vault: &'static str,
    pub quote_vault: &'static str,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub curve: CurveKind,
}

pub const SOL_USDC: PoolConfig = PoolConfig {
    id: "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2",
    label: "SOL / USDC",
    base_mint: "So11111111111111111111111111111111111111112",
    quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    base_vault: "DQyrAcCrDXQ7NeoqGgDCZwBvWDcYmFCjSb9JtteuvPpz",
    quote_vault: "HLmqeL62xR1QoZ1HKKbXRrdN1p3phKpxRMb2VVopvBBz",
    base_decimals: 9,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const RAY_USDC: PoolConfig = PoolConfig {
    id: "6UmmUiYoBjSrhakAobJw8BvkmJtDVxaeBtbt7rxWo1mg",
    label: "RAY / USDC",
    base_mint: "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R",
    quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    base_vault: "FdmKUE4UMiJYFK5ogCngHzShuVKrFXBamPWcewDr31th",
    quote_vault: "Eqrhxd7bDUCH3MepKmdVkgwazXRzY6iHhEoBpY7yAohk",
    base_decimals: 6,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const RAY_SOL: PoolConfig = PoolConfig {
    id: "AVs9TA4nWDzfPJE9gGVNJMVhcQy3V9PGazuz33BfG2RA",
    label: "RAY / SOL",
    base_mint: "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R",
    quote_mint: "So11111111111111111111111111111111111111112",
    base_vault: "Em6rHi68trYgBFyJ5261A2nhwuQWfLcirgzZZYoRcrkX",
    quote_vault: "3mEFzHsJyu2Cpjrz6zPmTzP7uoLFj9SbbecGVzzkL1mJ",
    base_decimals: 6,
    quote_decimals: 9,
    curve: CurveKind::AmmV4,
};

pub const SOL_USDT: PoolConfig = PoolConfig {
    id: "7XawhbbxtsRcQA8KTkHT9f9nc6d69UwqCDh6U5EEbEmX",
    label: "SOL / USDT",
    base_mint: "So11111111111111111111111111111111111111112",
    quote_mint: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
    base_vault: "876Z9waBygfzUrwwKFfnRcc7cfY4EQf6Kz1w7GRgbVYW",
    quote_vault: "CB86HtaqpXbNWbq67L18y5x2RhqoJ6smb7xHUcyWdQAQ",
    base_decimals: 9,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const RAY_USDT: PoolConfig = PoolConfig {
    id: "DVa7Qmb5ct9RCpaU7UTpSaf3GVMYz17vNVU67XpdCRut",
    label: "RAY / USDT",
    base_mint: "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R",
    quote_mint: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
    base_vault: "3wqhzSB9avepM9xMteiZnbJw75zmTBDVmPFLTQAGcSMN",
    quote_vault: "5GtSbKJEPaoumrDzNj4kGkgZtfDyUceKaHrPziazALC1",
    base_decimals: 6,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const WETH_SOL: PoolConfig = PoolConfig {
    id: "4yrHms7ekgTBgJg77zJ33TsWrraqHsCXDtuSZqUsuGHb",
    label: "WETH / SOL",
    base_mint: "7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs",
    quote_mint: "So11111111111111111111111111111111111111112",
    base_vault: "5ushog8nHpHmYVJVfEs3NXqPJpne21sVZNuK3vqm8Gdg",
    quote_vault: "CWGyCCMC7xmWJZgAynhfAG7vSdYoJcmh27FMwVPsGuq5",
    base_decimals: 8,
    quote_decimals: 9,
    curve: CurveKind::AmmV4,
};

pub const MSOL_RAY: PoolConfig = PoolConfig {
    id: "6gpZ9JkLoYvpA5cwdyPZFsDw6tkbPyyXM5FqRqHxMCny",
    label: "MSOL / RAY",
    base_mint: "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So",
    quote_mint: "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R",
    base_vault: "BusJVbHEkJeYRpHkqCrt85d1LALS1EVcKRjqRFZtBSty",
    quote_vault: "GM1CjxKixFkKpakxx5Lg9u3zYjXAK2Gr2pzoy1G88Td5",
    base_decimals: 9,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const MSOL_SOL: PoolConfig = PoolConfig {
    id: "EGyhb2uLAsRUbRx9dNFBjMVYnFaASWMvD6RE1aEf2LxL",
    label: "MSOL / SOL",
    base_mint: "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So",
    quote_mint: "So11111111111111111111111111111111111111112",
    base_vault: "85SxT7AdDQvJg6pZLoDf7vPiuXLj5UYZLVVNWD1NjnFK",
    quote_vault: "BtGUR6y7uwJ6UGXNMcY3gCLm7dM3WaBdmgtKVgGnE1TJ",
    base_decimals: 9,
    quote_decimals: 9,
    curve: CurveKind::AmmV4,
};

pub const MSOL_USDC: PoolConfig = PoolConfig {
    id: "ZfvDXXUhZDzDVsapffUyXHj9ByCoPjP4thL6YXcZ9ix",
    label: "MSOL / USDC",
    base_mint: "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So",
    quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    base_vault: "8JUjWjAyXTMB4ZXcV7nk3p6Gg1fWAAoSck7xekuyADKL",
    quote_vault: "DaXyxj42ZDrp3mjrL9pYjPNyBp5P8A2f37am4Kd4EyrK",
    base_decimals: 9,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const USDT_USDC: PoolConfig = PoolConfig {
    id: "7TbGqz32RsuwXbXY7EyBCiAnMbJq1gm1wKmfjQjuwoyF",
    label: "USDT / USDC",
    base_mint: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
    quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    base_vault: "Enb9jGaKzgDBfEbbUN3Ytx2ZLoZuBhBpjVX6DULiRmvu",
    quote_vault: "HyyZpz1JUZjsfyiVSt3qz6E9PkwnBcyhUg4zKGthMNeH",
    base_decimals: 6,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const MSOL_USDT: PoolConfig = PoolConfig {
    id: "BhuMVCzwFVZMSuc1kBbdcAnXwFg9p4HJp7A9ddwYjsaF",
    label: "MSOL / USDT",
    base_mint: "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So",
    quote_mint: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB",
    base_vault: "FaoMKkKzMDQaURce1VLewT6K38F6FQS5UQXD1mTXJ2Cb",
    quote_vault: "GE8m3rHHejrNf4jE96n5gzMmLbxTfPPcmv9Ppaw24FZa",
    base_decimals: 9,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const BONK_USDC: PoolConfig = PoolConfig {
    id: "G7mw1d83ismcQJKkzt62Ug4noXCjVhu3eV7U5EMgge6Z",
    label: "BONK / USDC",
    base_mint: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263",
    quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    base_vault: "B4kvik9yEXTJ1UScDUqTxfvhnHUkfChv5pF6zNSyEraQ",
    quote_vault: "5BsukuACaQcjVu9XSZ8PP3BN9pBxxtZjQELxosC4Ucyx",
    base_decimals: 5,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const WETH_USDC: PoolConfig = PoolConfig {
    id: "EoNrn8iUhwgJySD1pHu8Qxm5gSQqLK3za4m8xzD2RuEb",
    label: "WETH / USDC",
    base_mint: "7vfCXTUXx5WJV5JADk17DUJ4ksgau7utNKj4b963voxs",
    quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    base_vault: "DVWRhoXKCoRbvC5QUeTECRNyUSU1gwUM48dBMDSZ88U",
    quote_vault: "HftKFJJcUTu6xYcS75cDkm3y8HEkGgutcbGsdREDWdMr",
    base_decimals: 8,
    quote_decimals: 6,
    curve: CurveKind::AmmV4,
};

pub const BONK_SOL: PoolConfig = PoolConfig {
    id: "HVNwzt7Pxfu76KHCMQPTLuTCLTm6WnQ1esLv4eizseSv",
    label: "BONK / SOL",
    base_mint: "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263",
    quote_mint: "So11111111111111111111111111111111111111112",
    base_vault: "7KFdXKA5WkZBspxwqd9kSrDGTg9WhiX5TptUB3yRwEaE",
    quote_vault: "GehmCo7EgzkB4xxyviW6xdUhm1Ed2nN98QcfcRWQCfA9",
    base_decimals: 5,
    quote_decimals: 9,
    curve: CurveKind::AmmV4,
};

pub const JUP_SOL: PoolConfig = PoolConfig {
    id: "EYErUp5muPYEEkeaUCY22JibeZX7E9UuMcJFZkmNAN7c",
    label: "JUP / SOL",
    base_mint: "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN",
    quote_mint: "So11111111111111111111111111111111111111112",
    base_vault: "4xmePoAm93k4KrMzkG7UuVqKnF5WUNUHpXdavXGECaN2",
    quote_vault: "4nR7HAVv7TDh8mzg7XLbsBnVfgMEGyL7EBq7WuR6ZB16",
    base_decimals: 6,
    quote_decimals: 9,
    curve: CurveKind::AmmV4,
};

pub const SOL_USDC_CLMM: PoolConfig = PoolConfig {
    id: "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv",
    label: "SOL / USDC (CLMM)",
    base_mint: "So11111111111111111111111111111111111111112",
    quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    base_vault: "4ct7br2vTPzfdmY3S5HLtTxcGSBfn6pnw98hsS6v359A",
    quote_vault: "5it83u57VRrVgc51oNV19TTmAJuffPx5GtGwQr7gQNUo",
    base_decimals: 9,
    quote_decimals: 6,
    // Spot price ~73.48 at decode time; ±20% fixed range.
    curve: CurveKind::Clmm { price_lower: 58.78, price_upper: 88.18 },
};

pub const RAY_USDC_CLMM: PoolConfig = PoolConfig {
    id: "61R1ndXxvsWXXkWSyNkCxnzwd3zUNB8Q2ibmkiLPC8ht",
    label: "RAY / USDC (CLMM)",
    base_mint: "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R",
    quote_mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
    base_vault: "EtjUEdstCK856io1ZNoGs9aamsjBJaSm6rmNYz5uTKqv",
    quote_vault: "JBGAcAQP59HrqWsvUF9pXyQjfSpL7ivH3uEt95t7KahY",
    base_decimals: 6,
    quote_decimals: 6,
    // Spot price ~0.610 at decode time; ±20% fixed range.
    curve: CurveKind::Clmm { price_lower: 0.488, price_upper: 0.732 },
};

pub fn fixed_pools() -> &'static [PoolConfig] {
    &[
        SOL_USDC, RAY_USDC, RAY_SOL, SOL_USDT, RAY_USDT, WETH_SOL, MSOL_RAY, MSOL_SOL, MSOL_USDC,
        USDT_USDC, MSOL_USDT, BONK_USDC, WETH_USDC, BONK_SOL, JUP_SOL, SOL_USDC_CLMM, RAY_USDC_CLMM,
    ]
}

pub fn find_pool(id: &str) -> Option<&'static PoolConfig> {
    fixed_pools().iter().find(|p| p.id == id)
}
