// SPDX-License-Identifier: Apache-2.0
// ProvenanceChip: says where a number came from. Every figure in a kit UI
// carries one (or sits in a panel whose header carries one). See DESIGN.md 8.2.
import type { ReactNode } from 'react';
import { Archive, Calculator, Database, Question, TestTube } from '@phosphor-icons/react';
import { cn, formatLedger } from './cn';

export type ProvenanceKind =
  | 'live' //      live read-only call made by this page, now
  | 'recorded' //  recorded mainnet capture (read-only, dated, ledger-pinned)
  | 'fixture' //   checked-in fixture with a *.provenance.json beside it
  | 'testnet' //   testnet run with throwaway keys
  | 'estimate' //  [E] estimate: not measured; the note says how it was derived
  | 'unknown'; //  provenance missing: treat the number as unverified

export interface Provenance {
  kind: ProvenanceKind;
  /** Ledger sequence the value is pinned to. */
  ledger?: number;
  /** ISO date (YYYY-MM-DD) or ISO timestamp, UTC. */
  date?: string;
  /** Short human source, e.g. "mainnet.sorobanrpc.com" or "fixtures/mainnet/harvest.json". */
  source?: string;
  /** Link to the provenance file, capture script or explorer. */
  href?: string;
  /** Extra detail shown in the tooltip (method, parameters, caveats). */
  note?: string;
}

const KIND: Record<ProvenanceKind, { label: string; icon: ReactNode; estimated?: boolean }> = {
  live: { label: 'Live read', icon: null },
  recorded: { label: 'Recorded mainnet capture', icon: <Archive weight="regular" aria-hidden /> },
  fixture: { label: 'Fixture', icon: <Database weight="regular" aria-hidden /> },
  testnet: { label: 'Testnet run', icon: <TestTube weight="regular" aria-hidden /> },
  estimate: { label: 'Estimate [E]', icon: <Calculator weight="regular" aria-hidden />, estimated: true },
  unknown: { label: 'Provenance unknown', icon: <Question weight="bold" aria-hidden />, estimated: true },
};

function displayDate(date: string): string {
  // Timestamps render as "2026-11-04 01:00 UTC"; plain dates stay as given.
  if (date.length <= 10) return date;
  const d = new Date(date);
  if (Number.isNaN(d.getTime())) return date;
  return `${d.toISOString().slice(0, 10)} ${d.toISOString().slice(11, 16)} UTC`;
}

export interface ProvenanceChipProps extends Provenance {
  size?: 'sm' | 'md';
  className?: string;
}

export function ProvenanceChip({ kind, ledger, date, source, href, note, size = 'sm', className }: ProvenanceChipProps) {
  const k = KIND[kind];
  const parts: string[] = [k.label];
  if (ledger !== undefined) parts.push(`ledger ${formatLedger(ledger)}`);
  if (date) parts.push(displayDate(date));
  if (source && ledger === undefined) parts.push(source);
  const full = [...parts, source && ledger !== undefined ? source : undefined, note].filter(Boolean).join(', ');

  // Segments wrap as whole units in narrow containers: a ledger number or a
  // date is never cut or ellipsised (an honesty rule, DESIGN.md 8.2).
  const body = (
    <>
      {kind === 'live' ? (
        <span className="relative mt-[5px] inline-flex size-2 shrink-0 self-start" aria-hidden>
          <span className="animate-live absolute inset-0 rounded-full bg-good" />
          <span className="relative inline-flex size-2 rounded-full bg-good" />
        </span>
      ) : (
        <span
          className={cn(
            'mt-[2px] shrink-0 self-start text-[1.1em] leading-none',
            kind === 'unknown' ? 'text-serious-ink' : 'text-ink-3',
          )}
        >
          {k.icon}
        </span>
      )}
      <span className="flex min-w-0 flex-wrap items-baseline gap-x-1.5">
        {parts.map((p, i) => (
          <span key={p} className={cn(i > 0 && 'whitespace-nowrap')}>
            <span className={i === 0 ? 'font-medium text-ink' : undefined}>{p}</span>
            {i < parts.length - 1 && (
              <span className="ml-1.5 text-ink-3" aria-hidden>
                ·
              </span>
            )}
          </span>
        ))}
      </span>
    </>
  );

  const cls = cn(
    'inline-flex max-w-full items-start gap-1.5 rounded-xs border bg-surface text-left text-ink-2 tabular',
    'transition-colors duration-[120ms] ease-settle',
    k.estimated ? 'border-dashed border-line-strong' : 'border-line',
    size === 'sm' ? 'min-h-6 px-2 py-[3px] text-xs' : 'min-h-7 px-2.5 py-1 text-sm',
    href && 'hover:border-line-strong hover:bg-sunken',
    className,
  );

  return href ? (
    <a href={href} className={cls} title={full} aria-label={`Provenance: ${full}`}>
      {body}
    </a>
  ) : (
    <span className={cls} title={full} role="note" aria-label={`Provenance: ${full}`}>
      {body}
    </span>
  );
}
