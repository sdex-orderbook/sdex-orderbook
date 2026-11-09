// SPDX-License-Identifier: Apache-2.0
// GateBadge and GateNotice: make the kit's gates (docs/GATES.md) visible.
// Badges sit in the app header and next to the feature they constrain.
// Notices are full-width callouts for wording a gate requires (G-ZK).
import type { ReactNode } from 'react';
import { Eye, Flask, Hourglass, Lock, LockKey, LockOpen, ShieldWarning, TestTube } from '@phosphor-icons/react';
import { cn } from './cn';

export type Tone = 'good' | 'warning' | 'serious' | 'critical' | 'neutral';

export const TONE_CLASSES: Record<Tone, { badge: string; icon: string; notice: string }> = {
  good: { badge: 'bg-good-wash text-good-ink border-good-ring', icon: 'text-good-ink', notice: 'bg-good-wash border-good-ring' },
  warning: { badge: 'bg-warning-wash text-warning-ink border-warning-ring', icon: 'text-warning-ink', notice: 'bg-warning-wash border-warning-ring' },
  serious: { badge: 'bg-serious-wash text-serious-ink border-serious-ring', icon: 'text-serious-ink', notice: 'bg-serious-wash border-serious-ring' },
  critical: { badge: 'bg-critical-wash text-critical-ink border-critical-ring', icon: 'text-critical-ink', notice: 'bg-critical-wash border-critical-ring' },
  neutral: { badge: 'bg-neutral-wash text-neutral-ink border-neutral-ring', icon: 'text-neutral-ink', notice: 'bg-sunken border-line' },
};

export type GateKind =
  | 'demo-only'
  | 'testnet-only'
  | 'mainnet-read-only'
  | 'unaudited'
  | 'g-zk'
  | 'not-yet-available'
  | 'gate';

interface GateSpec {
  label: string;
  tone: Tone;
  icon: ReactNode;
  description: string;
}

const SPEC: Record<Exclude<GateKind, 'gate'>, GateSpec> = {
  'demo-only': {
    label: 'Demo-only',
    tone: 'serious',
    icon: <Flask weight="bold" aria-hidden />,
    description: 'Prototype code. Unaudited. Do not hold value with it.',
  },
  'testnet-only': {
    label: 'Testnet only',
    tone: 'warning',
    icon: <TestTube weight="bold" aria-hidden />,
    description: 'Writes go to Stellar testnet only, signed with throwaway keys generated at runtime and never stored.',
  },
  'mainnet-read-only': {
    label: 'Mainnet read-only',
    tone: 'neutral',
    icon: <Eye weight="bold" aria-hidden />,
    description: 'Reads public Stellar mainnet data. Never signs or submits a transaction (gate G-VALUE).',
  },
  unaudited: {
    label: 'Unaudited',
    tone: 'warning',
    icon: <ShieldWarning weight="bold" aria-hidden />,
    description: 'No external audit has been done. Findings are evidence for reviewers, not an audit.',
  },
  'g-zk': {
    label: 'G-ZK: not zero-knowledge',
    tone: 'warning',
    icon: <LockKey weight="bold" aria-hidden />,
    description: 'Amounts are encrypted from the public; not zero-knowledge. Gate G-ZK is closed.',
  },
  'not-yet-available': {
    label: 'Not yet available',
    tone: 'neutral',
    icon: <Hourglass weight="bold" aria-hidden />,
    description: 'This feature is not built yet. Nothing here is simulated.',
  },
};

export interface GateBadgeProps {
  kind: GateKind;
  /** For kind="gate": the gate id from docs/GATES.md, e.g. "G-VALUE". */
  gateId?: string;
  /** For kind="gate": whether the gate is open or closed. */
  state?: 'open' | 'closed';
  /** Override the label (keep it to two or three words). */
  label?: string;
  /** Override the tooltip / accessible description. */
  description?: string;
  href?: string;
  className?: string;
}

function resolve({ kind, gateId, state, label, description }: GateBadgeProps): GateSpec {
  if (kind === 'gate') {
    const open = state === 'open';
    return {
      label: label ?? `${gateId ?? 'Gate'} ${open ? 'open' : 'closed'}`,
      tone: open ? 'good' : 'neutral',
      icon: open ? <LockOpen weight="bold" aria-hidden /> : <Lock weight="bold" aria-hidden />,
      description: description ?? `${gateId ?? 'This gate'} is ${open ? 'open' : 'closed'}. See docs/GATES.md.`,
    };
  }
  const s = SPEC[kind];
  return { ...s, label: label ?? s.label, description: description ?? s.description };
}

export function GateBadge(props: GateBadgeProps) {
  const { href, className } = props;
  const s = resolve(props);
  const t = TONE_CLASSES[s.tone];
  const cls = cn(
    'inline-flex h-6 shrink-0 items-center gap-1.5 whitespace-nowrap rounded-xs border px-2 label-mono',
    t.badge,
    href && 'transition-[filter] duration-[120ms] ease-settle hover:brightness-[0.97]',
    className,
  );
  const content = (
    <>
      <span className={cn('text-[12px] leading-none', t.icon)}>{s.icon}</span>
      <span>{s.label}</span>
      <span className="sr-only">: {s.description}</span>
    </>
  );
  return href ? (
    <a href={href} className={cls} title={s.description}>
      {content}
    </a>
  ) : (
    <span className={cls} title={s.description}>
      {content}
    </span>
  );
}

export interface GateNoticeProps {
  tone?: Tone;
  icon?: ReactNode;
  title: ReactNode;
  children?: ReactNode;
  /** Small trailing reference, e.g. "Gate G-ZK, docs/GATES.md". */
  reference?: ReactNode;
  className?: string;
}

/** Full-width callout for gate wording. Not dismissable: a gate is not a toast. */
export function GateNotice({ tone = 'warning', icon, title, children, reference, className }: GateNoticeProps) {
  const t = TONE_CLASSES[tone];
  return (
    <aside
      role="note"
      className={cn('grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 rounded-md border px-4 py-3 text-sm', t.notice, className)}
    >
      <span className={cn('row-span-2 mt-0.5 text-base', t.icon)} aria-hidden>
        {icon ?? <LockKey weight="bold" />}
      </span>
      <p className="font-medium text-ink">{title}</p>
      <div className="text-ink-2">
        {children}
        {reference && <p className="mt-1 font-mono text-xs text-ink-3">{reference}</p>}
      </div>
    </aside>
  );
}

/**
 * The G-ZK notice with the wording docs/GATES.md requires. Use `variant="sigma"`
 * for confidential-engine stage 1 (sigma proofs, issuer co-signature).
 */
export function GZKNotice({ variant = 'oz-verifier', className }: { variant?: 'oz-verifier' | 'sigma'; className?: string }) {
  return variant === 'sigma' ? (
    <GateNotice
      className={className}
      title="Amounts hidden from the public; issuer-visible."
      reference="Gate G-ZK (sigma proofs), docs/GATES.md"
    >
      Stage 1 proofs are checked against the issuer&rsquo;s co-signature. The issuer can see every amount. This is not a
      zero-knowledge or private system.
    </GateNotice>
  ) : (
    <GateNotice
      className={className}
      title="Amounts are encrypted from the public; not zero-knowledge."
      reference="Gate G-ZK, docs/GATES.md. OZ verifier status: CR-03"
    >
      The on-chain verifier for OpenZeppelin stellar-confidential accepts proofs that are not zero-knowledge. Until a
      zero-knowledge verifier is merged upstream, released, adopted and audited, do not describe balances as private.
    </GateNotice>
  );
}
