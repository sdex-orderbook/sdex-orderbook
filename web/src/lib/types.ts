// SPDX-License-Identifier: Apache-2.0
// Shape of src/data/demo.json, written by scripts/build-data.ts from committed
// fixtures only (fixtures/diff, fixtures/runs, fixtures/probes). Every number the
// UI shows comes from this file or from a live read the user starts.

export interface DiffRun {
  id: string; // fixture directory name
  label: string;
  includePools: boolean;
  capturedAt: string; // ISO UTC
  horizon: string;
  command: string;
  bootstrapCheckpoint: number;
  stateLedgerAtStart: number;
  finalLedger: number;
  firstCaseLedger: number;
  lastCaseLedger: number;
  horizonRequests: number;
  seconds: number;
  kinds: Record<string, KindSummary>;
  replayTotals: { state_checked: number; state_mismatch: number };
}

export interface KindSummary {
  cases: number;
  equal: number;
  mismatched: number;
  skipped: number;
  compared_at_matching_ledger: number;
  best_equal: number;
  equal_ignoring_order: number;
  matched_previous_ledger: number;
}

export interface Level {
  price: string;
  amount: string;
  n: number;
  d: number;
}

export interface OrderBookCase {
  run: string;
  index: number;
  ledger: number;
  capturedAt: string;
  selling: string; // "native" or CODE:ISSUER
  buying: string;
  limit: number;
  equal: boolean;
  bidsEqual: boolean;
  asksEqual: boolean;
  horizon: { bids: Level[]; asks: Level[] };
  ours: { bids: Level[]; asks: Level[] };
  sha256Horizon: string;
  sha256Ours: string;
  query: string;
}

// [timestamp_ms, trade_count, base_volume, counter_volume, avg, high, low, open, close]
export type AggRow = [string, string, string, string, string, string, string, string, string];

export interface AggregationCase {
  run: string;
  index: number;
  ledger: number;
  capturedAt: string;
  base: string;
  counter: string;
  resolution: number;
  windowMs: [number, number];
  equal: boolean;
  horizonBuckets: number;
  ourBuckets: number;
  rowEqual: boolean[];
  horizon: AggRow[];
  ours: AggRow[];
  sha256Horizon: string;
  sha256Ours: string;
  query: string;
}

export interface PathRecord {
  source: string; // asset label "XLM" or CODE
  sourceAmount: string;
  destination: string;
  destinationAmount: string;
  path: string[];
}

export interface PathCase {
  run: string;
  includePools: boolean;
  index: number;
  ledger: number;
  kind: 'strict_send' | 'strict_receive';
  from: string;
  to: string;
  amount: string;
  equal: boolean;
  bestEqual: boolean;
  horizonBest: string | null;
  ourBest: string | null;
  horizon: PathRecord[];
  ours: PathRecord[];
}

export interface BucketRow {
  level: string | null;
  hash: string;
  records_read: number;
  bytes_uncompressed: number;
  offers_live: number;
  offers_dead: number;
  pools_live: number;
  pools_dead: number;
  seconds: number;
}

export interface RunStats {
  provenance: { exportedAt: string; script: string; notes: string[] };
  state: { ledger: number; close_time: number; offers: number; pools: number; sha256: string; bootstrap_checkpoint: number };
  bootstrap: {
    checkpoint: number;
    offers: number;
    pools: number;
    assets: number;
    seconds_this_attempt: number;
    peak_rss_kib: number;
    bytes_uncompressed: number;
    archive: string | null;
    has_header_check: { computed_from_has: string; header_bucket_list_hash: string; ledger: number; match: boolean; protocol: number } | null;
    buckets: BucketRow[];
  };
  replay: {
    from_ledger: number;
    to_ledger: number;
    ledgers: number;
    first_close_time: number;
    last_close_time: number;
    trades: number;
    trade_errors: number;
    state_checked: number;
    state_mismatch: number;
    offers_upserted: number;
    offers_removed: number;
    pools_upserted: number;
    apply_ms_median: number;
    columns: string[];
    rows: number[][];
  };
}

export interface PoolProbe {
  dir: string;
  capturedAt: string;
  script: string;
  poolId: string;
  fee_bp: number;
  reserves: { asset: string; amount: string }[];
  lastModifiedLedger: number;
  lastModifiedTime: string;
  poolLedger: number;
  pathsLedger: number;
  sourceAmount: string;
  paths: { destinationAmount: string; path: string[] }[];
  bookLedger: number;
  bookBids: number;
  bookAsks: number;
}

export interface DemoData {
  generatedFrom: string[];
  runs: DiffRun[];
  orderBooks: OrderBookCase[];
  aggregations: AggregationCase[];
  paths: PathCase[];
  runStats: RunStats;
  probe: PoolProbe | null;
  totals: {
    orderBooks: { cases: number; equal: number };
    aggregations: { cases: number; equal: number };
    pathsWithPools: { cases: number; equal: number };
    pathsNoPools: { cases: number; equal: number };
  };
}
