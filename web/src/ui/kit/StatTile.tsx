// SPDX-License-Identifier: Apache-2.0
// StatTile and Sparkline. Contract (dataviz figure spec): label, value,
// optional delta vs a named period, optional 12-point trend, and provenance.
// A tile with no value renders UnknownValue with the reason. See DESIGN.md 8.4.
import type { ReactNode } from 'react';
import { ArrowDownRight, ArrowUpRight, Minus } from '@phosphor-icons/react';
import { cn, formatCompact, formatExact } from './cn';
import { UnknownValue, type UnknownKind } from './EmptyState';
import { ProvenanceChip, type Provenance } from './ProvenanceChip';

export interface Delta {
  /** Signed change, in the same unit as the value. */
  value: number;
  /** What it is compared to, e.g. "vs ledger 65,290,900" or "vs 2026-10-28". */
  period: string;
  /** Whether an increase is good. Omit when direction carries no judgement. */
  upIsGood?: boolean;
  format?: (n: number) => string;
}

export interface StatTileProps {
  /** Sentence case, no trailing colon. */
  label: string;
  /** null/undefined renders UnknownValue with `missing.reason`. */
  value?: number | string | null;
  /** Unit shown after the value in muted ink, e.g. "bytes", "%", "XLM". */
  unit?: string;
  /** Number formatter for the visible value. Default: full under 10,000, compact above. */
  format?: (n: number) => string;
  delta?: Delta;
  /** 2 to 24 points, oldest first. Only real series: omit rather than invent. */
  trend?: number[];
  trendLabel?: string;
  provenance?: Provenance;
  missing?: { kind?: UnknownKind; reason: string };
  /** One line of method or caveat under the figure. */
  footnote?: ReactNode;
  /** The single hero tile of a view: larger figure. One per view. */
  hero?: boolean;
  className?: string;
}

export function StatTile({
  label,
  value,
  unit,
  format = formatCompact,
  delta,
  trend,
  trendLabel,
  provenance,
  missing,
  footnote,
  hero = false,
  className,
}: StatTileProps) {
  const has = value !== null && value !== undefined;
  const shown = typeof value === 'number' ? format(value) : value;
  const exact = typeof value === 'number' ? formatExact(value) : undefined;

  return (
    <div className={cn('flex min-w-0 flex-col gap-3 rounded-md border border-line bg-surface p-4 sm:p-5', className)}>
      <p className="text-sm text-ink-2">{label}</p>
      <div className="flex min-w-0 items-end justify-between gap-4">
        <div className="min-w-0">
          {has ? (
            <p
              className={cn(
                'font-medium text-ink',
                hero ? 'text-hero tracking-hero' : 'text-figure tracking-tight',
              )}
              title={exact && exact !== shown ? `${exact}${unit ? ` ${unit}` : ''}` : undefined}
            >
              {shown}
              {unit && <span className={cn('ml-1.5 font-normal tracking-normal text-ink-3', hero ? 'text-xl' : 'text-base')}>{unit}</span>}
            </p>
          ) : (
            <UnknownValue size="figure" kind={missing?.kind} reason={missing?.reason ?? 'No value was supplied.'} />
          )}
          {delta && has && <DeltaLine delta={delta} />}
        </div>
        {trend && trend.length > 1 && <Sparkline values={trend} label={trendLabel ?? `${label} trend`} />}
      </div>
      {footnote && <p className="text-xs text-ink-3">{footnote}</p>}
      {provenance && (
        <div className="mt-auto pt-1">
          <ProvenanceChip {...provenance} />
        </div>
      )}
    </div>
  );
}

function DeltaLine({ delta }: { delta: Delta }) {
  const { value, period, upIsGood, format = formatCompact } = delta;
  const dir = value > 0 ? 'up' : value < 0 ? 'down' : 'flat';
  const judged = upIsGood === undefined || dir === 'flat' ? 'neutral' : (dir === 'up') === upIsGood ? 'good' : 'bad';
  const color = judged === 'good' ? 'text-good-ink' : judged === 'bad' ? 'text-critical-ink' : 'text-ink-2';
  const Icon = dir === 'up' ? ArrowUpRight : dir === 'down' ? ArrowDownRight : Minus;
  const sign = value > 0 ? '+' : value < 0 ? '−' : '';
  return (
    <p className="mt-1 flex items-center gap-1 text-sm">
      <span className={cn('inline-flex items-center gap-0.5 font-medium tabular', color)}>
        <Icon weight="bold" aria-hidden />
        {sign}
        {format(Math.abs(value))}
      </span>
      <span className="text-ink-3">{period}</span>
    </p>
  );
}

export interface SparklineProps {
  values: number[];
  label: string;
  width?: number;
  height?: number;
  className?: string;
}

/** De-emphasised 2px line, current point in the accent with a 2px surface ring. */
export function Sparkline({ values, label, width = 96, height = 32, className }: SparklineProps) {
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const pad = 4;
  const pts = values.map((v, i) => {
    const x = pad + (i * (width - pad * 2)) / (values.length - 1);
    const y = pad + (1 - (v - min) / span) * (height - pad * 2);
    return [x, y] as const;
  });
  const last = pts[pts.length - 1];
  const d = pts.map(([x, y], i) => `${i ? 'L' : 'M'}${x.toFixed(1)} ${y.toFixed(1)}`).join(' ');
  return (
    <svg
      role="img"
      aria-label={`${label}: ${values.length} points, from ${formatExact(values[0] ?? 0)} to ${formatExact(values[values.length - 1] ?? 0)}`}
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={cn('shrink-0 overflow-visible', className)}
    >
      <path d={d} fill="none" stroke="var(--sik-viz-other)" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
      {last && <circle cx={last[0]} cy={last[1]} r={4} fill="var(--sik-accent)" stroke="var(--sik-surface)" strokeWidth={2} />}
    </svg>
  );
}
