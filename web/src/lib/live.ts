// SPDX-License-Identifier: Apache-2.0
// Live mode: read-only GETs to a local `sdx serve` the user started. The page
// never writes, never signs, and labels everything it reads with the ledger the
// server reported in its Latest-Ledger header.
import { assetParams } from './assets.ts';
import type { Level } from './types.ts';

export interface LiveStatus {
  ledger: number;
  ledgerCloseTime: number;
  offers: number;
  pools: number;
  network: string;
}

export interface LiveBook {
  ledger: number;
  bids: Level[];
  asks: Level[];
}

export function orderBookUrl(base: string, selling: string, buying: string, limit = 20): string {
  const b = base.replace(/\/+$/, '');
  return `${b}/order_book?${assetParams('selling', selling)}&${assetParams('buying', buying)}&limit=${limit}`;
}

/** Only loopback servers: the live panel is for a local sdx, not arbitrary hosts. */
export function isLoopback(base: string): boolean {
  try {
    const u = new URL(base);
    return (u.protocol === 'http:' || u.protocol === 'https:') && ['127.0.0.1', 'localhost', '[::1]'].includes(u.hostname);
  } catch {
    return false;
  }
}

type Fetch = (url: string, init?: RequestInit) => Promise<Response>;

function ledgerHeader(r: Response): number {
  const v = Number(r.headers.get('Latest-Ledger'));
  if (!Number.isFinite(v) || v <= 0) throw new Error('response has no Latest-Ledger header (is this sdx serve?)');
  return v;
}

export async function fetchStatus(base: string, f: Fetch = fetch): Promise<LiveStatus> {
  const r = await f(`${base.replace(/\/+$/, '')}/`, { signal: AbortSignal.timeout(4000) });
  if (!r.ok) throw new Error(`GET / returned ${r.status}`);
  const j = (await r.json()) as Record<string, unknown>;
  if (j.service !== 'sdex-orderbook') throw new Error('the server at this address is not sdx serve');
  return {
    ledger: Number(j.ledger),
    ledgerCloseTime: Number(j.ledger_close_time),
    offers: Number(j.offers),
    pools: Number(j.liquidity_pools),
    network: String(j.network),
  };
}

export async function fetchOrderBook(base: string, selling: string, buying: string, f: Fetch = fetch): Promise<LiveBook> {
  const r = await f(orderBookUrl(base, selling, buying), { signal: AbortSignal.timeout(8000) });
  const ledger = ledgerHeader(r);
  if (!r.ok) throw new Error(`GET /order_book returned ${r.status}`);
  const j = (await r.json()) as { bids: { price: string; amount: string; price_r: { n: number; d: number } }[]; asks: typeof j.bids };
  const lv = (l: (typeof j.bids)[number]): Level => ({ price: l.price, amount: l.amount, n: Number(l.price_r.n), d: Number(l.price_r.d) });
  return { ledger, bids: j.bids.map(lv), asks: j.asks.map(lv) };
}

/** A read is labelled "live" only when the served ledger closed this recently (seconds). */
export const LIVE_MAX_AGE_S = 600;

/** Age of served data for display, e.g. "4 days old". Negative means the clocks disagree. */
export function ageLabel(sec: number): string {
  if (sec < 0) return 'ledger close time is ahead of this clock';
  if (sec < 3600) return `${Math.round(sec / 60)} min old`;
  if (sec < 172800) return `${(sec / 3600).toFixed(1)} h old`;
  return `${Math.floor(sec / 86400)} days old`;
}
