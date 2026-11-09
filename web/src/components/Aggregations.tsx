// SPDX-License-Identifier: Apache-2.0
// Trade aggregations: OHLC candles drawn from the buckets sdx computed from
// trades it derived out of ledger meta, with Horizon's bucket beside each one
// in the table view and the equality result per bucket.
import { useMemo, useState } from 'react';
import type { AggRow, AggregationCase } from '../lib/types';
import { assetLabel } from '../lib/assets';
import { day, hhmm, resolutionLabel, utc } from '../lib/format';
import { ChartFrame, ProvenanceChip, formatLedger } from '../ui/kit';
import { DiffBadge, HashLine, Picker, RowMatch, stepHover } from './common';

const W = 720;
const H = 260;
const PAD = { l: 64, r: 12, t: 12, b: 28 };

function ticks(lo: number, hi: number, n = 4): number[] {
  const span = hi - lo || Math.abs(hi) || 1;
  const step0 = span / n;
  const mag = 10 ** Math.floor(Math.log10(step0));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= step0) ?? step0;
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + 1e-12; v += step) out.push(Number(v.toPrecision(12)));
  return out;
}

function Candles({ rows, rowEqual, base, counter }: { rows: AggRow[]; rowEqual: boolean[]; base: string; counter: string }) {
  const [hover, setHover] = useState<number | null>(null);
  const lows = rows.map((r) => Number(r[6]));
  const highs = rows.map((r) => Number(r[5]));
  const lo = Math.min(...lows);
  const hi = Math.max(...highs);
  const pad = (hi - lo) * 0.06 || hi * 0.01 || 1;
  const y0 = lo - pad;
  const y1 = hi + pad;
  const iw = W - PAD.l - PAD.r;
  const ih = H - PAD.t - PAD.b;
  const x = (i: number) => PAD.l + (iw * (i + 0.5)) / rows.length;
  const y = (v: number) => PAD.t + ih * (1 - (v - y0) / (y1 - y0));
  const bw = Math.max(1, Math.min(14, (iw / rows.length) * 0.62));
  const yt = ticks(y0, y1);
  const xEvery = Math.max(1, Math.ceil(rows.length / 6));
  const h = hover !== null ? rows[hover] : undefined;

  return (
    <div className="relative">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={`Candles of ${rows.length} buckets, price in ${assetLabel(counter)} per ${assetLabel(base)}. Focus the chart and use the arrow keys to read one bucket at a time, or use the table view for exact values.`}
        onPointerLeave={() => setHover(null)}
        tabIndex={0}
        onKeyDown={(e) => stepHover(e, rows.length, hover, setHover)}
        onBlur={() => setHover(null)}
        className="h-auto w-full rounded-xs focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--sik-viz-axis)]"
      >
        {yt.map((v) => (
          <g key={v}>
            <line x1={PAD.l} x2={W - PAD.r} y1={y(v)} y2={y(v)} stroke="var(--sik-viz-grid)" strokeWidth={1} />
            <text x={PAD.l - 8} y={y(v)} dy="0.32em" textAnchor="end" fontSize={12} fill="var(--sik-viz-label)" className="tabular">
              {v.toPrecision(6).replace(/\.?0+$/, '')}
            </text>
          </g>
        ))}
        <line x1={PAD.l} x2={W - PAD.r} y1={H - PAD.b} y2={H - PAD.b} stroke="var(--sik-viz-axis)" strokeWidth={1} />
        {rows.map((r, i) =>
          i % xEvery === 0 ? (
            <text key={`t${r[0]}`} x={x(i)} y={H - PAD.b + 18} textAnchor="middle" fontSize={12} fill="var(--sik-viz-label)" className="tabular">
              {hhmm(Number(r[0]))}
            </text>
          ) : null,
        )}
        {rows.map((r, i) => {
          const o = Number(r[7]);
          const c = Number(r[8]);
          const up = c >= o;
          const color = up ? 'var(--sik-viz-1)' : 'var(--sik-viz-2)';
          const top = y(Math.max(o, c));
          const bh = Math.max(1, Math.abs(y(o) - y(c)));
          return (
            <g key={r[0]} opacity={hover === null || hover === i ? 1 : 0.45}>
              <line x1={x(i)} x2={x(i)} y1={y(Number(r[5]))} y2={y(Number(r[6]))} stroke={color} strokeWidth={1.5} />
              <rect x={x(i) - bw / 2} y={top} width={bw} height={bh} rx={1} fill={up ? color : 'var(--sik-surface)'} stroke={color} strokeWidth={1.5} />
              {!rowEqual[i] && <circle cx={x(i)} cy={PAD.t + 4} r={4} fill="var(--sik-critical)" />}
            </g>
          );
        })}
        {/* Hit targets wider than the marks. */}
        {rows.map((r, i) => (
          <rect
            key={`h${r[0]}`}
            x={PAD.l + (iw * i) / rows.length}
            y={PAD.t}
            width={iw / rows.length}
            height={ih}
            fill="transparent"
            onPointerEnter={() => setHover(i)}
          />
        ))}
        {hover !== null && <line x1={x(hover)} x2={x(hover)} y1={PAD.t} y2={H - PAD.b} stroke="var(--sik-viz-axis)" strokeDasharray="3 3" />}
      </svg>
      {h && hover !== null && (
        <div
          role="tooltip"
          className="pointer-events-none absolute top-2 z-[var(--sik-z-popover)] rounded-sm border border-line bg-raised px-3 py-2 text-xs shadow-2"
          style={hover < rows.length / 2 ? { left: `${(x(hover) / W) * 100 + 2}%` } : { right: `${100 - (x(hover) / W) * 100 + 2}%` }}
        >
          <p className="font-semibold text-ink tabular">{utc(Number(h[0]), 'ms')}</p>
          <dl className="mt-1 grid grid-cols-[auto_auto] gap-x-3 gap-y-0.5 tabular text-ink-2">
            <dt>Open</dt>
            <dd className="text-right text-ink">{h[7]}</dd>
            <dt>High</dt>
            <dd className="text-right text-ink">{h[5]}</dd>
            <dt>Low</dt>
            <dd className="text-right text-ink">{h[6]}</dd>
            <dt>Close</dt>
            <dd className="text-right text-ink">{h[8]}</dd>
            <dt>Trades</dt>
            <dd className="text-right text-ink">{h[1]}</dd>
            <dt>Base volume</dt>
            <dd className="text-right text-ink">{h[2]}</dd>
          </dl>
          <p className="mt-1.5">
            <RowMatch equal={rowEqual[hover] ?? false} /> <span className="text-ink-3">vs Horizon bucket</span>
          </p>
        </div>
      )}
    </div>
  );
}

function BucketTable({ c }: { c: AggregationCase }) {
  const n = Math.max(c.ours.length, c.horizon.length);
  const idx = Array.from({ length: n }, (_, i) => i);
  return (
    <div className="max-h-[420px] overflow-auto rounded-sm border border-line">
      <table className="w-full min-w-[40rem] text-sm">
        <caption className="sr-only">Buckets: ours and Horizon, with equality per bucket</caption>
        <thead className="sticky top-0 bg-sunken text-xs text-ink-2">
          <tr>
            {['Bucket start (UTC)', 'Source', 'Trades', 'Open', 'High', 'Low', 'Close', 'Base volume', 'Match'].map((t) => (
              <th key={t} scope="col" className="px-3 py-1.5 text-right font-medium first:text-left last:text-left">
                {t}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {idx.map((i) =>
            (
              [
                ['sdx', c.ours[i]],
                ['Horizon', c.horizon[i]],
              ] as const
            ).map(([src, r], j) => (
              <tr key={`${i}-${src}`} className={j === 1 ? 'border-b border-line' : undefined}>
                <td className="px-3 py-1 text-left tabular text-ink-2">{j === 0 && r ? utc(Number(r[0]), 'ms').slice(11) : ''}</td>
                <td className="px-3 py-1 text-right text-ink-3">{src}</td>
                {r ? (
                  [r[1], r[7], r[5], r[6], r[8], r[2]].map((v, k) => (
                    <td key={k} className="px-3 py-1 text-right tabular text-ink">
                      {v}
                    </td>
                  ))
                ) : (
                  <td colSpan={6} className="px-3 py-1 text-right text-critical-ink">
                    Missing bucket
                  </td>
                )}
                <td className="px-3 py-1 text-left">{j === 0 ? <RowMatch equal={c.rowEqual[i] ?? false} /> : null}</td>
              </tr>
            )),
          )}
        </tbody>
      </table>
    </div>
  );
}

export function Aggregations({ cases }: { cases: AggregationCase[] }) {
  const [key, setKey] = useState(() => `${cases[0]?.run}/${cases[0]?.index}`);
  const c = useMemo(() => cases.find((x) => `${x.run}/${x.index}` === key) ?? cases[0], [cases, key]);
  if (!c) return null;
  const opts = cases.map((x) => ({
    value: `${x.run}/${x.index}`,
    label: `${assetLabel(x.base)} / ${assetLabel(x.counter)}, ${resolutionLabel(x.resolution)} (${x.ourBuckets} buckets)`,
  }));
  const equalBuckets = c.rowEqual.filter(Boolean).length;
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <Picker label="Recorded case (base / counter, resolution)" value={key} options={opts} onChange={setKey} />
        <DiffBadge equal={c.equal} what={`${equalBuckets} of ${Math.max(c.ourBuckets, c.horizonBuckets)} buckets identical`} />
      </div>
      <ChartFrame
        title={`${assetLabel(c.base)} / ${assetLabel(c.counter)} candles, ${resolutionLabel(c.resolution)}`}
        subtitle={`Price in ${assetLabel(c.counter)} per ${assetLabel(c.base)}, ${utc(c.windowMs[0], 'ms')} to ${hhmm(c.windowMs[1])} UTC. Filled: close at or above open. A red dot would mark a bucket that differs from Horizon.`}
        provenance={
          <div className="flex flex-col gap-2">
            <ProvenanceChip
              kind="recorded"
              ledger={c.ledger}
              date={day(c.capturedAt)}
              source={`fixtures/diff/${c.run}`}
              note={`sdx buckets from trades of ledgers it replayed; Horizon queried for the same closed window. Computed at state ledger ${formatLedger(c.ledger)}.`}
            />
            <HashLine label="SHA-256 sdx records" value={c.sha256Ours} />
            <HashLine label="SHA-256 Horizon records" value={c.sha256Horizon} />
          </div>
        }
        table={<BucketTable c={c} />}
      >
        <Candles rows={c.ours} rowEqual={c.rowEqual} base={c.base} counter={c.counter} />
      </ChartFrame>
    </div>
  );
}
