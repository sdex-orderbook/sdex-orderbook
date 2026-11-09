// SPDX-License-Identifier: Apache-2.0
// The demo data must say exactly what the committed fixtures say. These tests
// re-read the fixtures independently of scripts/build-data.ts.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import type { DemoData } from '../src/lib/types.ts';
import { canonicalJson } from '../src/lib/canonical.ts';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, '..', '..', 'fixtures');
const demo = JSON.parse(readFileSync(join(here, '..', 'src', 'data', 'demo.json'), 'utf8')) as DemoData;

test('totals equal the summary.json of every committed diff run', () => {
  const sums: Record<'order_book' | 'trade_aggregations' | 'paths_pools' | 'paths_nopools', [number, number]> = {
    order_book: [0, 0],
    trade_aggregations: [0, 0],
    paths_pools: [0, 0],
    paths_nopools: [0, 0],
  };
  for (const run of readdirSync(join(fixtures, 'diff'))) {
    const s = JSON.parse(readFileSync(join(fixtures, 'diff', run, 'summary.json'), 'utf8'));
    const nopools = run.includes('nopools');
    for (const [k, v] of Object.entries(s.kinds) as [string, { cases: number; equal: number }][]) {
      const key = k === 'order_book' || k === 'trade_aggregations' ? k : nopools ? 'paths_nopools' : 'paths_pools';
      const cur = sums[key];
      sums[key] = [cur[0] + v.cases, cur[1] + v.equal];
    }
  }
  assert.deepEqual([demo.totals.orderBooks.cases, demo.totals.orderBooks.equal], sums.order_book);
  assert.deepEqual([demo.totals.aggregations.cases, demo.totals.aggregations.equal], sums.trade_aggregations);
  assert.deepEqual([demo.totals.pathsWithPools.cases, demo.totals.pathsWithPools.equal], sums.paths_pools);
  assert.deepEqual([demo.totals.pathsNoPools.cases, demo.totals.pathsNoPools.equal], sums.paths_nopools);
});

test('the recorded numbers the README quotes are what the data holds', () => {
  assert.deepEqual(demo.totals.orderBooks, { cases: 12, equal: 12 });
  assert.deepEqual(demo.totals.aggregations, { cases: 24, equal: 24 });
  assert.deepEqual(demo.totals.pathsWithPools, { cases: 48, equal: 5 });
  assert.deepEqual(demo.totals.pathsNoPools, { cases: 24, equal: 12 });
  assert.equal(demo.runStats.replay.ledgers, 1472);
  assert.equal(demo.runStats.replay.state_checked, 525948);
  assert.equal(demo.runStats.replay.state_mismatch, 0);
  assert.equal(demo.runStats.replay.trades, 159171);
  assert.equal(demo.runStats.bootstrap.has_header_check?.match, true);
});

test('hashes agree exactly when a case is recorded as equal', () => {
  for (const c of [...demo.orderBooks, ...demo.aggregations]) {
    assert.equal(c.sha256Ours === c.sha256Horizon, c.equal, `${c.run}/${c.index}`);
    assert.match(c.sha256Ours, /^[0-9a-f]{64}$/);
  }
});

test('every order book level shown equals the level Horizon returned when the case is equal', () => {
  for (const c of demo.orderBooks.filter((x) => x.equal)) {
    assert.deepEqual(c.ours, c.horizon, `${c.run}/${c.index}`);
  }
});

test('aggregation rows: per-bucket equality covers every bucket', () => {
  for (const c of demo.aggregations) {
    assert.equal(c.rowEqual.length, Math.max(c.ours.length, c.horizon.length));
    if (c.equal) assert.ok(c.rowEqual.every(Boolean));
  }
});

test('points in time: every path and order book case was answered at a single ledger', () => {
  for (const c of [...demo.orderBooks, ...demo.paths]) assert.ok(c.ledger > 65_000_000);
});

test('canonical JSON does not depend on key order', () => {
  const a = createHash('sha256').update(canonicalJson({ b: [1, { y: 2, x: 'z' }], a: null })).digest('hex');
  const b = createHash('sha256').update(canonicalJson({ a: null, b: [1, { x: 'z', y: 2 }] })).digest('hex');
  assert.equal(a, b);
  assert.equal(canonicalJson({ b: 1, a: 'x' }), '{"a":"x","b":1}');
});
