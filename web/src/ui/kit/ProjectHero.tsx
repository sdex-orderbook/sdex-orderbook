// SPDX-License-Identifier: Apache-2.0
// ProjectHero: the top of a project's landing view. Left: monogram, name, one
// sentence, gates, one action. Right: the "instrument panel" holding the
// project's signature figure, its best real measurement, with provenance.
// No gradient blobs, no fake screenshots: the figure is the visual. DESIGN.md 10.
import type { ReactNode } from 'react';
import { cn, formatExact } from './cn';
import { Monogram } from './AppShell';
import { UnknownValue } from './EmptyState';

export interface SignatureFigure {
  label: string;
  value: number | string | null;
  unit?: string;
  format?: (n: number) => string;
  /** ProvenanceChip. Required in spirit: a hero number without a source is not allowed. */
  provenance: ReactNode;
  /** Method or caveat in one line. */
  footnote?: ReactNode;
  /** Reason shown when value is null. */
  missingReason?: string;
}

export interface ProjectHeroProps {
  code: string;
  name: string;
  family?: string;
  /** One sentence, <= 20 words. */
  lede: ReactNode;
  gates?: ReactNode;
  /** One primary action, optionally one secondary. */
  actions?: ReactNode;
  figure: SignatureFigure;
  className?: string;
}

export function ProjectHero({ code, name, family, lede, gates, actions, figure, className }: ProjectHeroProps) {
  const v = figure.value;
  const shown = typeof v === 'number' ? (figure.format ?? formatExact)(v) : v;
  return (
    <section className={cn('grid grid-cols-1 items-stretch gap-6 md:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)] md:gap-10', className)}>
      <div className="flex min-w-0 flex-col justify-center gap-4 py-2">
        <div className="flex items-center gap-3">
          <Monogram code={code} size="lg" />
          {family && <span className="text-sm text-ink-3">{family}</span>}
        </div>
        <h1 className="text-3xl font-semibold tracking-hero text-ink">{name}</h1>
        <p className="max-w-[52ch] text-md text-ink-2">{lede}</p>
        {gates && <div className="flex flex-wrap gap-1.5">{gates}</div>}
        {actions && <div className="mt-1 flex flex-wrap items-center gap-2">{actions}</div>}
      </div>
      {/* Instrument panel: outer tray + inner plate, concentric radii (14px outer, 8px inner, 6px gap). */}
      <div className="rounded-lg border border-line bg-sunken p-1.5">
        <div className="flex h-full flex-col justify-between gap-6 rounded-[8px] border border-line bg-surface p-5 inset-shadow-highlight sm:p-6">
          <p className="text-sm text-ink-2">{figure.label}</p>
          <div>
            {v === null ? (
              <UnknownValue size="figure" reason={figure.missingReason ?? 'Not measured yet.'} />
            ) : (
              <p className="text-hero font-medium tracking-hero text-ink">
                {shown}
                {figure.unit && <span className="ml-2 text-xl font-normal tracking-normal text-ink-3">{figure.unit}</span>}
              </p>
            )}
            {figure.footnote && <p className="mt-2 text-xs text-ink-3">{figure.footnote}</p>}
          </div>
          <div>{figure.provenance}</div>
        </div>
      </div>
    </section>
  );
}
