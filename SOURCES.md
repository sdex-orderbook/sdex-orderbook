# Sources

Trust tags: [P] primary, [S] secondary, [V] vendor, [I] inference, [E] estimate, [U] unverified, [M] measured here. Dates are when read.

## Discovery notes (this programme)

- `../zero-to-one-discovery/reports/SDF open issues build gaps.md` section 5a, candidate A6 (need, evidence issues, cheapest deliverable, novelty risk) [S, 2026-11-03].
- `../zero-to-one-discovery/research_notes/SDF open issues build gaps/rpc_horizon_data.md` T07 (issue list horizon#113, #82, #114, #84, #41, #104, #50, #53, #92, #83, #57, #81, #119; js-stellar-sdk#1620; StellarRoute; covered-by list) and section 7 "Not checked" [S, 2026-11-03].
- `../zero-to-one-discovery/research_notes/Stellar aggregator devtools infra ideas/defi_aggregators.md`, `data_aggregators.md` (Stellar Expert 402 on candles, SDEX volume not on DefiLlama) [S, 2026-11-03].
- `../zero-to-one-discovery/notes/IDEA_REGISTER.md` (router coverage: StellarBroker, WOWMAX, LumAgg, Soroswap Aggregator), `notes/candidate_pool.md` P020 (horizonless) [S].

## Primary code read (semantics we mirror)

- stellar-horizon `main` (2026-11-03): `internal/actions/path.go`, `internal/actions/orderbook.go`, `internal/actions/trade.go`, `internal/db2/history/orderbook.go`, `internal/db2/history/trade_aggregation.go`, `internal/ingest/processors/trades_processor.go`, `internal/simplepath/inmemory.go` (`maxAssetsPerPath = 5`, `MaxInMemoryPathLength = 5`), `internal/flags.go` (`max-path-length` 3, `max-assets-per-path-request` 15, `rounding-slippage-filter` 1000), `internal/db2/schema/migrations/10_add_trades_price.sql` (`max_price_agg` tie rule) [P]. https://github.com/stellar/stellar-horizon
- go-stellar-sdk `f8efc493fca9` (pinned by stellar-horizon `go.mod`): `exp/orderbook/{search,graph,edges,pools,utils}.go`, `price/main.go` [P]. https://github.com/stellar/go-stellar-sdk
- `stellar-xdr` 28.0.0 crate source (type layouts) [P].

## Live endpoints (read-only GETs)

- `https://history.stellar.org/prd/core-live/core_live_001/.well-known/stellar-history.json`: checkpoint 65,290,111, server `stellar-core 30.0.0`, 11 live levels + 11 hot-archive levels [P, 2026-11-03 20:11 UTC].
- Live bucket sizes (HEAD Content-Length, 21 non-zero buckets): 4.83 GB total, largest 2.53 GB [M, 2026-11-03].
- Archive throughput, one stream: 135,419,589 B in 223 s = 0.61 MB/s [M, 2026-11-03].
- Galexie lake `.config.json`: zstd, 1 ledger per batch, 64,000 per partition [P, 2026-11-03]; object sizes 100,909 B (65,290,112) and 177,312 B (65,290,200), GET 2.3 s [M].
- Horizon root: `horizon_version 30.0.0-b3ff4119`, `current_protocol_version 29`; `/order_book` and `/paths/*` responses carry a `Latest-Ledger` header [P, 2026-11-03].
- Stellar Expert: public OpenAPI paths (asset, holders, position, rating, supply, ledger time) [P]; `/explorer/public/market/XLM/USDC-GA5Z...-1` 24 h: 47,883 trades, base volume 4,640,663.9 XLM [V, 2026-11-03 ~20:19 UTC]; `/candles` HTTP 402 [P].
- StellarBroker `https://api.stellar.broker/`: `{"status":"active","ledger":65290226,...}` [P, 2026-11-03].

## Measured in this project (2026-11-04)

- Bootstrap, replay and diff numbers: README "Status" (raw: `fixtures/diff/*/summary.json`, `diff.provenance.json`) [M].
- Horizon `/liquidity_pools?reserves=USDC:GA5Z...,18:GBBB...` lists pool `05442dce...` with reserves 0.0022002 "18" / 75.4313983 USDC, last modified 65,305,602, while `/paths/strict-send` 0.0001 "18" -> USDC returns no records [P, 2026-11-04 ~18:20 UTC]. Horizon flag `--disable-pool-path-finding` exists (default false) [P: internal/flags.go]; whether SDF sets it is unknown [U].

## Docs

- SDF Horizon EOL wording ("nearing end-of-life ... won't receive new feature development"): see the 2026-11-08 [P] entry below. RPC migration guide: re-read 2026-11-08 via the stellar-docs MCP [P: developers.stellar.org/docs/data/apis/migrate-from-horizon-to-rpc#endpoint-mapping]; the mapping table has no row for `/order_book`, `/paths*` or `/trade_aggregations`, and the page says endpoints without mappings have no direct RPC replacement.

## Read or measured 2026-11-08 (MVP session)

- SDF docs, Horizon page: "Horizon is nearing end-of-life and will eventually be deprecated in favor of Stellar RPC and Portfolio APIs. While it will continue to receive updates to maintain compatibility with upcoming protocol releases, it won't receive new feature development." [P: https://developers.stellar.org/docs/data/apis, via the stellar-docs MCP search, 2026-11-08]
- stellar-horizon `main`: `internal/ingest/orderbook.go` (graph loads all non-deleted pools; incremental updates take rows with `last_modified_ledger` above the graph's last ledger), `internal/db2/history/liquidity_pools.go`, `internal/simplepath/inmemory.go`, `internal/actions/path.go` [P, raw.githubusercontent.com, 2026-11-08; not committed].
- go-stellar-sdk `exp/orderbook` v0.7.1 (local Go module cache; NOTICE cites f8efc493fca9, not compared line by line): `makeTrade` passes `calculateRoundingSlippage=false`; no reserve-size filter in `search.go`, `pools.go`, `graph.go`, `edges.go` [P].
- `fixtures/probes/20261108-pool-18-usdc`: pool 05442dce... reserves 0.0129024 "18" / 13.0498007 USDC, last modified ledger 65,364,568 (2026-11-08 03:34:42 UTC); `/paths/strict-send` 0.001 "18" -> USDC returns a direct record 0.9360584 USDC (Latest-Ledger 65,365,369) [M].
- `fixtures/probes/20261108-pool-the1-ustry`: no THE1/USTRY pool listed, empty order book, no path (Latest-Ledger 65,365,594-65,365,595) [M].
