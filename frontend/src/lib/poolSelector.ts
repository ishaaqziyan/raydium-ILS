// Fixed pool list — mirrors the backend's pools.rs fixed list. MVP scope:
// user picks from this dropdown, no arbitrary pool address input.

export interface PoolOption {
  id: string;
  label: string;
}

export const POOLS: PoolOption[] = [
  { id: "58oQChx4yWmvKdwLLZzBi4ChoCc2fqCUWBkwMihLYQo2", label: "SOL / USDC" },
  { id: "6UmmUiYoBjSrhakAobJw8BvkmJtDVxaeBtbt7rxWo1mg", label: "RAY / USDC" },
  { id: "AVs9TA4nWDzfPJE9gGVNJMVhcQy3V9PGazuz33BfG2RA", label: "RAY / SOL" },
  { id: "7XawhbbxtsRcQA8KTkHT9f9nc6d69UwqCDh6U5EEbEmX", label: "SOL / USDT" },
  { id: "DVa7Qmb5ct9RCpaU7UTpSaf3GVMYz17vNVU67XpdCRut", label: "RAY / USDT" },
  { id: "4yrHms7ekgTBgJg77zJ33TsWrraqHsCXDtuSZqUsuGHb", label: "WETH / SOL" },
  { id: "6gpZ9JkLoYvpA5cwdyPZFsDw6tkbPyyXM5FqRqHxMCny", label: "MSOL / RAY" },
  { id: "EGyhb2uLAsRUbRx9dNFBjMVYnFaASWMvD6RE1aEf2LxL", label: "MSOL / SOL" },
  { id: "ZfvDXXUhZDzDVsapffUyXHj9ByCoPjP4thL6YXcZ9ix", label: "MSOL / USDC" },
  { id: "7TbGqz32RsuwXbXY7EyBCiAnMbJq1gm1wKmfjQjuwoyF", label: "USDT / USDC" },
  { id: "BhuMVCzwFVZMSuc1kBbdcAnXwFg9p4HJp7A9ddwYjsaF", label: "MSOL / USDT" },
  { id: "G7mw1d83ismcQJKkzt62Ug4noXCjVhu3eV7U5EMgge6Z", label: "BONK / USDC" },
  { id: "EoNrn8iUhwgJySD1pHu8Qxm5gSQqLK3za4m8xzD2RuEb", label: "WETH / USDC" },
  { id: "HVNwzt7Pxfu76KHCMQPTLuTCLTm6WnQ1esLv4eizseSv", label: "BONK / SOL" },
  { id: "EYErUp5muPYEEkeaUCY22JibeZX7E9UuMcJFZkmNAN7c", label: "JUP / SOL" },
  { id: "3ucNos4NbumPLZNWztqGHNFFgkHeRMBQAVemeeomsUxv", label: "SOL / USDC (CLMM)" },
  { id: "61R1ndXxvsWXXkWSyNkCxnzwd3zUNB8Q2ibmkiLPC8ht", label: "RAY / USDC (CLMM)" },
];

export function populatePoolSelect(select: HTMLSelectElement): void {
  for (const pool of POOLS) {
    const option = document.createElement("option");
    option.value = pool.id;
    option.textContent = pool.label;
    select.appendChild(option);
  }
}
