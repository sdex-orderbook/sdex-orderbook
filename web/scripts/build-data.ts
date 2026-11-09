// SPDX-License-Identifier: Apache-2.0
// pnpm demo:data. Builds src/data/demo.json from committed fixtures only:
//   ../fixtures/diff/<run>/      sdx diff runs vs horizon.stellar.org (both bodies per case)
//   ../fixtures/runs/<name>/     bootstrap + replay statistics (scripts/export-run-stats.py)
//   ../fixtures/probes/<name>/   read-only Horizon probes (scripts/probe-pool.sh)
// No network, no data/ directory. Counts are recomputed from the case files and
// checked against each run's summary.json; a disagreement aborts the build.
import { readFileSync, readdirSync, writeFileSync, existsSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { zstdDecompressSync } from 'node:zlib';
import { canonicalJson, jsonEqual } from '../src/lib/canonical.ts';
import { horizonAssetLabel } from '../src/lib/assets.ts';
import type {
  AggRow,
  AggregationCase,
  DemoData,
  DiffRun,
  KindSummary,
  Level,
  OrderBookCase,
  PathCase,
  PathRecord,
  PoolProbe,
  RunStats,
} from '../src/lib/types.ts';

const here = dirname(fileURLToPath(import.meta.url));
const project = join(here, '..', '..');
const fixtures = join(project, 'fixtures');
const out = join(here, '..', 'src', 'data', 'demo.json');

type Json = any; // eslint-disable-line @typescript-eslint/no-explicit-any

const readJson = (p: string): Json => JSON.parse(readFileSync(p, 'utf8'));
const sha = (v: unknown): string => createHash('sha256').update(canonicalJson(v)).digest('hex');
const iso = (unix: number): string => new Date(unix * 1000).toISOString().replace('.000Z', 'Z');

function level(l: Json): Level {
  return { price: l.price, amount: l.amount, n: Number(l.price_r.n), d: Number(l.price_r.d) };
}

function aggRow(r: Json): AggRow {
  return [r.timestamp, r.trade_count, r.base_volume, r.counter_volume, r.avg, r.high, r.low, r.open, r.close];
}

function pathRecord(r: Json): PathRecord {
  return {
    source: r.source_asset_type === 'native' ? 'XLM' : r.source_asset_code,
    sourceAmount: r.source_amount,
    destination: r.destination_asset_type === 'native' ? 'XLM' : r.destination_asset_code,
    destinationAmount: r.destination_amount,
    path: (r.path as Json[]).map(horizonAssetLabel),
  };
}

const runs: DiffRun[] = [];
const orderBooks: OrderBookCase[] = [];
const aggregations: AggregationCase[] = [];
const paths: PathCase[] = [];
const generatedFrom: string[] = [];

const diffRoot = join(fixtures, 'diff');
for (const runId of readdirSync(diffRoot).sort()) {
  const dir = join(diffRoot, runId);
  const prov = readJson(join(dir, 'diff.provenance.json'));
  const summary = readJson(join(dir, 'summary.json'));
  const includePools = !String(prov.command).includes('--no-pools');
  generatedFrom.push(`fixtures/diff/${runId}`);
  const recount: Record<string, { cases: number; equal: number }> = {};
  let firstLedger = Number.MAX_SAFE_INTEGER;
  let lastLedger = 0;

  for (const f of readdirSync(join(dir, 'cases')).sort()) {
    const c = JSON.parse(zstdDecompressSync(readFileSync(join(dir, 'cases', f))).toString('utf8'));
    const kind: string = c.case.kind;
    // Point-in-time cases are answered at Horizon's Latest-Ledger (`ledger`);
    // aggregation cases cover a closed window of minutes and carry only the
    // state ledger they were computed at (`our_ledger`), SPEC 6 step 3.
    const ledger: number = kind === 'trade_aggregations' ? c.our_ledger : c.ledger;
    if (c.our_ledger !== ledger) throw new Error(`${runId}/${f}: compared at ${c.our_ledger}, Horizon at ${ledger}`);
    firstLedger = Math.min(firstLedger, ledger);
    lastLedger = Math.max(lastLedger, ledger);
    const capturedAt = iso(c.captured_at_unix);
    const hb = c.horizon.body;
    const ob = c.ours;
    const equal = c.compare.equal === true;
    const rc = (recount[kind] ??= { cases: 0, equal: 0 });
    rc.cases += 1;
    if (equal) rc.equal += 1;

    if (kind === 'order_book') {
      // Re-check what sdx diff compared: bids and asks as JSON values.
      const h = { bids: hb.bids, asks: hb.asks };
      const o = { bids: ob.bids, asks: ob.asks };
      if (jsonEqual(h, o) !== equal) throw new Error(`${runId}/${f}: recomputed equality differs from the fixture`);
      orderBooks.push({
        run: runId,
        index: c.index,
        ledger,
        capturedAt,
        selling: c.case.selling,
        buying: c.case.buying,
        limit: c.case.limit,
        equal,
        bidsEqual: c.compare.bids.equal,
        asksEqual: c.compare.asks.equal,
        horizon: { bids: hb.bids.map(level), asks: hb.asks.map(level) },
        ours: { bids: ob.bids.map(level), asks: ob.asks.map(level) },
        sha256Horizon: sha(h),
        sha256Ours: sha(o),
        query: c.request.query,
      });
    } else if (kind === 'trade_aggregations') {
      const h: Json[] = hb._embedded.records;
      const o: Json[] = ob._embedded.records;
      if (jsonEqual(h, o) !== equal) throw new Error(`${runId}/${f}: recomputed equality differs from the fixture`);
      const n = Math.max(h.length, o.length);
      const rowEqual: boolean[] = [];
      for (let i = 0; i < n; i++) rowEqual.push(h[i] !== undefined && o[i] !== undefined && jsonEqual(h[i], o[i]));
      aggregations.push({
        run: runId,
        index: c.index,
        ledger,
        capturedAt,
        base: c.case.base,
        counter: c.case.counter,
        resolution: c.case.resolution,
        windowMs: c.request.window_ms,
        equal,
        horizonBuckets: h.length,
        ourBuckets: o.length,
        rowEqual,
        horizon: h.map(aggRow),
        ours: o.map(aggRow),
        sha256Horizon: sha(h),
        sha256Ours: sha(o),
        query: c.request.query,
      });
    } else if (kind === 'strict_send' || kind === 'strict_receive') {
      const h: Json[] = hb?._embedded?.records ?? [];
      const o: Json[] = ob?._embedded?.records ?? [];
      const send = kind === 'strict_send';
      paths.push({
        run: runId,
        includePools,
        index: c.index,
        ledger,
        kind,
        from: send ? c.case.source : (c.case.sources as string[]).join(','),
        to: send ? (c.case.destinations as string[]).join(',') : c.case.destination,
        amount: c.case.amount,
        equal,
        bestEqual: c.compare.best_equal === true,
        horizonBest: c.compare.horizon_best ?? null,
        ourBest: c.compare.our_best ?? null,
        horizon: h.map(pathRecord),
        ours: o.map(pathRecord),
      });
    } else {
      throw new Error(`${runId}/${f}: unknown case kind ${kind}`);
    }
  }

  // The recount must agree with the run's own summary.json.
  for (const [kind, s] of Object.entries(summary.kinds as Record<string, KindSummary>)) {
    const r = recount[kind];
    if (!r || r.cases !== s.cases || r.equal !== s.equal) {
      throw new Error(`${runId}: ${kind} recount ${JSON.stringify(r)} != summary ${s.cases}/${s.equal}`);
    }
  }

  runs.push({
    id: runId,
    label: includePools ? 'Run 1: pools included' : 'Run 2: our side without pools (--no-pools)',
    includePools,
    capturedAt: iso(prov.captured_at_unix),
    horizon: prov.horizon,
    command: prov.command,
    bootstrapCheckpoint: prov.bootstrap_checkpoint,
    stateLedgerAtStart: prov.state_ledger_at_start,
    finalLedger: summary.final_ledger,
    firstCaseLedger: firstLedger,
    lastCaseLedger: lastLedger,
    horizonRequests: summary.horizon_requests,
    seconds: summary.seconds,
    kinds: summary.kinds,
    replayTotals: { state_checked: summary.replay_totals.state_checked, state_mismatch: summary.replay_totals.state_mismatch },
  });
}

// Bootstrap + replay statistics.
const runsRoot = join(fixtures, 'runs');
const runName = readdirSync(runsRoot).sort().at(-1);
if (!runName) throw new Error('fixtures/runs is empty; run scripts/export-run-stats.py');
const rs = readJson(join(runsRoot, runName, 'run-stats.json'));
const rsProv = readJson(join(runsRoot, runName, 'run.provenance.json'));
generatedFrom.push(`fixtures/runs/${runName}`);
const runStats: RunStats = {
  provenance: { exportedAt: rsProv.exported_at, script: rsProv.script, notes: rsProv.notes },
  state: rs.state,
  bootstrap: rs.bootstrap,
  replay: rs.replay,
};
if (runStats.replay.rows.length !== runStats.replay.ledgers) throw new Error('run-stats: row count != ledgers');

// Pool probe (optional).
let probe: PoolProbe | null = null;
const probesRoot = join(fixtures, 'probes');
if (existsSync(probesRoot)) {
  // The UI explains the 18/USDC probe (the pool behind with-pools case 001).
  // Other probes stay in fixtures/probes and are described in TASKS.md (SDX-012).
  const names = readdirSync(probesRoot).sort();
  const name = names.find((n) => n.endsWith('pool-18-usdc')) ?? names.at(-1);
  if (name) {
    const d = join(probesRoot, name);
    const pv = readJson(join(d, 'probe.provenance.json'));
    const pool = readJson(join(d, 'pool.json'))._embedded.records[0];
    const ss = readJson(join(d, 'strict-send.json'))._embedded.records as Json[];
    const book = readJson(join(d, 'order-book.json'));
    generatedFrom.push(`fixtures/probes/${name}`);
    probe = {
      dir: `fixtures/probes/${name}`,
      capturedAt: pv.captured_at,
      script: pv.script,
      poolId: pool.id,
      fee_bp: pool.fee_bp,
      reserves: pool.reserves,
      lastModifiedLedger: pool.last_modified_ledger,
      lastModifiedTime: pool.last_modified_time,
      poolLedger: Number(pv.requests.pool.latest_ledger),
      pathsLedger: Number(pv.requests['strict-send'].latest_ledger),
      sourceAmount: ss[0]?.source_amount ?? '',
      paths: ss.map((r) => ({ destinationAmount: r.destination_amount, path: (r.path as Json[]).map(horizonAssetLabel) })),
      bookLedger: Number(pv.requests['order-book'].latest_ledger),
      bookBids: book.bids.length,
      bookAsks: book.asks.length,
    };
  }
}

const sum = (xs: { equal: boolean }[]) => ({ cases: xs.length, equal: xs.filter((x) => x.equal).length });
const data: DemoData = {
  generatedFrom,
  runs,
  orderBooks,
  aggregations,
  paths,
  runStats,
  probe,
  totals: {
    orderBooks: sum(orderBooks),
    aggregations: sum(aggregations),
    pathsWithPools: sum(paths.filter((p) => p.includePools)),
    pathsNoPools: sum(paths.filter((p) => !p.includePools)),
  },
};

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, JSON.stringify(data) + '\n');
const t = data.totals;
console.log(
  `demo.json: order books ${t.orderBooks.equal}/${t.orderBooks.cases}, aggregations ${t.aggregations.equal}/${t.aggregations.cases}, ` +
    `paths ${t.pathsWithPools.equal}/${t.pathsWithPools.cases} (pools) ${t.pathsNoPools.equal}/${t.pathsNoPools.cases} (no pools), ` +
    `replay ${runStats.replay.ledgers} ledgers, probe ${probe ? 'yes' : 'no'}`,
);
