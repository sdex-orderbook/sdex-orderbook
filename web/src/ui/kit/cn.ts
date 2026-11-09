// SPDX-License-Identifier: Apache-2.0
// Tiny class joiner. Copy with the components; no dependency needed.

export function cn(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(' ');
}

const LEDGER = new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 });
const COMPACT = new Intl.NumberFormat('en-US', { notation: 'compact', maximumFractionDigits: 1 });
const FULL = new Intl.NumberFormat('en-US', { maximumFractionDigits: 2 });

/** 65293477 -> "65,293,477". Ledger sequence numbers are always shown in full. */
export function formatLedger(n: number): string {
  return LEDGER.format(n);
}

/** 1284 -> "1,284", 12940 -> "12.9K". Values under 10,000 are shown in full. */
export function formatCompact(n: number): string {
  return Math.abs(n) < 10_000 ? FULL.format(n) : COMPACT.format(n);
}

/** Exact value for tooltips and screen readers next to a compact display. */
export function formatExact(n: number): string {
  return FULL.format(n);
}

/** "abcdef...uvwxyz" for hashes and addresses; the full value goes in `title`. */
export function truncateMiddle(value: string, head = 6, tail = 6): string {
  return value.length <= head + tail + 1 ? value : `${value.slice(0, head)}…${value.slice(-tail)}`;
}
