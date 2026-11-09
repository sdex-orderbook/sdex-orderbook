// SPDX-License-Identifier: Apache-2.0
// StatusTable: a list of checks, probes, scenarios or gates, each with a
// status that is always icon + word (never colour alone), plus a summary line.
// DESIGN.md 8.5.
import type { ReactNode } from 'react';
import { CheckCircle, Hourglass, MinusCircle, Question, Warning, XCircle } from '@phosphor-icons/react';
import { cn } from './cn';
import type { Tone } from './GateBadge';
import { ProvenanceChip, type Provenance } from './ProvenanceChip';

export type Status = 'pass' | 'warn' | 'fail' | 'unknown' | 'skipped' | 'running';

const STATUS: Record<Status, { word: string; tone: Tone; icon: ReactNode }> = {
  pass: { word: 'Pass', tone: 'good', icon: <CheckCircle weight="fill" /> },
  warn: { word: 'Warn', tone: 'warning', icon: <Warning weight="fill" /> },
  fail: { word: 'Fail', tone: 'critical', icon: <XCircle weight="fill" /> },
  unknown: { word: 'Unknown', tone: 'neutral', icon: <Question weight="bold" /> },
  skipped: { word: 'Skipped', tone: 'neutral', icon: <MinusCircle weight="bold" /> },
  running: { word: 'Running', tone: 'neutral', icon: <Hourglass weight="bold" /> },
};

const MARK: Record<Tone, string> = {
  good: 'text-good',
  warning: 'text-warning',
  serious: 'text-serious',
  critical: 'text-critical',
  neutral: 'text-ink-3',
};
const WORD: Record<Tone, string> = {
  good: 'text-good-ink',
  warning: 'text-warning-ink',
  serious: 'text-serious-ink',
  critical: 'text-critical-ink',
  neutral: 'text-ink-2',
};

/** Icon + word. The icon carries the status mark colour; the word stays readable text colour. */
export function StatusLabel({ status, label }: { status: Status; label?: string }) {
  const s = STATUS[status];
  return (
    <span className="inline-flex items-center gap-1.5 whitespace-nowrap font-medium">
      <span aria-hidden className={cn('text-[15px] leading-none', MARK[s.tone])}>
        {s.icon}
      </span>
      <span className={WORD[s.tone]}>{label ?? s.word}</span>
    </span>
  );
}

export interface StatusRow {
  id: string;
  name: ReactNode;
  status: Status;
  /** Optional override for the status word, e.g. "Lag 0" or "OK". */
  statusLabel?: string;
  /** One-line explanation (why it passed or failed). */
  detail?: ReactNode;
  /** The measured value, right-aligned. */
  value?: ReactNode;
  provenance?: Provenance;
}

export interface StatusTableProps {
  rows: StatusRow[];
  caption?: ReactNode;
  meta?: ReactNode;
  /** Header for the value column; omit to hide the column. */
  valueHeader?: string;
  className?: string;
}

export function StatusTable({ rows, caption, meta, valueHeader, className }: StatusTableProps) {
  const counts = rows.reduce<Partial<Record<Status, number>>>((acc, r) => {
    acc[r.status] = (acc[r.status] ?? 0) + 1;
    return acc;
  }, {});
  const order: Status[] = ['fail', 'warn', 'pass', 'unknown', 'skipped', 'running'];
  const summary = order
    .filter((s) => counts[s])
    .map((s) => `${counts[s]} ${STATUS[s].word.toLowerCase()}`)
    .join(', ');

  return (
    <div className={cn('min-w-0 overflow-hidden rounded-md border border-line bg-surface', className)}>
      <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b border-line px-4 py-3">
        <div className="min-w-0">
          {caption && <h3 className="text-sm font-semibold text-ink">{caption}</h3>}
          <p className="text-xs text-ink-3 tabular" aria-live="polite">
            {rows.length} {rows.length === 1 ? 'check' : 'checks'}: {summary || 'none run'}
          </p>
        </div>
        {meta && <div className="flex flex-wrap items-center gap-2">{meta}</div>}
      </div>
      <ul role="list">
        {rows.map((r) => (
          <li
            key={r.id}
            className={cn(
              'grid grid-cols-1 gap-x-4 gap-y-1 border-b border-line px-4 py-3 last:border-b-0',
              valueHeader ? 'md:grid-cols-[7.5rem_minmax(0,1fr)_auto]' : 'md:grid-cols-[7.5rem_minmax(0,1fr)]',
            )}
          >
            <div className="text-sm">
              <StatusLabel status={r.status} label={r.statusLabel} />
            </div>
            <div className="min-w-0">
              <p className="text-sm font-medium text-ink">{r.name}</p>
              {r.detail && <p className="mt-0.5 text-sm text-ink-2">{r.detail}</p>}
              {r.provenance && (
                <div className="mt-2">
                  <ProvenanceChip {...r.provenance} />
                </div>
              )}
            </div>
            {valueHeader && (
              <div className="text-sm tabular text-ink md:text-right">
                <span className="mr-2 text-xs text-ink-3 md:hidden">{valueHeader}</span>
                {r.value}
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
