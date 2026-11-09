// SPDX-License-Identifier: Apache-2.0
// Small shared pieces for the demo views: section heading, segmented picker,
// equality badge and a hash line. Status is always icon + word (DESIGN.md 1.5).
import type { KeyboardEvent, ReactNode } from 'react';
import { CheckCircle, XCircle } from '@phosphor-icons/react';
import { cn, formatLedger, truncateMiddle } from '../ui/kit';

export function Section({
  id,
  title,
  lede,
  meta,
  children,
}: {
  id: string;
  title: string;
  lede?: ReactNode;
  meta?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section id={id} aria-labelledby={`${id}-h`} className="mt-10 border-t border-line pt-8 first:mt-0 first:border-t-0 first:pt-0">
      <h2 id={`${id}-h`} className="text-xl font-semibold tracking-tight text-balance text-ink">
        {title}
      </h2>
      {lede && <p className="mt-2 max-w-prose text-base text-pretty text-ink-2">{lede}</p>}
      {meta && <div className="mt-3 flex flex-wrap items-center gap-2">{meta}</div>}
      <div className="mt-6">{children}</div>
    </section>
  );
}

export interface Option<T extends string> {
  value: T;
  label: ReactNode;
  title?: string;
}

/** Segmented control for a handful of options; wraps on narrow screens. */
export function Segmented<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: Option<T>[];
  onChange: (v: T) => void;
}) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex max-w-full flex-wrap gap-0.5 rounded-sm border border-line bg-sunken p-0.5">
      {options.map((o) => {
        const on = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={on}
            title={o.title}
            onClick={() => onChange(o.value)}
            className={cn(
              'inline-flex h-7 items-center gap-1.5 rounded-[4px] px-2.5 text-sm whitespace-nowrap transition-colors duration-[120ms] ease-settle',
              on ? 'bg-surface font-medium text-ink shadow-1' : 'text-ink-2 hover:text-ink',
            )}
          >
            {o.label}
          </button>
        );
      })}
    </div>
  );
}

/** Native select styled to the kit, for longer option lists. */
export function Picker<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
}) {
  return (
    <label className="flex flex-col gap-1.5 text-xs text-ink-2">
      {label}
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as T)}
        className="h-8 max-w-full rounded-sm border border-line-strong bg-surface px-2.5 text-sm text-ink"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}

/** "Identical at ledger N" or "Differs at ledger N". */
export function DiffBadge({ equal, ledger, what = 'Identical' }: { equal: boolean; ledger?: number; what?: string }) {
  return (
    <span
      className={cn(
        'inline-flex min-h-7 items-center gap-1.5 rounded-xs border px-2.5 text-sm font-medium',
        equal ? 'border-good-ring bg-good-wash text-good-ink' : 'border-critical-ring bg-critical-wash text-critical-ink',
      )}
    >
      <span aria-hidden className={cn('text-base leading-none', equal ? 'text-good' : 'text-critical')}>
        {equal ? <CheckCircle weight="fill" /> : <XCircle weight="fill" />}
      </span>
      {equal ? what : 'Differs'}
      {ledger !== undefined && <span className="font-normal tabular">at ledger {formatLedger(ledger)}</span>}
    </span>
  );
}

/** Mono hash with the full value in the tooltip and for screen readers. */
export function HashLine({ label, value }: { label: string; value: string }) {
  return (
    <p className="flex min-w-0 items-baseline gap-2 text-xs text-ink-3">
      <span className="shrink-0">{label}</span>
      <span className="truncate font-mono text-ink-2" title={value} aria-label={`${label} ${value}`}>
        {truncateMiddle(value, 12, 10)}
      </span>
    </p>
  );
}

/** Inline match mark for a table row: icon + word, compact. */
export function RowMatch({ equal }: { equal: boolean }) {
  return (
    <span className={cn('inline-flex items-center gap-1 text-xs', equal ? 'text-good-ink' : 'text-critical-ink')}>
      <span aria-hidden className={equal ? 'text-good' : 'text-critical'}>
        {equal ? <CheckCircle weight="fill" /> : <XCircle weight="fill" />}
      </span>
      {equal ? 'Same' : 'Differs'}
    </span>
  );
}

/** Keyboard stepping for chart hover detail: arrows, Home, End, Escape. */
export function stepHover(e: KeyboardEvent, n: number, cur: number | null, set: (i: number | null) => void): void {
  if (n === 0) return;
  const next =
    e.key === 'ArrowRight' || e.key === 'ArrowDown' ? (cur === null ? 0 : Math.min(n - 1, cur + 1))
    : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? (cur === null ? n - 1 : Math.max(0, cur - 1))
    : e.key === 'Home' ? 0
    : e.key === 'End' ? n - 1
    : e.key === 'Escape' ? null
    : undefined;
  if (next === undefined) return;
  e.preventDefault();
  set(next);
}
