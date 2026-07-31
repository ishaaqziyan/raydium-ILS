// Fetch the IL time series from the backend. See raydium-il-backend's
// main.rs for the exact response shape — kept in sync by hand since this
// is a small, single-endpoint MVP.

export interface IlPoint {
  timestamp: number;
  holdValue: number;
  lpValue: number;
  ilPercent: number;
}

export interface IlSeriesResponse {
  poolId: string;
  poolLabel: string;
  entryPrice: number;
  depositUsd: number;
  points: IlPoint[];
}

const API_BASE = import.meta.env.PUBLIC_API_BASE ?? "http://localhost:3001";

export async function fetchIlSeries(
  poolId: string,
  days: 7 | 30 | 90,
  depositUsd?: number
): Promise<IlSeriesResponse> {
  const url = new URL("/api/il-series", API_BASE);
  url.searchParams.set("pool", poolId);
  url.searchParams.set("days", String(days));
  if (depositUsd !== undefined) {
    url.searchParams.set("deposit_usd", String(depositUsd));
  }

  const res = await fetch(url.toString());
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`API error ${res.status}: ${body}`);
  }
  return res.json();
}
