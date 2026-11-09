// SPDX-License-Identifier: Apache-2.0
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { assetLabel, assetParams, parseAsset } from '../src/lib/assets.ts';
import { ageLabel, fetchOrderBook, fetchStatus, isLoopback, LIVE_MAX_AGE_S, orderBookUrl } from '../src/lib/live.ts';

const USDC = 'USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN';

test('asset strings parse like sdx writes them', () => {
  assert.deepEqual(parseAsset('native'), { type: 'native', code: 'XLM', issuer: null });
  assert.equal(parseAsset(USDC).type, 'credit_alphanum4');
  assert.equal(parseAsset('USTRY:GCRYUGD5NVARGXT56XEZI5CIFCQETYHAPQQTHO2O3IQZTHDH4LATMYWC').type, 'credit_alphanum12');
  assert.equal(assetLabel(USDC), 'USDC');
  assert.throws(() => parseAsset('USDC'), /bad asset/);
  assert.equal(assetParams('selling', 'native'), 'selling_asset_type=native');
});

test('live mode builds Horizon-shaped order book URLs', () => {
  assert.equal(
    orderBookUrl('http://127.0.0.1:8000/', 'native', USDC, 5),
    'http://127.0.0.1:8000/order_book?selling_asset_type=native&buying_asset_type=credit_alphanum4&buying_asset_code=USDC&buying_asset_issuer=GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN&limit=5',
  );
});

test('live mode only talks to loopback servers', () => {
  assert.equal(isLoopback('http://127.0.0.1:8000'), true);
  assert.equal(isLoopback('http://localhost:8000'), true);
  assert.equal(isLoopback('https://horizon.stellar.org'), false);
  assert.equal(isLoopback('not a url'), false);
});

test('live order book takes its ledger from the Latest-Ledger header and refuses responses without it', async () => {
  const body = { bids: [{ price: '0.16', amount: '10.0000000', price_r: { n: 4, d: 25 } }], asks: [] };
  const ok = async () => new Response(JSON.stringify(body), { status: 200, headers: { 'Latest-Ledger': '65306175' } });
  const b = await fetchOrderBook('http://127.0.0.1:8000', 'native', USDC, ok);
  assert.equal(b.ledger, 65306175);
  assert.deepEqual(b.bids[0], { price: '0.16', amount: '10.0000000', n: 4, d: 25 });
  const noHeader = async () => new Response(JSON.stringify(body), { status: 200 });
  await assert.rejects(fetchOrderBook('http://127.0.0.1:8000', 'native', USDC, noHeader), /Latest-Ledger/);
});

test('live status refuses a server that is not sdx', async () => {
  const other = async () => new Response(JSON.stringify({ service: 'something-else' }), { status: 200 });
  await assert.rejects(fetchStatus('http://127.0.0.1:8000', other), /not sdx serve/);
  const sdx = async () =>
    new Response(JSON.stringify({ service: 'sdex-orderbook', ledger: 7, ledger_close_time: 1, offers: 3, liquidity_pools: 0, network: 'pubnet' }), { status: 200 });
  assert.equal((await fetchStatus('http://127.0.0.1:8000', sdx)).offers, 3);
});

test('age label: minutes, hours, days; negative is flagged', () => {
  assert.equal(ageLabel(90), '2 min old');
  assert.equal(ageLabel(7200), '2.0 h old');
  assert.equal(ageLabel(4 * 86400 + 3600), '4 days old');
  assert.match(ageLabel(-5), /ahead/);
  assert.ok(LIVE_MAX_AGE_S <= 600);
});
