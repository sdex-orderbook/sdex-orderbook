// SPDX-License-Identifier: Apache-2.0
// How the state was built: history-archive bootstrap, Galexie replay, and the
// integrity checks, from fixtures/runs (exported from the run logs).
import { useState, type ReactNode } from 'react';
import { Archive, ArrowRight, Database, HardDrives, Scales } from '@phosphor-icons/react';
import type { RunStats } from '../lib/types';
import { bytes, day, duration, int, utc } from '../lib/format';
import { stepHover } from './common';
import { ChartFrame, DataTable, ProvenanceChip, RankedBars, StatusTable, truncateMiddle, type Provenance } from '../ui/kit';

const W = 960;
const H = 220;

/** "10curr" -> "10 curr" */
const lvl = (l: string | null) => (l ? l.replace(/^(\d+)/, '$1 ') : '?');
const PAD = { l: 52, r: 12, t: 10, b: 26 };

function ReplayChart({ rs }: { rs: RunStats }) {
  const rows = rs.replay.rows;
  const [hover, setHover] = useState<number | null>(null);
  const vals = rows.map((r) => r[3] ?? 0); // state_checked
  const max = Math.max(1, ...vals);
  const iw = W - PAD.l - PAD.r;
  const ih = H - PAD.t - PAD.b;
  const x = (i: number) => PAD.l + (iw * i) / Math.max(1, rows.length - 1);
  const y = (v: number) => PAD.t + ih * (1 - v / max);
  const line = vals.map((v, i) => `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join('');
  const area = `${line}L${x(rows.length - 1).toFixed(1)},${y(0)}L${x(0).toFixed(1)},${y(0)}Z`;
  const step = max > 600 ? 200 : max > 300 ? 100 : 50;
  const yt = Array.from({ length: Math.floor(max / step) + 1 }, (_, i) => i * step);
  const xt = [0, Math.floor(rows.length / 3), Math.floor((2 * rows.length) / 3), rows.length - 1];
  const h = hover !== null ? rows[hover] : undefined;
  return (
    <div className="relative">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        className="h-auto w-full rounded-xs focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--sik-viz-axis)]"
        role="img"
        tabIndex={0}
        onKeyDown={(e) => stepHover(e, rows.length, hover, setHover)}
        onBlur={() => setHover(null)}
        aria-label={`Offer and pool pre-images checked per ledger over ${rows.length} replayed ledgers; maximum ${max}. Focus the chart and use the arrow keys to read one ledger at a time, or use the table view for exact values.`}
        onPointerMove={(e) => {
          const r = (e.currentTarget as SVGSVGElement).getBoundingClientRect();
          const px = ((e.clientX - r.left) / r.width) * W;
          const i = Math.round(((px - PAD.l) / iw) * (rows.length - 1));
          setHover(i >= 0 && i < rows.length ? i : null);
        }}
        onPointerLeave={() => setHover(null)}
      >
        {yt.map((v) => (
          <g key={v}>
            <line x1={PAD.l} x2={W - PAD.r} y1={y(v)} y2={y(v)} stroke="var(--sik-viz-grid)" />
            <text x={PAD.l - 8} y={y(v)} dy="0.32em" textAnchor="end" fontSize={12} fill="var(--sik-viz-label)" className="tabular">
              {v}
            </text>
          </g>
        ))}
        <path d={area} fill="var(--sik-viz-1)" opacity={0.18} />
        <path d={line} fill="none" stroke="var(--sik-viz-1)" strokeWidth={1.25} strokeLinejoin="round" />
        <line x1={PAD.l} x2={W - PAD.r} y1={y(0)} y2={y(0)} stroke="var(--sik-viz-axis)" />
        {xt.map((i) => (
          <text key={i} x={x(i)} y={H - 8} textAnchor={i === 0 ? 'start' : i === rows.length - 1 ? 'end' : 'middle'} fontSize={12} fill="var(--sik-viz-label)" className="tabular">
            {int(rows[i]?.[0] ?? 0)}
          </text>
        ))}
        {hover !== null && (
          <>
            <line x1={x(hover)} x2={x(hover)} y1={PAD.t} y2={y(0)} stroke="var(--sik-viz-axis)" strokeDasharray="3 3" />
            <circle cx={x(hover)} cy={y(vals[hover] ?? 0)} r={4} fill="var(--sik-viz-1)" stroke="var(--sik-surface)" strokeWidth={2} />
          </>
        )}
      </svg>
      {h && hover !== null && (
        <div
          role="tooltip"
          className="pointer-events-none absolute top-1 z-[var(--sik-z-popover)] rounded-sm border border-line bg-raised px-3 py-2 text-xs shadow-2"
          style={hover < rows.length / 2 ? { left: `${(x(hover) / W) * 100 + 2}%` } : { right: `${100 - (x(hover) / W) * 100 + 2}%` }}
        >
          <p className="font-semibold text-ink tabular">Ledger {int(h[0] ?? 0)}</p>
          <p className="text-ink-3 tabular">{utc(h[1] ?? 0)}</p>
          <dl className="mt-1 grid grid-cols-[auto_auto] gap-x-3 tabular text-ink-2">
            <dt>Pre-images checked</dt>
            <dd className="text-right text-ink">{h[3]}</dd>
            <dt>Mismatches</dt>
            <dd className="text-right text-ink">{h[4]}</dd>
            <dt>Trades derived</dt>
            <dd className="text-right text-ink">{h[2]}</dd>
            <dt>Apply time</dt>
            <dd className="text-right text-ink">{h[8]} ms</dd>
          </dl>
        </div>
      )}
    </div>
  );
}

function Step({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <li className="flex min-w-0 flex-col gap-1.5 rounded-md border border-line bg-surface p-4">
      <span className="flex items-center gap-2 text-sm font-semibold text-ink">
        <span aria-hidden className="text-lg text-accent">
          {icon}
        </span>
        {title}
      </span>
      <span className="text-sm text-ink-2">{children}</span>
    </li>
  );
}

export function Pipeline({ rs }: { rs: RunStats }) {
  const b = rs.bootstrap;
  const r = rs.replay;
  const date = day(new Date(r.last_close_time * 1000).toISOString());
  const prov: Provenance = {
    kind: 'recorded',
    date,
    source: 'fixtures/runs/20261104-mainnet',
    note: `exported ${rs.provenance.exportedAt} by ${rs.provenance.script}`,
  };
  const hasOk = b.has_header_check?.match === true;
  return (
    <div className="flex flex-col gap-4">
      <ol className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
        <Step icon={<Archive weight="duotone" />} title="Bootstrap from buckets">
          History archive at checkpoint {int(b.checkpoint)}: {b.buckets.length} live buckets, {bytes(b.bytes_uncompressed)} read, {int(b.offers)} offers and{' '}
          {int(b.pools)} pools kept.
        </Step>
        <Step icon={<HardDrives weight="duotone" />} title="Replay the Galexie lake">
          {int(r.ledgers)} consecutive ledgers ({int(r.from_ledger)} to {int(r.to_ledger)}) applied; {int(r.trades)} trades derived from ClaimAtoms.
        </Step>
        <Step icon={<Database weight="duotone" />} title="Serve Horizon shapes">
          <code className="font-mono text-xs">sdx serve</code> answers /order_book, /trade_aggregations and /paths with Horizon request and response shapes.
        </Step>
        <Step icon={<Scales weight="duotone" />} title="Diff against Horizon">
          <code className="font-mono text-xs">sdx diff</code> records Horizon's body and Latest-Ledger, replays to that ledger, and compares.
        </Step>
      </ol>
      <p className="flex items-center gap-2 text-xs text-ink-3">
        <ArrowRight aria-hidden /> No Horizon, database or account is used to build the state. Horizon is read only by the diff step, as the reference.
      </p>

      <StatusTable
        caption="Integrity checks on the recorded run"
        meta={<ProvenanceChip {...prov} />}
        valueHeader="Measured"
        rows={[
          {
            id: 'has',
            name: 'Bucket list hash vs checkpoint ledger header',
            status: hasOk ? 'pass' : 'unknown',
            detail: b.has_header_check
              ? `SHA-256 over the HAS bucket hashes equals bucket_list_hash in the header of ledger ${int(b.has_header_check.ledger)} (protocol ${b.has_header_check.protocol}).`
              : 'Check output not exported.',
            value: b.has_header_check ? truncateMiddle(b.has_header_check.computed_from_has, 8, 6) : undefined,
          },
          {
            id: 'pre',
            name: 'Offer and pool STATE pre-images vs held state',
            status: r.state_mismatch === 0 ? 'pass' : 'fail',
            detail: `Every pre-image in ${int(r.ledgers)} ledgers compared with the entry the replayer held (full LedgerEntry equality).`,
            value: `${int(r.state_mismatch)} of ${int(r.state_checked)} differ`,
          },
          {
            id: 'trades',
            name: 'Trades with a missing maker pre-image',
            status: r.trade_errors === 0 ? 'pass' : 'warn',
            detail: 'A trade whose offer price cannot be recovered is dropped and counted.',
            value: `${int(r.trade_errors)} of ${int(r.trades)}`,
          },
          {
            id: 'hash',
            name: 'Per-bucket SHA-256 (full read)',
            status: 'skipped',
            statusLabel: 'Not run',
            detail: 'The bootstrap stops at the first Soroban entry, so bucket hashes are not recomputed; integrity rests on the two checks above. SDX-013.',
          },
        ]}
      />

      <div className="grid grid-cols-1 gap-3">
        <ChartFrame
          title="Pre-images checked per replayed ledger"
          subtitle={`Offer and pool STATE entries, ${int(r.ledgers)} ledgers. Mismatches: ${int(r.state_mismatch)} in every ledger.`}
          provenance={<ProvenanceChip {...prov} ledger={r.to_ledger} />}
          table={
            <DataTable
              dense
              maxHeight={360}
              rowKey={(row: number[]) => String(row[0])}
              rows={r.rows}
              columns={[
                { key: 'l', header: 'Ledger', cell: (row: number[]) => int(row[0] ?? 0), numeric: true, sortValue: (row: number[]) => row[0] ?? 0 },
                { key: 'c', header: 'Checked', cell: (row: number[]) => row[3], numeric: true, sortValue: (row: number[]) => row[3] ?? 0 },
                { key: 'm', header: 'Mismatch', cell: (row: number[]) => row[4], numeric: true },
                { key: 't', header: 'Trades', cell: (row: number[]) => row[2], numeric: true, sortValue: (row: number[]) => row[2] ?? 0 },
                { key: 'a', header: 'Apply ms', cell: (row: number[]) => row[8], numeric: true, sortValue: (row: number[]) => row[8] ?? 0, hideOnMobile: true },
              ]}
            />
          }
        >
          <ReplayChart rs={rs} />
        </ChartFrame>

        <ChartFrame
          title="Bytes read per bucket during bootstrap"
          subtitle={`Uncompressed XDR, ${b.buckets.length} buckets, ${bytes(b.bytes_uncompressed)} in total. Level 10 holds most accounts and trustlines.`}
          provenance={<ProvenanceChip {...prov} ledger={b.checkpoint} note={rs.provenance.notes[0]} />}
          table={
            <DataTable
              dense
              maxHeight={360}
              rowKey={(x) => x.hash}
              rows={b.buckets}
              columns={[
                { key: 'lv', header: 'Level', cell: (x) => (x.level ? lvl(x.level) : 'Unknown') },
                { key: 'by', header: 'Bytes', cell: (x) => bytes(x.bytes_uncompressed), numeric: true, sortValue: (x) => x.bytes_uncompressed },
                { key: 'of', header: 'Offers', cell: (x) => int(x.offers_live), numeric: true, sortValue: (x) => x.offers_live },
                { key: 'po', header: 'Pools', cell: (x) => int(x.pools_live), numeric: true, sortValue: (x) => x.pools_live },
                { key: 's', header: 'Seconds', cell: (x) => duration(x.seconds), numeric: true, sortValue: (x) => x.seconds, hideOnMobile: true },
              ]}
            />
          }
        >
          <RankedBars
            labelWidth="7.5rem"
            format={(n) => bytes(n)}
            bars={b.buckets
              .filter((x) => x.bytes_uncompressed > 0)
              .map((x) => ({ key: x.hash, label: `Level ${lvl(x.level)}`, value: x.bytes_uncompressed, text: `Bucket level ${x.level ?? 'unknown'}, ${x.hash}` }))}
          />
          <p className="mt-3 text-xs text-ink-3">
            Per-bucket times overlap (3 download workers) and some extracts came from the first attempt's cache, so they are not cold download times. The cold
            bootstrap took about 82 minutes over two attempts on a shared machine.
          </p>
        </ChartFrame>
      </div>
    </div>
  );
}
