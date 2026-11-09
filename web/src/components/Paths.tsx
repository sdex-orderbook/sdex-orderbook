// SPDX-License-Identifier: Apache-2.0
// Path finding: the honest panel. Every recorded case, Horizon's records next
// to ours, and the known mismatch class. Nothing here is presented as working.
import { useMemo, useState } from 'react';
import { ArrowRight } from '@phosphor-icons/react';
import type { DiffRun, PathCase, PathRecord, PoolProbe } from '../lib/types';
import { assetLabel } from '../lib/assets';
import { day } from '../lib/format';
import { GateNotice, ProvenanceChip, StatusLabel, cn, formatLedger, truncateMiddle } from '../ui/kit';
import { Segmented } from './common';

function Records({ title, recs, kind }: { title: string; recs: PathRecord[]; kind: PathCase['kind'] }) {
  const send = kind === 'strict_send';
  return (
    <div className="min-w-0">
      <h4 className="text-xs font-medium text-ink-2">{title}</h4>
      {recs.length === 0 ? (
        <p className="mt-2 text-sm text-ink-3">No records returned.</p>
      ) : (
        <ol className="mt-2 flex flex-col gap-1.5">
          {recs.map((r, i) => (
            <li key={i} className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
              <span className="w-[8.5rem] shrink-0 text-right font-medium tabular text-ink">
                {send ? r.destinationAmount : r.sourceAmount} {send ? r.destination : r.source}
              </span>
              <span className="flex flex-wrap items-center gap-1 text-xs text-ink-2">
                <span>{r.source}</span>
                {r.path.map((p, j) => (
                  <span key={j} className="inline-flex items-center gap-1">
                    <ArrowRight aria-hidden className="text-ink-3" />
                    <span className="rounded-xs border border-line bg-sunken px-1.5 font-mono">{p}</span>
                  </span>
                ))}
                <ArrowRight aria-hidden className="text-ink-3" />
                <span>{r.destination}</span>
                {r.path.length === 0 && <span className="text-ink-3">(direct)</span>}
              </span>
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}

function Probe({ p, origin }: { p: PoolProbe; origin: PathCase | undefined }) {
  const direct = p.paths.find((x) => x.path.length === 0);
  const ours = origin?.ours[0];
  const theirs = origin?.horizon[0];
  return (
    <div className="rounded-md border border-line bg-surface p-4 sm:p-5">
      <h3 className="text-sm font-semibold text-ink">Follow-up probe of the pool behind case 001 (2026-11-08)</h3>
      <p className="mt-2 max-w-prose text-sm text-ink-2">
        {origin && ours && theirs ? (
          <>
            In case {String(origin.index).padStart(3, '0')} our engine sends {ours.sourceAmount} {ours.source} through {ours.path.join(', then ')}, then the
            18/USDC classic pool, to get {ours.destinationAmount} {ours.destination}; Horizon's best at the same ledger was {theirs.destinationAmount}{' '}
            {theirs.destination}.{' '}
          </>
        ) : null}
        Four days later the pool had changed (last modified at ledger {formatLedger(p.lastModifiedLedger)}, {p.lastModifiedTime.replace('T', ' ').replace('Z', ' UTC')}),
        and Horizon now routes through it:
      </p>
      <dl className="mt-3 grid grid-cols-1 gap-x-6 gap-y-2 text-sm sm:grid-cols-2">
        <div>
          <dt className="text-xs text-ink-3">Pool {truncateMiddle(p.poolId, 8, 6)} reserves (fee {p.fee_bp} bps)</dt>
          <dd className="tabular text-ink">
            {p.reserves.map((r) => `${r.amount} ${assetLabel(r.asset)}`).join(' and ')}
          </dd>
        </div>
        <div>
          <dt className="text-xs text-ink-3">Horizon strict-send {p.sourceAmount} 18 to USDC, direct record</dt>
          <dd className="tabular text-ink">{direct ? `${direct.destinationAmount} USDC` : 'no direct record'}</dd>
        </div>
      </dl>
      <p className="mt-3 max-w-prose text-sm text-ink-2">
        Our CAP-38 pool maths gives the same {direct?.destinationAmount ?? 'amount'} USDC for those reserves (unit test{' '}
        <code className="font-mono text-xs">payout_matches_horizon_probe_2026_11_08</code>). So Horizon does use this pool today, and the 2026-11-04 difference
        is not a blanket filter on small pools. The cause is still open (SDX-012).
      </p>
      <div className="mt-3">
        <ProvenanceChip kind="recorded" ledger={p.pathsLedger} date={day(p.capturedAt)} source={p.dir} note={p.script} />
      </div>
    </div>
  );
}

export function Paths({ runs, cases, probe }: { runs: DiffRun[]; cases: PathCase[]; probe: PoolProbe | null }) {
  const [runId, setRunId] = useState(() => runs.find((r) => r.includePools)?.id ?? runs[0]?.id ?? '');
  const run = runs.find((r) => r.id === runId);
  const list = useMemo(() => cases.filter((c) => c.run === runId), [cases, runId]);
  const [sel, setSel] = useState<number | null>(null);
  const c = list.find((x) => x.index === sel) ?? list.find((x) => !x.equal) ?? list[0];
  const eq = list.filter((x) => x.equal).length;
  const best = list.filter((x) => x.bestEqual).length;

  return (
    <div className="flex flex-col gap-4">
      <GateNotice tone="warning" title="Path finding does not match Horizon yet. Do not rely on /paths output.">
        <p>
          The engine is a port of go-stellar-sdk <code className="font-mono">exp/orderbook</code>. With pools, our results often use thin classic pools that
          horizon.stellar.org did not use at the same ledger and report far larger amounts. Run 2 switched pools off on our side only (Horizon always uses
          them), so its matches are partial evidence, not parity. Cause not found yet; ticket SDX-012.
        </p>
      </GateNotice>

      <div className="flex flex-wrap items-center justify-between gap-3">
        <Segmented
          label="Diff run"
          value={runId}
          onChange={(v) => {
            setRunId(v);
            setSel(null);
          }}
          options={runs.map((r) => ({ value: r.id, label: r.includePools ? 'Run 1: pools on' : 'Run 2: pools off (ours)' }))}
        />
        <p className="text-sm text-ink-2 tabular">
          {eq} of {list.length} identical, best amount equal in {best} of {list.length}
        </p>
      </div>

      <div className="grid grid-cols-1 gap-3 2xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
        <div className="max-h-[360px] min-w-0 overflow-auto rounded-md border border-line bg-surface">
          <table className="w-full min-w-[36rem] text-sm">
            <caption className="sr-only">Path cases of the selected run</caption>
            <thead className="sticky top-0 bg-sunken text-xs text-ink-2">
              <tr>
                <th scope="col" className="px-3 py-1.5 text-left font-medium">Request</th>
                <th scope="col" className="px-3 py-1.5 text-right font-medium">Horizon best</th>
                <th scope="col" className="px-3 py-1.5 text-right font-medium">Our best</th>
                <th scope="col" className="px-3 py-1.5 text-left font-medium">Status</th>
              </tr>
            </thead>
            <tbody>
              {list.map((x) => {
                const on = c?.index === x.index;
                return (
                  <tr
                    key={x.index}
                    className={cn('cursor-pointer border-b border-line last:border-b-0', on ? 'bg-accent-wash' : 'hover:bg-sunken')}
                    onClick={() => setSel(x.index)}
                  >
                    <td className="px-3 py-1.5 whitespace-nowrap">
                      <button
                        type="button"
                        className="text-left text-ink"
                        aria-pressed={on}
                        onClick={(e) => {
                          e.stopPropagation();
                          setSel(x.index);
                        }}
                      >
                        <span className="text-ink-3">{x.kind === 'strict_send' ? 'Send' : 'Receive'}</span>{' '}
                        <span className="tabular">{x.amount}</span> {x.kind === 'strict_send' ? assetLabel(x.from) : assetLabel(x.to)}
                        <span className="text-ink-3"> {x.kind === 'strict_send' ? 'to' : 'from'} </span>
                        {x.kind === 'strict_send' ? assetLabel(x.to) : assetLabel(x.from)}
                      </button>
                    </td>
                    <td className="px-3 py-1.5 text-right tabular text-ink-2">{x.horizonBest ?? 'none'}</td>
                    <td className="px-3 py-1.5 text-right tabular text-ink-2">{x.ourBest ?? 'none'}</td>
                    <td className="px-3 py-1.5">
                      <StatusLabel status={x.equal ? 'pass' : 'fail'} label={x.equal ? 'Same' : 'Differs'} />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        {c && (
          <div className="flex min-w-0 flex-col gap-4 rounded-md border border-line bg-surface p-4 sm:p-5">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h3 className="text-sm font-semibold text-ink">
                Case {String(c.index).padStart(3, '0')}: {c.kind === 'strict_send' ? 'strict-send' : 'strict-receive'} {c.amount}{' '}
                {c.kind === 'strict_send' ? assetLabel(c.from) : assetLabel(c.to)}
              </h3>
              <StatusLabel status={c.equal ? 'pass' : 'fail'} label={c.equal ? 'Identical' : 'Differs'} />
            </div>
            <Records title="horizon.stellar.org (pools on)" recs={c.horizon} kind={c.kind} />
            <Records title={`sdx (${c.includePools ? 'pools on' : 'pools off'})`} recs={c.ours} kind={c.kind} />
            <ProvenanceChip kind="recorded" ledger={c.ledger} date={run ? day(run.capturedAt) : undefined} source={`fixtures/diff/${c.run}`} />
          </div>
        )}
      </div>

      {probe && <Probe p={probe} origin={cases.find((x) => x.includePools && x.ours[0]?.path.includes('18'))} />}
    </div>
  );
}
