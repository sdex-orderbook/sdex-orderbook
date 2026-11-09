// SPDX-License-Identifier: Apache-2.0
// Optional live mode against a local `sdx serve`. Off until the user asks; if
// no server answers, the panel says so and shows the command to start one.
import { useState } from 'react';
import { Plugs } from '@phosphor-icons/react';
import { assetLabel } from '../lib/assets';
import { ageLabel, fetchOrderBook, fetchStatus, isLoopback, LIVE_MAX_AGE_S, type LiveBook, type LiveStatus } from '../lib/live';
import { int, utc } from '../lib/format';
import { Button, EmptyState, ProvenanceChip } from '../ui/kit';
import { Picker } from './common';

export function LivePanel({ pairs }: { pairs: { selling: string; buying: string }[] }) {
  const [base, setBase] = useState('http://127.0.0.1:8000');
  const [pair, setPair] = useState(() => (pairs[0] ? `${pairs[0].selling}|${pairs[0].buying}` : ''));
  const [state, setState] = useState<
    { k: 'idle' } | { k: 'loading' } | { k: 'error'; msg: string } | { k: 'ok'; status: LiveStatus; book: LiveBook; at: string }
  >({ k: 'idle' });
  const [selling = '', buying = ''] = pair.split('|');

  async function run() {
    if (!isLoopback(base)) {
      setState({ k: 'error', msg: 'Live mode only talks to a server on this machine (127.0.0.1 or localhost).' });
      return;
    }
    setState({ k: 'loading' });
    try {
      const status = await fetchStatus(base);
      const book = await fetchOrderBook(base, selling, buying);
      setState({ k: 'ok', status, book, at: new Date().toISOString() });
    } catch (e) {
      setState({ k: 'error', msg: e instanceof Error ? e.message : String(e) });
    }
  }

  const ageSec = state.k === 'ok' ? (Date.parse(state.at) - state.status.ledgerCloseTime * 1000) / 1000 : 0;
  const uniq = Array.from(new Map(pairs.map((p) => [`${p.selling}|${p.buying}`, p])).values());
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-end gap-3">
        <label className="flex flex-col gap-1.5 text-xs text-ink-2">
          Local server address
          <input
            value={base}
            onChange={(e) => setBase(e.target.value)}
            spellCheck={false}
            className="h-8 w-64 max-w-full rounded-sm border border-line-strong bg-surface px-2.5 font-mono text-sm text-ink"
          />
        </label>
        <Picker
          label="Pair (selling / buying)"
          value={pair}
          onChange={setPair}
          options={uniq.map((p) => ({ value: `${p.selling}|${p.buying}`, label: `${assetLabel(p.selling)} / ${assetLabel(p.buying)}` }))}
        />
        <Button variant="secondary" onClick={run} disabled={state.k === 'loading'}>
          {state.k === 'loading' ? 'Querying' : 'Query local server'}
        </Button>
      </div>

      {state.k === 'idle' && (
        <p className="max-w-prose text-sm text-ink-2">
          Nothing is read until you press the button. Start a server first with{' '}
          <code className="rounded-xs bg-sunken px-1 font-mono text-xs">sdx serve --state data/state --addr 127.0.0.1:8000</code>. It answers from its own
          state at its own ledger, so there is no Horizon comparison here; the recorded diffs above are the comparison.
        </p>
      )}
      {state.k === 'error' && (
        <EmptyState tone="error" icon={<Plugs weight="light" />} title="No local sdx server answered" detail={state.msg}>
          Nothing was updated. Start <code className="font-mono">sdx serve</code> on this machine (DEMO.md, step 5) and try again.
        </EmptyState>
      )}
      {state.k === 'ok' && (
        <div className="rounded-md border border-line bg-surface">
          <div className="flex flex-col gap-2 border-b border-line px-4 py-3 sm:px-5">
            <h3 className="text-sm font-semibold text-ink">
              {assetLabel(selling)} / {assetLabel(buying)} from your local sdx
            </h3>
            <ProvenanceChip
              kind={ageSec <= LIVE_MAX_AGE_S ? 'live' : 'recorded'}
              ledger={state.book.ledger}
              date={ageSec <= LIVE_MAX_AGE_S ? state.at : new Date(state.status.ledgerCloseTime * 1000).toISOString()}
              source={base}
              note={`${ageLabel(ageSec)}; read from your local server at ${utc(state.at)}; ${int(state.status.offers)} offers, ${int(state.status.pools)} pools held`}
            />
            <p className="text-xs text-ink-3">
              {ageSec <= LIVE_MAX_AGE_S
                ? `The server's state closed ${utc(state.status.ledgerCloseTime)}, within ${LIVE_MAX_AGE_S / 60} minutes of this read.`
                : `Not live: the server's state closed ${utc(state.status.ledgerCloseTime)}, ${ageLabel(ageSec)}. The chip says "recorded" with that date. Run sdx serve --follow to keep it current.`}
            </p>
          </div>
          <div className="grid grid-cols-1 gap-0 sm:grid-cols-2">
            {(['bids', 'asks'] as const).map((side) => (
              <table key={side} className="w-full text-sm">
                <caption className="px-4 pt-3 text-left text-xs font-medium text-ink-3 sm:px-5">
                  {side === 'bids' ? `Bids (amount in ${assetLabel(buying)})` : `Asks (amount in ${assetLabel(selling)})`}
                </caption>
                <thead className="text-xs text-ink-2">
                  <tr>
                    <th scope="col" className="px-4 py-1 text-right font-medium sm:px-5">
                      Price
                    </th>
                    <th scope="col" className="px-4 py-1 text-right font-medium sm:px-5">
                      Amount
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {state.book[side].slice(0, 10).map((l) => (
                    <tr key={l.price} className="border-t border-line">
                      <td className="px-4 py-1 text-right tabular text-ink sm:px-5">{l.price}</td>
                      <td className="px-4 py-1 text-right tabular text-ink-2 sm:px-5">{l.amount}</td>
                    </tr>
                  ))}
                  {state.book[side].length === 0 && (
                    <tr>
                      <td colSpan={2} className="px-4 py-2 text-right text-ink-3 sm:px-5">
                        No offers on this side.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
