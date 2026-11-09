// SPDX-License-Identifier: Apache-2.0
// EmptyState, UnknownValue, Skeleton: the honest states. A value the page
// does not have is shown as missing, with the reason; never as zero, never
// as a plausible placeholder. See DESIGN.md 8.6.
import type { ReactNode } from 'react';
import { Hourglass, LockKey, Plugs, Question, Tray, WarningOctagon } from '@phosphor-icons/react';
import { cn } from './cn';

export type EmptyTone = 'empty' | 'not-yet-available' | 'error';

const EMPTY_ICON: Record<EmptyTone, ReactNode> = {
  empty: <Tray weight="light" />,
  'not-yet-available': <Hourglass weight="light" />,
  error: <WarningOctagon weight="light" />,
};

export interface EmptyStateProps {
  tone?: EmptyTone;
  icon?: ReactNode;
  title: ReactNode;
  children?: ReactNode;
  /** One action (button or link). Keep the label to one or two words. */
  action?: ReactNode;
  /** Ticket that tracks the missing feature, e.g. "DTC-012". Shown for not-yet-available. */
  ticket?: string;
  /** Error detail (message, status code). Shown in mono for tone="error". */
  detail?: string;
  className?: string;
}

export function EmptyState({ tone = 'empty', icon, title, children, action, ticket, detail, className }: EmptyStateProps) {
  return (
    <section
      role={tone === 'error' ? 'alert' : 'status'}
      className={cn(
        'grid grid-cols-1 gap-4 rounded-md border bg-surface px-5 py-6 sm:grid-cols-[auto_1fr] sm:gap-5 sm:px-6',
        tone === 'error' ? 'border-critical-ring' : 'border-line',
        className,
      )}
    >
      <span
        aria-hidden
        className={cn(
          'grid size-10 place-items-center rounded-sm border text-[22px]',
          tone === 'error' ? 'border-critical-ring bg-critical-wash text-critical-ink' : 'border-line bg-sunken text-ink-3',
        )}
      >
        {icon ?? EMPTY_ICON[tone]}
      </span>
      <div className="min-w-0 max-w-[60ch]">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <h3 className="text-base font-semibold text-ink">{title}</h3>
          {tone === 'not-yet-available' && (
            <span className="label-mono inline-flex h-5 items-center rounded-xs border border-neutral-ring bg-neutral-wash px-1.5 text-neutral-ink">
              Not yet available
            </span>
          )}
        </div>
        {children && <div className="mt-1 text-sm text-ink-2">{children}</div>}
        {tone === 'error' && detail && (
          <pre className="mt-3 whitespace-pre-wrap break-words rounded-sm bg-sunken px-3 py-2 font-mono text-xs text-ink-2">{detail}</pre>
        )}
        {(action || ticket) && (
          <div className="mt-4 flex flex-wrap items-center gap-3">
            {action}
            {ticket && <span className="font-mono text-xs text-ink-3">Tracked as {ticket}</span>}
          </div>
        )}
      </div>
    </section>
  );
}

export type UnknownKind = 'unknown' | 'unavailable' | 'error' | 'encrypted';

const UNKNOWN: Record<UnknownKind, { text: string; icon: ReactNode }> = {
  unknown: { text: 'Unknown', icon: <Question weight="bold" /> },
  unavailable: { text: 'Not yet available', icon: <Hourglass weight="bold" /> },
  error: { text: 'Read failed', icon: <Plugs weight="bold" /> },
  encrypted: { text: 'Encrypted', icon: <LockKey weight="bold" /> },
};

export interface UnknownValueProps {
  kind?: UnknownKind;
  /** Why the value is missing. Required: an unexplained gap is a bug. */
  reason: string;
  /** Override the visible text. */
  label?: string;
  size?: 'inline' | 'figure';
  className?: string;
}

/** Stands in for a number the page does not have. Never renders "0" or "-". */
export function UnknownValue({ kind = 'unknown', reason, label, size = 'inline', className }: UnknownValueProps) {
  const u = UNKNOWN[kind];
  return (
    <span
      title={reason}
      className={cn(
        'inline-flex items-center gap-1 text-ink-3',
        size === 'figure' ? 'text-lg font-medium tracking-tight' : 'text-[length:inherit]',
        kind === 'error' && 'text-critical-ink',
        className,
      )}
    >
      <span aria-hidden className={size === 'figure' ? 'text-base' : 'text-[0.9em]'}>
        {u.icon}
      </span>
      <span className="underline decoration-dotted decoration-1 underline-offset-[3px]">{label ?? u.text}</span>
      <span className="sr-only">. {reason}</span>
    </span>
  );
}

/** Shape-matched loading placeholder for the first load only. On refetch, keep
    the previous render at reduced opacity instead (DESIGN.md 9). */
export function Skeleton({ className }: { className?: string }) {
  return <span aria-hidden className={cn('block animate-pulse rounded-xs bg-line', className)} />;
}
