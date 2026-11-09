// SPDX-License-Identifier: Apache-2.0
// SDX demo: SDEX order book, trade aggregations and path finding rebuilt from
// Stellar ledger data, compared with horizon.stellar.org at the same ledger.
// All figures come from src/data/demo.json (pnpm demo:data, committed fixtures)
// or from a live read of a local sdx server that the user starts.
import { BookOpen, ChartLineUp, Path, Stack, Plugs, Gauge } from '@phosphor-icons/react';
import type { DemoData } from './lib/types';
import { assetLabel } from './lib/assets';
import { day, int } from './lib/format';
import { AppShell, GateBadge, ProjectHero, ProvenanceChip, StatTile, StatusTable, formatLedger } from './ui/kit';
import { Section } from './components/common';
import { OrderBooks } from './components/OrderBooks';
import { Aggregations } from './components/Aggregations';
import { Paths } from './components/Paths';
import { Pipeline } from './components/Pipeline';
import { LivePanel } from './components/LivePanel';

const DOCS_URL: string = import.meta.env.VITE_DOCS_URL ?? 'docs/';

export function App({ data: d }: { data: DemoData }) {
  const t = d.totals;
  const runs = d.runs;
  const withPools = runs.find((r) => r.includePools);
  // Range of the order-book cases only (runs also hold path cases at other ledgers).
  const obLedgers = d.orderBooks.map((o) => o.ledger);
  const firstLedger = Math.min(...obLedgers);
  const lastLedger = Math.max(...obLedgers);
  const captureDay = runs[0] ? day(runs[0].capturedAt) : undefined;
  const pairs = Array.from(new Set(d.orderBooks.map((o) => `${assetLabel(o.selling)}/${assetLabel(o.buying)}`)));
  const rs = d.runStats;
  const diffProv = { kind: 'recorded' as const, date: captureDay, source: 'fixtures/diff (2 runs)' };

  const gates = (
    <>
      <GateBadge kind="mainnet-read-only" />
      <GateBadge kind="unaudited" />
      <GateBadge kind="not-yet-available" label="Path parity: not yet" description="Path finding does not match Horizon yet (SDX-012). Order books and trade aggregations do." />
    </>
  );

  return (
    <AppShell
      project={{ name: 'sdex-orderbook', code: 'SDX', family: 'Markets and liquidity data', href: '#overview' }}
      gates={gates}
      nav={[
        { label: 'Overview', href: '#overview', icon: <Gauge />, active: true },
        { label: 'Order books', href: '#order-books', icon: <Stack /> },
        { label: 'Trade aggregations', href: '#aggregations', icon: <ChartLineUp /> },
        { label: 'Path finding', href: '#paths', icon: <Path /> },
        { label: 'How it was built', href: '#pipeline', icon: <BookOpen /> },
        { label: 'Live (local)', href: '#live', icon: <Plugs /> },
      ]}
      actions={
        <a href={DOCS_URL} className="hidden h-8 items-center rounded-sm px-3 text-sm font-medium text-ink-2 hover:bg-sunken hover:text-ink sm:inline-flex">
          Docs
        </a>
      }
      footer={<span>Data: {d.generatedFrom.join(', ')}</span>}
    >
      <section id="overview" aria-label="Overview">
        <ProjectHero
          code="SDX"
          name="SDEX order book, rebuilt from ledger data"
          family="Markets and liquidity data"
          lede="Offers, pools and trades rebuilt from history-archive buckets and the public ledger lake, served in Horizon's shapes, checked against Horizon."
          gates={gates}
          actions={
            <>
              <a
                href="#order-books"
                className="inline-flex h-8 items-center rounded-sm bg-accent px-3 text-sm font-medium text-ink-inverse inset-shadow-highlight transition-[filter,transform] duration-[120ms] ease-settle hover:brightness-110 active:translate-y-px"
              >
                Compare order books
              </a>
              <a href={DOCS_URL} className="inline-flex h-8 items-center rounded-sm border border-line-strong bg-surface px-3 text-sm font-medium text-ink hover:bg-sunken">
                Read the docs
              </a>
            </>
          }
          figure={{
            label: 'Order books identical to Horizon at the same ledger',
            value: `${t.orderBooks.equal} of ${t.orderBooks.cases}`,
            footnote: `${pairs.length} pairs, ${d.orderBooks[0]?.limit ?? 20} price levels per side, ledgers ${formatLedger(firstLedger)} to ${formatLedger(lastLedger)}.`,
            provenance: <ProvenanceChip {...diffProv} note="sdx diff, Horizon bodies stored verbatim beside ours" />,
          }}
        />

        <div className="mt-8 grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-4">
          <StatTile
            label="Offers held at the last replayed ledger"
            value={rs.state.offers}
            format={int}
            footnote={`${int(rs.state.pools)} classic pools; ${int(rs.bootstrap.assets)} assets seen at bootstrap.`}
            provenance={{ kind: 'recorded', ledger: rs.state.ledger, date: day(new Date(rs.state.close_time * 1000).toISOString()) }}
          />
          <StatTile
            label="Ledgers replayed from the lake"
            value={rs.replay.ledgers}
            format={int}
            footnote={`${formatLedger(rs.replay.from_ledger)} to ${formatLedger(rs.replay.to_ledger)}, median apply ${rs.replay.apply_ms_median.toFixed(1)} ms.`}
            provenance={{ kind: 'recorded', ledger: rs.replay.to_ledger, date: day(new Date(rs.replay.last_close_time * 1000).toISOString()) }}
          />
          <StatTile
            label="Pre-image mismatches"
            value={rs.replay.state_mismatch}
            format={int}
            footnote={`Out of ${int(rs.replay.state_checked)} offer and pool STATE entries checked.`}
            provenance={{ kind: 'recorded', ledger: rs.replay.to_ledger, date: day(new Date(rs.replay.last_close_time * 1000).toISOString()) }}
          />
          <StatTile
            label="Trades derived from ledger meta"
            value={rs.replay.trades}
            format={int}
            footnote={`${int(rs.replay.trade_errors)} dropped for a missing maker pre-image.`}
            provenance={{ kind: 'recorded', ledger: rs.replay.to_ledger, date: day(new Date(rs.replay.last_close_time * 1000).toISOString()) }}
          />
        </div>

        <StatusTable
          className="mt-3"
          caption="Horizon parity by endpoint"
          meta={<ProvenanceChip {...diffProv} />}
          valueHeader="Identical cases"
          rows={[
            {
              id: 'ob',
              name: '/order_book',
              status: t.orderBooks.equal === t.orderBooks.cases ? 'pass' : 'fail',
              detail: 'Bids and asks compared as JSON at Horizon\'s Latest-Ledger.',
              value: `${t.orderBooks.equal} of ${t.orderBooks.cases}`,
            },
            {
              id: 'ta',
              name: '/trade_aggregations',
              status: t.aggregations.equal === t.aggregations.cases ? 'pass' : 'fail',
              detail: '1 and 5 minute buckets over the replayed window, every field.',
              value: `${t.aggregations.equal} of ${t.aggregations.cases}`,
            },
            {
              id: 'pa',
              name: '/paths/strict-send and /paths/strict-receive',
              status: 'fail',
              statusLabel: 'Differs',
              detail: `Does not match yet. With pools (run 1). Run 2 with pools off on our side only: ${t.pathsNoPools.equal} of ${t.pathsNoPools.cases}. SDX-012.`,
              value: `${t.pathsWithPools.equal} of ${t.pathsWithPools.cases}`,
            },
            {
              id: 'acct',
              name: 'source_account / destination_account path parameters',
              status: 'skipped',
              statusLabel: 'Not supported',
              detail: 'Rejected with 400: the service holds no account or trustline state (SDX-020).',
              value: 'Not tested',
            },
          ]}
        />
      </section>

      <Section
        id="order-books"
        title="Order books, side by side"
        lede="Each case was recorded by sdx diff: Horizon's response and its Latest-Ledger, then our book computed after replaying to exactly that ledger."
        meta={<ProvenanceChip {...diffProv} ledger={undefined} note={`ledgers ${formatLedger(firstLedger)} to ${formatLedger(lastLedger)}`} />}
      >
        <OrderBooks cases={d.orderBooks} />
      </Section>

      <Section
        id="aggregations"
        title="Trade aggregations from derived trades"
        lede="Trades come from ClaimAtoms in transaction results, not from Horizon. Buckets follow Horizon's rules, including the rounding-slippage filter."
      >
        <Aggregations cases={d.aggregations} />
      </Section>

      <Section
        id="paths"
        title="Path finding"
        lede={`Ported from go-stellar-sdk, but not yet equal to Horizon: ${t.pathsWithPools.equal} of ${t.pathsWithPools.cases} cases identical with pools. Shown as recorded.`}
      >
        <Paths runs={runs} cases={d.paths} probe={d.probe} />
      </Section>

      <Section
        id="pipeline"
        title="How the state was built"
        lede="One bootstrap from the history archive, then ledger-by-ledger replay of the public Galexie lake, with every pre-image checked."
      >
        <Pipeline rs={rs} />
      </Section>

      <Section
        id="live"
        title="Live mode with a local server"
        lede="Optional. Query an sdx server running on your machine; this page only reads from it."
      >
        <LivePanel pairs={d.orderBooks.map((o) => ({ selling: o.selling, buying: o.buying }))} />
      </Section>

      {withPools && (
        <p className="mt-10 border-t border-line pt-6 text-xs text-ink-3">
          Recorded with <code className="font-mono">{withPools.command.replace('data/bin/sdx3', 'sdx')}</code> on {day(withPools.capturedAt)} ({int(withPools.horizonRequests)}{' '}
          Horizon requests, at most one every 600 ms). The state was bootstrapped at checkpoint {formatLedger(withPools.bootstrapCheckpoint)}.
        </p>
      )}
    </AppShell>
  );
}
