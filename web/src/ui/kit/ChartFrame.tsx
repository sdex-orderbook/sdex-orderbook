// SPDX-License-Identifier: Apache-2.0
// ChartFrame and RankedBars. Every chart sits in a ChartFrame: title, one-line
// subtitle, provenance, and a Chart/Table switch (the table is the accessible
// twin and is never optional). RankedBars is the reference chart: one series,
// one hue (viz-1), bars <= 24px, 4px rounded data-end, square baseline, value
// at the tip, hover and keyboard tooltip. DESIGN.md 7.
import { useId, useState, type ReactNode } from 'react';
import { ChartBar, Table } from '@phosphor-icons/react';
import { cn, formatCompact, formatExact } from './cn';

export interface ChartFrameProps {
  title: ReactNode;
  /** Unit and scope in one line, e.g. "Bytes per wasm blob, 13 blobs". */
  subtitle?: ReactNode;
  /** ProvenanceChip for the data shown. */
  provenance?: ReactNode;
  /** The chart. */
  children: ReactNode;
  /** The same data as a table (DataTable). Required: tooltips never gate a value. */
  table: ReactNode;
  /** Legend, shown for two or more series only. */
  legend?: ReactNode;
  className?: string;
}

export function ChartFrame({ title, subtitle, provenance, children, table, legend, className }: ChartFrameProps) {
  const [view, setView] = useState<'chart' | 'table'>('chart');
  const id = useId();
  return (
    <figure className={cn('min-w-0 rounded-md border border-line bg-surface', className)} aria-labelledby={`${id}-t`}>
      <div className="flex flex-wrap items-start justify-between gap-x-4 gap-y-3 px-4 pt-4 sm:px-5">
        <div className="min-w-0">
          <h3 id={`${id}-t`} className="text-sm font-semibold text-ink">
            {title}
          </h3>
          {subtitle && <p className="mt-0.5 text-xs text-ink-3">{subtitle}</p>}
        </div>
        <div role="tablist" aria-label="View" className="inline-flex rounded-sm border border-line bg-sunken p-0.5">
          {(
            [
              ['chart', 'Chart', <ChartBar key="c" />],
              ['table', 'Table', <Table key="t" />],
            ] as const
          ).map(([v, label, icon]) => (
            <button
              key={v}
              type="button"
              role="tab"
              aria-selected={view === v}
              onClick={() => setView(v)}
              className={cn(
                'inline-flex h-6 items-center gap-1 rounded-[4px] px-2 text-xs transition-colors duration-[120ms] ease-settle',
                view === v ? 'bg-surface text-ink shadow-1' : 'text-ink-3 hover:text-ink',
              )}
            >
              <span aria-hidden>{icon}</span>
              {label}
            </button>
          ))}
        </div>
      </div>
      {legend && view === 'chart' && <div className="px-4 pt-3 sm:px-5">{legend}</div>}
      <div className={cn(view === 'chart' ? 'px-4 pb-4 pt-4 sm:px-5' : 'p-3')}>{view === 'chart' ? children : table}</div>
      {provenance && <figcaption className="border-t border-line px-4 py-2.5 sm:px-5">{provenance}</figcaption>}
    </figure>
  );
}

export interface Bar {
  key: string;
  label: ReactNode;
  value: number;
  /** Plain-text label for the tooltip and accessible name. */
  text: string;
}

export interface RankedBarsProps {
  bars: Bar[];
  unit?: string;
  format?: (n: number) => string;
  /** Highlight one bar (emphasis form): the rest go to viz-other. */
  emphasize?: string;
  /** Width reserved for category labels. */
  labelWidth?: string;
  className?: string;
}

export function RankedBars({ bars, unit, format = formatCompact, emphasize, labelWidth = '11rem', className }: RankedBarsProps) {
  const [hover, setHover] = useState<string | null>(null);
  const max = Math.max(0, ...bars.map((b) => b.value)) || 1;
  const sorted = [...bars].sort((a, b) => b.value - a.value);
  return (
    <ul
      role="list"
      className={cn('grid gap-1.5', className)}
      style={{ gridTemplateColumns: `minmax(0, ${labelWidth}) minmax(0, 1fr)` }}
    >
      {sorted.map((b) => {
        const pct = Math.max(0, (b.value / max) * 100);
        const color = emphasize && emphasize !== b.key ? 'var(--sik-viz-other)' : 'var(--sik-viz-1)';
        const active = hover === b.key;
        return (
          <li key={b.key} className="col-span-2 grid grid-cols-subgrid items-center">
            <span className="truncate pr-3 text-xs text-ink-2" title={b.text}>
              {b.label}
            </span>
            <span
              tabIndex={0}
              aria-label={`${b.text}: ${formatExact(b.value)}${unit ? ` ${unit}` : ''}`}
              onPointerEnter={() => setHover(b.key)}
              onPointerLeave={() => setHover((h) => (h === b.key ? null : h))}
              onFocus={() => setHover(b.key)}
              onBlur={() => setHover((h) => (h === b.key ? null : h))}
              className="relative flex h-6 items-center gap-2 rounded-xs outline-offset-1"
            >
              <span
                className="block h-4 rounded-r-bar transition-[filter] duration-[120ms] ease-settle"
                style={{ width: `${pct}%`, minWidth: b.value > 0 ? 2 : 0, background: color, filter: active ? 'brightness(1.12)' : undefined }}
              />
              <span className="shrink-0 text-xs text-ink tabular">{format(b.value)}</span>
              {active && (
                <span
                  role="tooltip"
                  className="pointer-events-none absolute bottom-full left-0 z-[var(--sik-z-popover)] mb-1.5 rounded-sm border border-line bg-raised px-2.5 py-1.5 text-xs whitespace-nowrap shadow-2"
                >
                  <span className="font-semibold text-ink tabular">
                    {formatExact(b.value)}
                    {unit ? ` ${unit}` : ''}
                  </span>
                  <span className="ml-2 text-ink-3">{b.text}</span>
                </span>
              )}
            </span>
          </li>
        );
      })}
    </ul>
  );
}
