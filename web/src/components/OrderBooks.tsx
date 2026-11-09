// SPDX-License-Identifier: Apache-2.0
// Order book comparison: one recorded `sdx diff` case at a time, our book
// (rebuilt from ledger data) beside Horizon's body for the same Latest-Ledger.
import { useMemo, useState, type ReactNode } from 'react';
import type { Level, OrderBookCase } from '../lib/types';
import { assetLabel } from '../lib/assets';
import { day } from '../lib/format';
import { cn, ProvenanceChip } from '../ui/kit';
import { DiffBadge, HashLine, Picker, RowMatch } from './common';

const levelEq = (a: Level | undefined, b: Level | undefined) =>
  !!a && !!b && a.price === b.price && a.amount === b.amount && a.n === b.n && a.d === b.d;

interface Row {
  side: 'ask' | 'bid';
  level: Level;
  other: Level | undefined;
  /** Base-asset depth up to and including this level, for the bar. */
  depth: number;
}

function rows(book: { bids: Level[]; asks: Level[] }, other: { bids: Level[]; asks: Level[] }, n: number) {
  // Asks amount is in base units; bids amount is in counter units (Horizon), so
  // bid depth in base units is amount / price. Only used for bar lengths.
  let acc = 0;
  const asks: Row[] = book.asks.slice(0, n).map((l, i) => {
    acc += Number(l.amount);
    return { side: 'ask', level: l, other: other.asks[i], depth: acc };
  });
  acc = 0;
  const bids: Row[] = book.bids.slice(0, n).map((l, i) => {
    acc += Number(l.amount) / Number(l.price);
    return { side: 'bid', level: l, other: other.bids[i], depth: acc };
  });
  return { asks: asks.reverse(), bids };
}

function Ladder({
  title,
  book,
  other,
  n,
  base,
  counter,
  hash,
  provenance,
}: {
  title: string;
  book: { bids: Level[]; asks: Level[] };
  other: { bids: Level[]; asks: Level[] };
  n: number;
  base: string;
  counter: string;
  hash: string;
  provenance: ReactNode;
}) {
  const { asks, bids } = rows(book, other, n);
  const max = Math.max(1e-12, ...asks.map((r) => r.depth), ...bids.map((r) => r.depth));
  const line = (r: Row) => (
    <tr key={`${r.side}-${r.level.price}`} className="border-b border-line last:border-b-0">
      <td className="py-1 pr-3 pl-4 text-right tabular sm:pl-5">
        {r.level.price}
      </td>
      <td className="py-1 pr-3 text-right tabular text-ink-2">{r.level.amount}</td>
      <td className="w-[28%] py-1 pr-3" aria-hidden>
        <span
          className="block h-3 rounded-r-bar"
          style={{
            width: `${Math.max(1, (r.depth / max) * 100)}%`,
            background: r.side === 'ask' ? 'var(--sik-viz-2)' : 'var(--sik-viz-1)',
          }}
        />
      </td>
      <td className="py-1 pr-4 sm:pr-5">
        <RowMatch equal={levelEq(r.level, r.other)} />
      </td>
    </tr>
  );
  return (
    <div className="min-w-0 rounded-md border border-line bg-surface">
      <div className="flex flex-col gap-1 border-b border-line px-4 py-3 sm:px-5">
        <h3 className="text-sm font-semibold text-ink">{title}</h3>
        <HashLine label="SHA-256 of bids + asks" value={hash} />
      </div>
      <div className="overflow-x-auto">
        <table className="w-full min-w-[22rem] text-sm">
          <caption className="sr-only">
            {title}: asks then bids, price in {assetLabel(counter)} per {assetLabel(base)}
          </caption>
          <thead className="bg-sunken text-xs text-ink-2">
            <tr>
              <th scope="col" className="py-1.5 pr-3 pl-4 text-right font-medium sm:pl-5">
                Price ({assetLabel(counter)})
              </th>
              <th scope="col" className="py-1.5 pr-3 text-right font-medium">
                Amount
              </th>
              <th scope="col" className="py-1.5 pr-3 text-left font-medium">
                Depth
              </th>
              <th scope="col" className="py-1.5 pr-4 text-left font-medium sm:pr-5">
                Vs other side
              </th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <th colSpan={4} scope="rowgroup" className="bg-surface px-4 pt-2 pb-1 text-left text-xs font-medium text-ink-3 sm:px-5">
                Asks: offers selling {assetLabel(base)} (amount in {assetLabel(base)})
              </th>
            </tr>
            {asks.map(line)}
            <tr>
              <th colSpan={4} scope="rowgroup" className="border-t border-line-strong bg-surface px-4 pt-2 pb-1 text-left text-xs font-medium text-ink-3 sm:px-5">
                Bids: offers buying {assetLabel(base)} (amount in {assetLabel(counter)})
              </th>
            </tr>
            {bids.map(line)}
          </tbody>
        </table>
      </div>
      <div className="border-t border-line px-4 py-2.5 sm:px-5">{provenance}</div>
    </div>
  );
}

export function OrderBooks({ cases }: { cases: OrderBookCase[] }) {
  const [key, setKey] = useState(() => `${cases[0]?.run}/${cases[0]?.index}`);
  const [all, setAll] = useState(false);
  const c = useMemo(() => cases.find((x) => `${x.run}/${x.index}` === key) ?? cases[0], [cases, key]);
  if (!c) return null;
  const n = all ? c.limit : 8;
  const opts = cases.map((x) => ({
    value: `${x.run}/${x.index}`,
    label: `${assetLabel(x.selling)} / ${assetLabel(x.buying)} at ledger ${x.ledger.toLocaleString('en-US')}`,
  }));
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <Picker label="Recorded case (selling / buying)" value={key} options={opts} onChange={setKey} />
        <div className="flex flex-wrap items-center gap-2">
          <DiffBadge equal={c.equal} ledger={c.ledger} />
          <button
            type="button"
            onClick={() => setAll((v) => !v)}
            className="inline-flex h-8 items-center rounded-sm border border-line-strong bg-surface px-3 text-sm font-medium text-ink transition-colors duration-[120ms] ease-settle hover:bg-sunken active:translate-y-px"
          >
            {all ? 'Show 8 levels' : `Show all ${c.limit} levels`}
          </button>
        </div>
      </div>
      <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
        <Ladder
          title="Rebuilt from ledger data (sdx)"
          book={c.ours}
          other={c.horizon}
          n={n}
          base={c.selling}
          counter={c.buying}
          hash={c.sha256Ours}
          provenance={
            <ProvenanceChip
              kind="recorded"
              ledger={c.ledger}
              date={day(c.capturedAt)}
              source={`fixtures/diff/${c.run}`}
              note="computed by sdx from the replayed state at this ledger"
            />
          }
        />
        <Ladder
          title="horizon.stellar.org /order_book"
          book={c.horizon}
          other={c.ours}
          n={n}
          base={c.selling}
          counter={c.buying}
          hash={c.sha256Horizon}
          provenance={
            <ProvenanceChip
              kind="recorded"
              ledger={c.ledger}
              date={day(c.capturedAt)}
              source="horizon.stellar.org"
              note="Horizon body stored verbatim; ledger from its Latest-Ledger header"
            />
          }
        />
      </div>
      <p className={cn('text-xs text-ink-3')}>
        Equality is checked on the JSON values of <code className="font-mono">bids</code> and <code className="font-mono">asks</code> (all {c.limit} levels of
        each side), as <code className="font-mono">sdx diff</code> did when it recorded the case. The hashes are SHA-256 of the canonical JSON of those two
        fields. Depth bars are cumulative base-asset amounts, drawn for orientation only.
      </p>
    </div>
  );
}
