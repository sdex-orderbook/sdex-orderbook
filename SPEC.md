# sdex-orderbook SPEC (normative)

Version 0.1.0, 2026-11-03 (section 7a added 2026-11-08). Read-only service and CLI. Network: Stellar pubnet (protocol 29 on 2026-11-03 [P: horizon.stellar.org root `current_protocol_version: 29`, 2026-11-03]).

## 1. Scope

Rebuild, from ledger data and without Horizon:

1. the set of all SDEX offers and classic constant-product liquidity pools at a ledger;
2. trades derived from transaction results;

and serve:

- `GET /order_book`
- `GET /paths/strict-send`
- `GET /paths/strict-receive` (and the legacy alias `GET /paths`)
- `GET /trade_aggregations`
- `GET /` (service status, not Horizon-shaped)

with Horizon-compatible request parameters and response bodies. Soroban AMMs are out of scope (Horizon `/paths` does not use them either).

## 2. Data sources

| Source | Use | Access |
|---|---|---|
| History archive (default `https://history.stellar.org/prd/core-live/core_live_001`) | HAS JSON + live buckets at a checkpoint | anonymous HTTPS GET |
| Galexie public lake (`https://aws-public-blockchain.s3.amazonaws.com/v1.1/stellar/ledgers/pubnet`) | one zstd `LedgerCloseMetaBatch` per ledger | anonymous HTTPS GET |
| Horizon (`https://horizon.stellar.org`) | differential check only (`sdx diff`) | anonymous HTTPS GET, >= 600 ms between requests |

Every source may be replaced by a `file://` mirror with the same layout.

## 3. Bootstrap (`sdx bootstrap`)

1. Fetch the HAS (`.well-known/stellar-history.json` or `history/xx/yy/zz/history-<hex>.json` for `--checkpoint`). The checkpoint ledger L satisfies (L + 1) % 64 == 0.
2. For each non-zero `curr`/`snap` hash in `currentBuckets` (hot-archive buckets hold only Soroban entries and are skipped), stream `bucket/xx/yy/zz/bucket-<hash>.xdr.gz`: gunzip, read RFC 5531 record marks, peek the BucketEntry type and entry type from the first 12 bytes, decode only OFFER (2) and LIQUIDITY_POOL (5) entries, stop at the first entry type > 5.
3. Shadowing: buckets are processed newest first (level 0 curr, level 0 snap, ..., level 10 snap). The first LIVE/INIT occurrence of a key wins; a DEAD occurrence hides older ones.
4. In full-read mode (library `early_exit = false`) a bucket must hash (SHA-256 of the uncompressed stream) to its name, else `E_DECODE`. The bootstrap uses early-exit mode, which does not hash (`hash_verified: false`); integrity comes from the HAS-vs-ledger-header check (step 6) and the replay pre-image checks.
5. Per-bucket extracts are cached under `--cache` as `bucket-<hash>.extract.xdr` (framed BucketEntry XDR) + `.json` stats. A later bootstrap reuses them.
6. The HAS is checked against the checkpoint ledger header's `bucket_list_hash` (SHA-256 over level hashes; from protocol 23 SHA-256(live || hot archive)) fetched from the lake; a mismatch fails with `E_CONSISTENCY`.
7. Long bucket downloads resume with HTTP `Range` after a dropped connection (up to 20 times).

## 4. State directory

```
state.json         {format: "sdx-state/1", network, ledger, close_time, offers, pools, sha256, bootstrap_checkpoint, source}
state.xdr.zst      zstd(framed LedgerEntry XDR: offers by offer_id asc, then pools by pool id asc)
trades.jsonl       one Trade object per line (section 5.3)
replay.jsonl       one line per replayed ledger: {ledger, close_time, trades, trade_errors[], stats, offers, pools, apply_ms}
bootstrap-report.json   per-bucket timings and counts, total seconds, peak RSS (KiB)
```

`sha256` covers the uncompressed framed bytes. `load` refuses a mismatch (`E_CONSISTENCY`). Trades with `ledger > state.ledger` are ignored on load (an interrupted replay never double counts).

## 5. Behaviour

### 5.1 Incremental ingestion (`sdx replay`, `sdx serve --follow`)

Ledger L+1 must follow state ledger L (`E_SEQUENCE` otherwise). Apply order: fee processing of all transactions; per transaction `tx_changes_before`, operation changes, `tx_changes_after`; `post_tx_apply_fee_processing` (meta V2); upgrade changes. CREATED/UPDATED/RESTORED upsert, REMOVED deletes, STATE pre-images of offers and pools are compared with the held entry (full `LedgerEntry` equality, including `last_modified_ledger_seq`). Mismatches are counted (`state_mismatch`) or, with `--strict`, fail the replay with `E_CONSISTENCY`.

### 5.2 Order book

`GET /order_book?selling_asset_type&selling_asset_code&selling_asset_issuer&buying_asset_type&...&limit` (limit default 20, max 200). Asks = offers selling `selling` for `buying`; bids = offers selling `buying` for `selling`. Each side is grouped by the float64 value N/D, in ascending order of that value, first `limit` levels. A level prints the reduced rational (`price_r`, bids inverted), `price` = rational to 7 decimals (round half away from zero) and `amount` = sum of offer amounts (asks in base units, bids in counter units). Response: `{bids, asks, base, counter}` exactly as Horizon. Pools are not included (as in Horizon).

### 5.3 Trades

Trades come from successful transactions only (`txSUCCESS` or fee-bump inner success), from the ClaimAtoms of PathPaymentStrictReceive, PathPaymentStrictSend, ManageBuyOffer, ManageSellOffer and CreatePassiveSellOffer results. Atoms with both amounts zero are skipped. Fields:

`{ledger, close_time, op_id, order, op_type, sold, bought, amount_sold, amount_bought, price_n, price_d, offer_id?, seller?, pool_id?, rounding_slippage?}`

`op_id` = TOID (ledger << 32 | tx_index << 12 | op_index + 1, tx_index 1-based in apply order). Order-book trade price = maker offer price from the last STATE pre-image of that offer in the operation's changes; pool trade price = amount_bought / amount_sold (not reduced). Pool trades carry Horizon's rounding slippage in bips; a missing pre-image is reported in `replay.jsonl` `trade_errors` and the trade is dropped.

### 5.4 Path finding

`GET /paths/strict-send?source_asset_*&source_amount&destination_assets` and `GET /paths/strict-receive?destination_asset_*&destination_amount&source_assets`. The engine is a port of go-stellar-sdk `exp/orderbook` (NOTICE): bounded Bellman-Ford relaxation for `max_path_length` rounds (default 3, Horizon `--max-path-length`; allowed 1..5), each hop takes the better of the pair's sorted offers and its pool (never both), results sorted by asset string, then amount, then path length, at most 5 per asset. Asset lists accept at most 15 entries. Response `{_embedded: {records: [...]}}` with Horizon's record fields.

Known differences from Horizon (also measured by `sdx diff`):

1. `source_account` (strict-receive) and `destination_account` (strict-send) are rejected with 400: this service holds no account or trustline state. Horizon uses them to list assets, set balances (validated) and exclude the source account's own offers.
2. Edge iteration order is ascending interned asset id; Horizon's is edge-insertion order. Only ties between equal amounts can differ.
3. Result sort is stable; Go's `sort.Sort` is not. Only the order of records with equal (asset, amount, length) can differ.
4. Equal-priced offers are ordered by insertion; after a state reload insertion order becomes offer-id order.
5. Extension: optional `max_path_length` query parameter (1..server max).
6. No rate limiting, no `Latest-Ledger` lag check (`StaleThreshold`).
7. Offer-total arithmetic wraps on i64 overflow exactly like Go (a wrapped total disables the hop); found on mainnet data 2026-11-04.

### 5.5 Trade aggregations

`GET /trade_aggregations?base_asset_*&counter_asset_*&resolution&start_time&end_time&offset&order&limit` with Horizon validation: resolution in {60000, 300000, 900000, 3600000, 86400000, 604800000}; offset a whole number of hours, < 24 h and <= resolution; start rounded up, end rounded down to the bucket grid; limit default 10, max 200. Trades with rounding slippage >= 1000 bips are excluded (Horizon `--rounding-slippage-filter` default). Minute rows first, then buckets, then the "first `limit` buckets" page, exactly as `trade_aggregation.go` (module docs of `aggregate.rs`). Record fields `timestamp, trade_count, base_volume, counter_volume, avg, high, high_r, low, low_r, open, open_r, close, close_r` (strings, as Horizon). Differences: only trades of replayed ledgers exist (`GET /` reports `trades_cover_ledgers`); `_links` has `self` only; `avg` emulates Postgres numeric division with 30 digits then float64 (ties at the 8th decimal could differ).

## 6. Differential check (`sdx diff`)

Cases (JSON array, `kind` = `order_book` | `strict_send` | `strict_receive` | `trade_aggregations`) are run against Horizon in record-then-replay mode: (1) every live case is queried from Horizon and its `Latest-Ledger` H recorded; cases whose H is not ahead of our state are recorded as skipped; (2) our state is replayed ledger by ledger and each case is answered at its H (and at H-1, recorded as `matched_previous_ledger` when only that matches). This is ledger-exact even when replay is slower than the network. (3) Trade aggregations: window = complete minutes between the first replayed trade and our last close time minus 2 minutes. Output `<out>/cases/NNN-<kind>.json` with request, ledger, both bodies, a comparison object and, for order books, the hex XDR offers of both books at H (offline re-check); `<out>/summary.json` with per-kind counts.

## 7. Errors

`E_BAD_REQUEST` (400), `E_NOT_FOUND` (404), `E_FETCH`, `E_DECODE`, `E_SEQUENCE`, `E_CONSISTENCY`, `E_IO`, `E_ARITHMETIC` (500). HTTP errors use Horizon's problem shape `{type, title, status, detail, extras: {code}}`. CLI exit codes: 0 ok, 2 bad request, 1 other.

## 7a. HTTP response headers (2026-11-08, SDX-023)

Every `sdx serve` response carries `Content-Type: application/hal+json; charset=utf-8`, `Latest-Ledger: <state ledger>`, `Access-Control-Allow-Origin: *` and `Access-Control-Expose-Headers: Latest-Ledger`. The API is read-only and GET-only, and Horizon also allows any origin. The exposed header lets a browser page (the `web/` live mode) show which ledger answered. Pre-flight `OPTIONS` requests are not handled; simple GETs do not need them.

## 8. Interfaces exposed to other projects

None yet (no cross-project consumer). The HTTP API (section 1) and the state-directory format (section 4) are the candidate public interfaces; changing them after a consumer appears needs an ADR.
