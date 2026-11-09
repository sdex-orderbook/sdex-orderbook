# sdex-orderbook

Horizon-free SDEX order book, path finding and trade aggregation, rebuilt from ledger data (history-archive buckets + the public Galexie lake) and served with Horizon-compatible shapes for `/order_book`, `/paths/strict-send`, `/paths/strict-receive` and `/trade_aggregations`. Rust library + `sdx` CLI + local HTTP server. Read-only: never signs or sends anything. Ticket prefix SDX. Discovery source: candidate A6 / RH-T07 (see `SOURCES.md`).

No claim is made that this is the first such service; see `PRIOR-ART.md` (re-checked 2026-11-03: no open Horizon-free equivalent found; StellarRoute, Stellar Pathfinder and pathguard consume Horizon; partial overlaps for trades/OHLCV).

## Demo and docs (2026-11-08)

- `web/`: investor demo (Vite + React + TypeScript + Tailwind v4, kit design system). Order books and trade aggregations side by side with Horizon from the recorded diffs, the path-finding status, the bootstrap and replay evidence, and an optional live mode against a local `sdx serve`. Data: `pnpm demo:data` from `fixtures/` only.
- `site/`: docs site (Astro + Starlight, kit theme): quickstart, CLI, HTTP API, state format, differential check, status and limits.
- `scripts/build-demo.sh` builds both into `demo-dist/`. See [DEMO.md](DEMO.md) (commands and a 3-5 minute script), [PITCH.md](PITCH.md), [DEPLOY.md](DEPLOY.md), [MVP-PLAN.md](MVP-PLAN.md).
- Hosted (2026-11-09): static demo and docs on GitHub Pages, `https://sdex-orderbook.github.io/sdex-orderbook/` and `.../sdex-orderbook/docs/` (recorded fixtures, not live data; SDX-031). A hosted live `sdx serve` endpoint does not exist yet: Render and VPS files are in the repo, nothing is deployed (SDX-032). `/paths` still does not match Horizon (SDX-012); no parity claim is made anywhere.

## Status (2026-11-04, re-checked 2026-11-08)

Measured on 2026-11-04 on a shared, heavily loaded 4-core Fedora box (other agents building), debug build (dev profile with opt-level 3 for gunzip/SHA/zstd crates, 1 for `stellar-xdr` and core), `nice -n 10`. [M] = measured here.

| Piece | State | Evidence |
|---|---|---|
| Bootstrap from history-archive buckets (`sdx bootstrap`) | **works** | Checkpoint 65,304,703: 955,681 offers, 40,265 classic pools, 229,767 assets interned. HAS bucket-list hash equals the checkpoint ledger header's `bucket_list_hash` (from the lake) [M]. 21 live buckets, 9.62 GB uncompressed read. Cold run took ~82 min wall in two attempts (36:47 for 20 buckets, then a dropped connection on the 2.53 GB level-10 bucket; that bucket alone 41 min on the retry); peak RSS 1.83 GB [M]. Early exit only skips the Soroban tail: classic entries are 88% of records (18.6 M accounts, 40.4 M trustlines), so ~90% of the largest bucket was still downloaded [M]. Per-bucket extracts are cached, so a re-bootstrap at a later checkpoint re-downloads only changed buckets. HTTP resume (Range) added after the drop; not yet exercised by a real drop |
| Incremental ingestion from Galexie (`sdx replay`) | **works** | 1,472 consecutive ledgers (65,304,704-65,306,175) applied with 525,948 offer/pool STATE pre-images compared and **0 mismatches**; 159,171 trades derived, 0 missing pre-images [M]. Throughput 0.62 ledger/s average over 683 ledgers (peaks 1.2/s), network closes ~0.18/s [M]. Peak RSS 1.5 GB |
| `/order_book` | **matches Horizon** | `sdx diff`: 12/12 order books identical to horizon.stellar.org at the same `Latest-Ledger` (JSON equality of all 20 bid and ask levels; XLM vs USDC, AQUA, SHX, DOGET, XRP, NRV, SGB, VELO) [M]. Re-verified offline 2026-11-08 by `web/scripts/build-data.ts` |
| `/trade_aggregations` | **matches Horizon** | 24/24 (1-min and 5-min, 8 most-traded pairs, complete minutes of the replayed window) identical, every field of every bucket [M] |
| Trades from ClaimAtoms | **matches Horizon** | 200 trades in 5 fixture ledgers equal Horizon `/trades` (assets, amounts, price n/d, op id/order) [M] |
| `/paths/strict-send`, `/paths/strict-receive` | **does not match yet** | With pools: 5/48 cases equal; our results exploit thin classic pools (e.g. 18/USDC with 0.0022 "18" vs 75 USDC reserves) that horizon.stellar.org did not use on 2026-11-04, although its own `/liquidity_pools` listed them [M]. Without pools on our side only (`--no-pools`; Horizon still uses pools): 12/24 equal [M]. Probe 2026-11-08: after that pool changed (ledger 65,364,568), Horizon routes through it and its direct amount (0.9360584 USDC for 0.001 "18") equals our CAP-38 maths exactly [M: `fixtures/probes/20261108-pool-18-usdc`, unit test]. Horizon's code (go-stellar-sdk `exp/orderbook`, stellar-horizon orderbook stream) has no small-pool filter [P]. Cause still open; SDX-012. Do not rely on `/paths` output |
| HTTP server (`sdx serve`), CLI | works | integration tests; `Latest-Ledger` header; CORS (`Access-Control-Allow-Origin: *`, `Latest-Ledger` exposed) for the web live mode, checked in a browser against `data/state` on 2026-11-08; Horizon problem JSON; `--follow` applies new lake ledgers (not soak-tested); request limits added 2026-11-08 (10 s socket timeouts, line/header caps, 64 connections, 4 concurrent route computations, SDX-029) but no `/paths` deadline, TLS or rate limit: local use only, reverse proxy before any hosting |
| `source_account` / `destination_account` path params | not supported | 400 by design (no account state), SPEC 5.4 |
| Soroban AMMs, SSE streaming, trade backfill before the bootstrap checkpoint | not built | SDX-016, SDX-021 |
| Release build | not measured | the session's release builds were killed by machine restarts |

Tests: `cargo test --workspace --offline` -> 51 passed (38 core unit, 6 CLI unit, 5 fixture, 2 CLI/HTTP); `cargo clippy -D warnings` clean; `cargo fmt --check` clean; `cargo audit --no-fetch` reports no vulnerabilities after the zstd 0.13.3 -> 0.13.4 bump (RUSTSEC-2026-0391, SDX-030) (2026-11-08). Web: `cd web && pnpm test` -> 13 passed (data re-checked against the fixtures, library and live-mode tests); `pnpm typecheck` and `pnpm build` clean; docs `cd site && pnpm build` clean (2026-11-08). Live runs used `http://` archive and lake URLs to avoid debug-build TLS cost; the defaults are `https://`.

## Commands

```
cd projects/sdex-orderbook                    # rust-toolchain.toml pins 1.97.1
bash scripts/check.sh                         # fmt --check, clippy -D warnings, cargo test (offline)
cargo build -p sik-sdex-orderbook-cli         # binary: target/debug/sdx (dev profile optimises the hot crates)

# live, read-only
sdx bootstrap --out data/state                       # history archive -> offers + pools at the latest checkpoint
sdx replay --state data/state --to head              # Galexie lake -> current
sdx order-book --state data/state --selling native --buying USDC:GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN
sdx strict-send --state data/state --source native --amount 100 --destinations USDC:GA5Z...
sdx strict-receive --state data/state --destination USDC:GA5Z... --amount 10 --sources native
sdx trade-aggregations --state data/state --base native --counter USDC:GA5Z... --resolution 60000
sdx serve --state data/state --addr 127.0.0.1:8000 --follow   # GET /order_book?... like Horizon
sdx diff --state data/state --out fixtures/diff/<run> --auto 8  # vs horizon.stellar.org at matching ledgers
```

`data/` is a working directory (not committed). Fixtures live in `fixtures/` with provenance: `diff/` (sdx diff runs), `runs/` (bootstrap and replay statistics, `scripts/export-run-stats.py`), `probes/` (read-only Horizon probes, `scripts/probe-pool.sh`), `lake/` and `buckets/` (test inputs).

```
# demo (see DEMO.md)
scripts/build-demo.sh && python3 -m http.server 4173 --directory demo-dist
```

## Read next

`SPEC.md` (normative behaviour and Horizon differences) -> `ARCHITECTURE.md` (why Rust, why buckets) -> `TASKS.md` -> `TESTING.md` -> `RISKS.md` -> `PRIOR-ART.md` -> `SOURCES.md` -> `AGENTS.md`. Ported go-stellar-sdk code is attributed in `NOTICE`.
