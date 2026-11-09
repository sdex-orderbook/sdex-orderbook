// SPDX-License-Identifier: Apache-2.0
// Writes src/data/facts.json from committed fixtures (no network): the diff
// summaries and the exported bootstrap/replay statistics. Docs pages quote
// numbers only from this file.
import { readFileSync, readdirSync, writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, '..', '..', 'fixtures');
const read = (p) => JSON.parse(readFileSync(p, 'utf8'));

const runs = readdirSync(join(fixtures, 'diff')).sort().map((id) => {
  const s = read(join(fixtures, 'diff', id, 'summary.json'));
  const p = read(join(fixtures, 'diff', id, 'diff.provenance.json'));
  return {
    id,
    includePools: !String(p.command).includes('--no-pools'),
    date: new Date(p.captured_at_unix * 1000).toISOString().slice(0, 10),
    finalLedger: s.final_ledger,
    horizonRequests: s.horizon_requests,
    kinds: Object.fromEntries(Object.entries(s.kinds).map(([k, v]) => [k, { cases: v.cases, equal: v.equal, best_equal: v.best_equal }])),
  };
});
const total = (kind, pred = () => true) =>
  runs.filter(pred).reduce((a, r) => ({ cases: a.cases + (r.kinds[kind]?.cases ?? 0), equal: a.equal + (r.kinds[kind]?.equal ?? 0) }), { cases: 0, equal: 0 });
const pathTotal = (pools) => {
  const a = total('strict_send', (r) => r.includePools === pools);
  const b = total('strict_receive', (r) => r.includePools === pools);
  return { cases: a.cases + b.cases, equal: a.equal + b.equal };
};

const runName = readdirSync(join(fixtures, 'runs')).sort().at(-1);
const rs = read(join(fixtures, 'runs', runName, 'run-stats.json'));
const facts = {
  runs,
  orderBooks: total('order_book'),
  aggregations: total('trade_aggregations'),
  pathsWithPools: pathTotal(true),
  pathsNoPools: pathTotal(false),
  state: rs.state,
  stateDate: new Date(rs.state.close_time * 1000).toISOString().slice(0, 10),
  bootstrap: {
    checkpoint: rs.bootstrap.checkpoint,
    offers: rs.bootstrap.offers,
    pools: rs.bootstrap.pools,
    assets: rs.bootstrap.assets,
    buckets: rs.bootstrap.buckets.length,
    gb: Number((rs.bootstrap.bytes_uncompressed / 1e9).toFixed(2)),
    peakRssMiB: Math.round(rs.bootstrap.peak_rss_kib / 1024),
    hasMatch: rs.bootstrap.has_header_check?.match === true,
  },
  replay: {
    from: rs.replay.from_ledger,
    to: rs.replay.to_ledger,
    ledgers: rs.replay.ledgers,
    trades: rs.replay.trades,
    checked: rs.replay.state_checked,
    mismatch: rs.replay.state_mismatch,
    medianMs: rs.replay.apply_ms_median,
  },
};
mkdirSync(join(here, '..', 'src', 'data'), { recursive: true });
writeFileSync(join(here, '..', 'src', 'data', 'facts.json'), JSON.stringify(facts, null, 2) + '\n');
console.log(`facts.json: order books ${facts.orderBooks.equal}/${facts.orderBooks.cases}, paths ${facts.pathsWithPools.equal}/${facts.pathsWithPools.cases}`);
