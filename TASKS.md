# Tasks

Status: `TODO`, `IN-PROGRESS(date)`, `DONE(date, note)`, `BLOCKED(gate or reason)`.

| Id | Title | Deps | Acceptance | Status |
|---|---|---|---|---|
| SDX-001 | Prior-art re-check | - | `PRIOR-ART.md` dated, verdict recorded | DONE(2026-11-03, no full overlap; partial overlaps narrow scope) |
| SDX-002 | Scaffold: workspace, ten docs, NOTICE | 001 | builds; docs present | DONE(2026-11-03) |
| SDX-003 | Value types and exact maths (asset, amount, price, pool maths) | 002 | unit tests incl. Horizon-captured vectors | DONE(2026-11-03) |
| SDX-004 | BookState + ledger apply with STATE pre-image checks | 003 | unit tests; fixture replay strict with 0 mismatches | DONE(2026-11-04) |
| SDX-005 | History-archive bootstrap (HAS, bucket streaming, early exit, shadowing, extract cache) | 004 | unit tests; real level-0 buckets hash-verify | DONE(2026-11-04, mainnet checkpoint 65,304,703: 955,681 offers, 40,265 pools, HAS = header hash; HTTP Range resume added) |
| SDX-006 | Galexie lake fetch/decode/replay | 004 | fixture ledgers decode and apply | DONE(2026-11-04, 1,472 ledgers, 525,948 pre-images, 0 mismatches; parallel decode) |
| SDX-007 | Trades from ClaimAtoms + trade aggregations | 006 | derived trades equal Horizon `/trades` for 5 fixture ledgers; aggregation unit tests | DONE(2026-11-04) |
| SDX-008 | Order book summary | 004 | unit + CLI tests | DONE(2026-11-03) |
| SDX-009 | Strict-send / strict-receive port | 004 | unit tests on synthetic graph | DONE(2026-11-03) |
| SDX-010 | HTTP server + CLI | 007-009 | CLI/HTTP integration tests | DONE(2026-11-04) |
| SDX-011 | Differential check vs Horizon at matching ledgers, fixtures | 005-010 | `fixtures/diff/<run>/` + offline reproduction test | DONE(2026-11-04, 2 runs in fixtures/diff: order book 12/12, aggregations 24/24 equal; paths 5/48 with pools, 12/24 without) |
| SDX-012 | Explain or fix each mismatch class from SDX-011 | 011 | every mismatch class has a cause in SPEC 5.4/5.5 or a fix | TODO (investigated 2026-11-08, cause not found). Findings: (1) Horizon's path code (go-stellar-sdk `exp/orderbook` search/pools/graph/edges, local copy v0.7.1; stellar-horizon `internal/ingest/orderbook.go`, `internal/db2/history/liquidity_pools.go`, `internal/simplepath/inmemory.go` read 2026-11-08) loads every non-deleted pool and has no small-pool filter [P]. (2) Probe `fixtures/probes/20261108-pool-18-usdc`: pool 05442dce... changed at ledger 65,364,568; Horizon now routes 0.001 "18" directly through it for 0.9360584 USDC, equal to our CAP-38 payout (unit test) [M]. On 2026-11-04 Horizon listed the same pool (0.0022002 / 75.4313983) yet returned no path (SOURCES.md) [P]. (3) Probe `fixtures/probes/20261108-pool-the1-ustry`: Horizon lists no THE1/USTRY pool and no offers on 2026-11-08 [M]. Our state at 65,306,175 (2026-11-04) has no THE1/USTRY offers but answers a direct strict-send 0.001 THE1 -> 0.0050774 USTRY, so it held a THE1/USTRY pool then (`sdx order-book` + `sdx strict-send --max-path-length 1` on data/state, 2026-11-08) [M]. Whether Horizon had that pool on 2026-11-04 is unknown (deleted since, or never in its graph); SDX-028 would let us print its reserves. Hypothesis [I, untested]: Horizon's in-memory graph held a stale or missing entry for some pools until their rows changed. Next step: record a fresh `sdx diff` at the network head (needs a new bootstrap or long replay) and probe thin pools before and after they change |
| SDX-013 | Scheduled full-read bucket hash verification | 005 | all buckets of one checkpoint verified | TODO |
| SDX-014 | Check HAS against checkpoint ledger header `bucket_list_hash` | 005 | wired in `sdx bootstrap`; unit test of hash layout | DONE(2026-11-04, matched on mainnet checkpoint 65,304,703) |
| SDX-015 | Nightly diff with a fixed case set + trend file | 011 | script + 7 runs | TODO |
| SDX-016 | Trade backfill for ledgers before the bootstrap checkpoint | 007 | `sdx backfill --from --to` | TODO |
| SDX-017 | Measure SDEX share of Stellar DEX volume from derived trades | 016 | 7-day report | TODO |
| SDX-018 | Bump `stellar-xdr` pin for protocol 30 | - | decodes P30 meta | BLOCKED(protocol 30 not on mainnet; vote 2026-11-23 [U]) |
| SDX-019 | Persist book insertion order across restarts | 004 | equal-price order survives save/load | TODO |
| SDX-020 | Trustline index to support `source_account` / `destination_account` | 005 | Horizon parity for account-based path queries | TODO |
| SDX-021 | SSE streaming for `/order_book` | 010 | stream test | TODO |
| SDX-022 | Drafts (not posted) for js-stellar-sdk#1620 / StellarRoute | 011 | files in `drafts/` | TODO |
| SDX-023 | CORS + exposed `Latest-Ledger` on `sdx serve` for the browser live mode | 010 | header test in `cli.rs`; SPEC 7a | DONE(2026-11-08, checked in a browser against data/state at 65,306,175) |
| SDX-024 | Export bootstrap + replay statistics into a committed fixture | 005, 006 | `scripts/export-run-stats.py`, `fixtures/runs/` with provenance | DONE(2026-11-08, 1,472 ledgers, 525,948 pre-images, 21 buckets; duplicated replay lines dropped after checking equal stats) |
| SDX-025 | Web demo (`web/`): order books, aggregations, paths status, pipeline, live mode | 011, 023, 024 | builds offline from fixtures; tests; both themes and 390 px checked | DONE(2026-11-08, 12 web tests; data recount checked against summary.json) |
| SDX-026 | Docs site (`site/`, Starlight with kit theme) | 010 | builds offline; numbers from fixtures | DONE(2026-11-08, 8 pages) |
| SDX-027 | MVP docs: MVP-PLAN, DEMO, PITCH, DEPLOY, README status | 025, 026 | files present, numbers tagged | DONE(2026-11-08) |
| SDX-028 | `sdx pool` command: print a pool's reserves from the state (to compare thin pools with Horizon without path queries) | 004 | CLI test | TODO |
| SDX-029 | `sdx serve` hardening: socket timeouts, line/header caps, connection and in-flight caps; docs say reverse proxy required | 010 | unit tests for head parsing and caps; CLI test for 414 and stalled client | DONE(2026-11-08). Remaining: per-request deadline for `/paths`, TLS, per-client rate limit (proxy) |
| SDX-030 | Bump `zstd` to =0.13.4 (RUSTSEC-2026-0391) | 004 | `cargo audit`, `check.sh` | DONE(2026-11-08) |
| SDX-031 | Publish static demo + docs on GitHub Pages | 025, 026 | `pages` workflow green; app and docs, with assets and links, answer under `/sdex-orderbook/` | IN-PROGRESS(2026-11-09). Workflow passed once; project-page docs base fix (`SDX_DOCS_BASE`) committed, not yet pushed or re-checked live |
| SDX-032 | Hosted read-only `sdx serve` (Oracle Always Free Arm VM) | 010, 029 | public HTTPS endpoint answers `/order_book` for XLM/USDC with a recent `Latest-Ledger`; README and DEPLOY.md state limits | IN-PROGRESS(2026-11-09). Target: Oracle Cloud Always Free Arm VM (DEPLOY.md 4, "Oracle setup"); systemd and nginx files written, nothing deployed. Render dropped (free plan: 512 MB RAM, no disk). Needs a proxy with rate limit and `/paths` timeout (remaining SDX-029) |
| SDX-033 | Release binary workflow | 010 | tag `v*` produces `sdx` tarball + sha256 | TODO (workflow written 2026-11-09, never run) |

## Cross-project requests

0. (2026-11-08) Root `docs/CHANGELOG.md` entry for the MVP session (orchestrator writes it).

1. Root `NOTICE` and `docs/THIRD_PARTY.md`: add go-stellar-sdk (Apache-2.0, commit f8efc493fca9) for the ports listed in `projects/sdex-orderbook/NOTICE` (gate G-LICENCE; agents may not edit root docs this session).
2. Root `docs/ARCHITECTURE.md`: optional entry for sdex-orderbook (Rust; consumes history archive + Galexie lake; no project dependencies).
