// SPDX-License-Identifier: Apache-2.0
// Formatting for the demo views. Amounts stay the strings Horizon and sdx
// print (7 decimals); these helpers only format times, sizes and labels.

const RES: Record<number, string> = {
  60000: '1 min',
  300000: '5 min',
  900000: '15 min',
  3600000: '1 hour',
  86400000: '1 day',
  604800000: '1 week',
};

export function resolutionLabel(ms: number): string {
  return RES[ms] ?? `${ms} ms`;
}

/** "2026-11-04 17:05 UTC" from unix seconds or milliseconds. */
export function utc(t: number | string, unit: 's' | 'ms' = 's'): string {
  const d = typeof t === 'string' ? new Date(t) : new Date(unit === 's' ? t * 1000 : t);
  const s = d.toISOString();
  return `${s.slice(0, 10)} ${s.slice(11, 16)} UTC`;
}

/** "17:05" (UTC) from milliseconds. */
export function hhmm(ms: number): string {
  return new Date(ms).toISOString().slice(11, 16);
}

/** ISO date (YYYY-MM-DD) from an ISO timestamp. */
export function day(iso: string): string {
  return iso.slice(0, 10);
}

/** Bytes as "7.11 GB" (decimal units, as the README uses). */
export function bytes(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(2)} GB`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`;
  if (n >= 1e3) return `${(n / 1e3).toFixed(1)} kB`;
  return `${n} B`;
}

/** Seconds as "41 min 0 s" / "2.6 s". */
export function duration(s: number): string {
  if (s < 60) return `${s.toFixed(1)} s`;
  const m = Math.floor(s / 60);
  const r = Math.round(s - m * 60);
  return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : `${m} min ${r} s`;
}

const INT = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 });
export function int(n: number): string {
  return INT.format(n);
}

/** Horizon decimal string to number for drawing only (never for display). */
export function num(s: string): number {
  return Number(s);
}
