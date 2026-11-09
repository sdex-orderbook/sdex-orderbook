# Prior art: sdex-orderbook (SDX-001)

Re-check run 2026-11-03 by the build agent (session rule 1). Question asked: is an open, Horizon-free service that rebuilds the SDEX order book, `/paths` and `/trade_aggregations` from ledger data already built and maintained by someone else?

**Verdict: no full overlap found; partial overlaps narrow the scope (below). Not a STOP.** This is not a claim that the project is "first" or "novel" (gate G-NOVELTY): GitHub search is shallow and private router engines exist.

## Method

- GitHub repository search (unauthenticated REST, 7 s between calls), sorted by last update, queries: `sdex orderbook`, `stellar order book path finding`, `stellar strict-send paths`, `horizon order_book stellar`, `stellar trade aggregations`, `stellar pathfinding`, `stellar orderbook rust`, `stellar dex indexer`, `horizon alternative stellar`, `stellar ledgerclosemeta offers`, `stellar galexie orderbook`, `stellar amm path payment router`. Raw JSON kept in the agent scratchpad, not committed.
- READMEs read for every hit that looked relevant (raw.githubusercontent.com).
- Stellar Expert public OpenAPI read (`https://stellar.expert/openapi.html`) and three undocumented endpoints probed.
- StellarBroker API root probed.
- SDF sources read: stellar-horizon `main` (path, order book, trade-aggregation code) and go-stellar-sdk `exp/orderbook` at f8efc493fca9 (the version Horizon's go.mod pins).
- Local registers: `../zero-to-one-discovery/notes/IDEA_REGISTER.md`, `candidate_pool.md` (P020), `research_notes/.../rpc_horizon_data.md` (T07), `defi_aggregators.md`, `data_aggregators.md`.

## Findings

| Project | What it does | Overlap | Verdict | Evidence |
|---|---|---|---|---|
| SDF Horizon (`stellar/stellar-horizon`, Apache-2.0) | Serves `/order_book` (Postgres offers table), `/paths/*` (in-memory graph from go-stellar-sdk `exp/orderbook`), `/trade_aggregations` (`history_trades_60000`) | Full functional overlap, but it is the end-of-life service this project replaces; needs captive core + Postgres | Reference implementation. We port its path engine and pool maths (NOTICE) and mirror its SQL semantics | [P] repo read 2026-11-03; docs say Horizon "won't receive new feature development" [S: rpc_horizon_data.md, 2026-11-03] |
| SDF Stellar RPC | Ledger entries by key, events, transactions | No order book, paths or aggregations; migration guide omits these endpoints | No overlap | [S: rpc_horizon_data.md T07] |
| SDF Hubble (BigQuery `crypto-stellar`) | Analytics tables incl. offers, liquidity pools, history_trades | Data overlap, not a serving API; needs a Google Cloud login (not used by agents) | Not checked hands-on [U] | [S: data_aggregators.md] |
| SDF stellar-etl / CDP processors (Go) | Export ledger-entry changes, trades | Building blocks (trade extraction), no book/paths/aggregation API | Partial (trades) | [S: rpc_horizon_data.md, IDEA_REGISTER] |
| `StellarRoute/StellarRoute` (MIT, ★4, created 2026-01-19, pushed 2026-10-28) | DEX aggregator: indexer + routing + REST incl. `/order_book` | Its README says the indexer "Syncs SDEX orderbooks and Soroban AMM pools states from Stellar Horizon API" | Consumer of Horizon; a potential user of this service, not prior art for the Horizon-free part | [P] README fetched 2026-11-03 |
| `Stellar-Pathfinder/stellar-pathfinder` (Apache-2.0, ★129, created 2026-10-06, pushed 2026-11-03) | Corridor-integrity monitor: prices stablecoin -> fiat corridors | README: "Uses Stellar's `strict-send` pathfinding API" (Horizon) | Consumer of Horizon `/paths`; potential user | [P] README fetched 2026-11-03 |
| `pathguardd/pathguard-backend` (★0, created 2026-09-14) | Rust/axum route-quality API | "Indexes SDEX order-book depth (via Horizon)" | Consumer of Horizon | [P] README 2026-11-03 |
| `fadesany/Stellar-Trade-Processor` (Apache-2.0, ★0, created 2026-09-15, pushed 2026-09-18) | Go: derives trades (offer fills, path-payment hops, Soroswap swaps) from ledgers via go-stellar-sdk ingest | Overlaps our trade derivation only; no order book, paths, aggregation or Horizon-shaped API | Partial overlap (trades). We derive trades ourselves to match Horizon's price/slippage rules | [P] README 2026-11-03 |
| `Stellar-Nest/stellanest-indexer` (★0) | Rust indexer polling Horizon `/trades` | Consumer of Horizon | No overlap | [P] README 2026-11-03 |
| `horizonless-labs/horizonless` (Apache-2.0, ★0, pushed 2026-10-19) | Horizon-style payments/operations/effects from RPC (TypeScript) | Same "Horizon after EOL" thesis; its README lists trades as unsupported, no book/paths | Adjacent, no overlap | [P] README + repo API 2026-11-03 |
| Stellar Expert API | Public OpenAPI lists asset, holders, supply, rating, ledger-time endpoints only. Undocumented `/explorer/public/market/{a}/{b}` returns 24 h/7 d volume; `/candles` returns **HTTP 402 Payment Required** | No public order book or paths; candles paywalled | No overlap for open use | [P] probed 2026-11-03 |
| StellarBroker (`api.stellar.broker`) | Router; API root returns `{"status":"active",...}` | Private routing engine; no public order-book/paths API found | Not an open replacement [I] | [P] probed 2026-11-03 |
| WOWMAX, LumAgg, Soroswap Aggregator | Routers with their own graphs | Private engines | Covered as routers, not as an open Horizon-free data service [S: IDEA_REGISTER D] | not re-probed |
| Rumble Fish SCF prices API | OHLCV from a Galexie pipeline | Overlaps trade aggregation only; licence and API not checked [U] | Partial (OHLCV) | [S: rpc_horizon_data.md T07] |

## How the scope is narrowed by the partial overlaps

1. Trades: we do not try to be a general trade indexer (Stellar-Trade-Processor, stellar-etl). Trades are derived only to serve `/trade_aggregations` with Horizon's exact rules (maker-offer price, pool rounding-slippage filter, 1-minute rollup).
2. OHLCV vendors (Rumble Fish, Stellar Expert paid candles) exist; our value is Horizon-compatible shapes and ledger-exact differential evidence, not a price feed.
3. Routers keep private graphs; we do not compete on routing quality. We reproduce Horizon `/paths` semantics so existing Horizon clients (wallets, StellarRoute, Stellar Pathfinder) can switch base URL.
4. Soroban AMMs (Aquarius, Soroswap, Phoenix, Sushi) are out of scope for v1: Horizon `/paths` does not include them either.

## Not checked

- GitHub code search (needs auth) and Sourcegraph; crates.io/npm searches for "orderbook stellar" were not run.
- Hubble tables hands-on (needs a Google Cloud login, forbidden for agents).
- Whether SDF plans an RPC-side replacement for these endpoints beyond what the migration guide says.
