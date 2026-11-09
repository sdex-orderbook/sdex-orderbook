<!-- SPDX-License-Identifier: Apache-2.0 -->
# sdex-orderbook MVP plan

Written 2026-11-08 at the start of the MVP session. Tags: [M] measured, [P] primary source, [E] estimate, [U] unverified.

## 1. Current state (before this session)

- Rust library + `sdx` CLI + local HTTP server, read-only. `scripts/check.sh` (fmt, clippy `-D warnings`, 44 offline tests) was green on 2026-11-04.
- A real mainnet state exists on this machine in `data/state/` (not committed, 115 MB): bootstrap at checkpoint 65,304,703 from history-archive buckets, replayed through Galexie to ledger 65,306,175 (2026-11-04), 955,466 offers, 40,273 classic pools, 194,375 derived trades [M].
- Two differential runs against horizon.stellar.org are committed in `fixtures/diff/` with both bodies per case at the same `Latest-Ledger` [M]:
  - order book 12/12 equal, trade aggregations 24/24 equal;
  - path finding 5/48 equal with pools, 12/24 without pools (SDX-012 open).
- No demo surface, no MVP docs (DEMO, PITCH, DEPLOY).
- No `todo!()` / unimplemented paths in the code (checked with grep, 2026-11-08).

Constraints for this session: no re-bootstrap (82 min, 9.6 GB) and no long replay. The state is 4 days behind the network (about 60,000 ledgers; at the measured 0.62 ledger/s a catch-up would take more than a day [E: 60,000 / 0.62 s]), so a new live diff against Horizon is not possible this session. All demo numbers come from the committed fixtures and from the run logs in `data/state/`, exported once into committed fixtures with provenance.

## 2. MVP definition (what an investor sees in 3-5 minutes)

A static web app (`web/`) titled "SDEX order book, rebuilt from ledger data", showing:

1. Hero: the signature figure "12 of 12 order books byte-equal to Horizon at the same ledger", chip "Recorded mainnet capture, ledgers 65,305,916-65,306,175, 2026-11-04". Gate badges in the header: MAINNET READ-ONLY, UNAUDITED.
2. Scoreboard: order books 12/12, trade aggregations 24/24, path finding 5/48 (status "Does not match yet", with the reason), replay 0 pre-image mismatches over 1,472 ledgers.
3. Order book comparison: pick one of the 12 recorded cases; two ladders side by side (rebuilt from ledger data | Horizon), depth bars, a diff badge ("Equal at ledger N") and SHA-256 of both bodies.
4. Trade aggregations: pick pair and resolution; OHLC candles drawn from our records, and a table with our row and Horizon's row per bucket plus the equality result.
5. Path finding: an honest panel. Per case: Horizon's best vs ours, the path, and the status. Explains the known mismatch class (dust classic pools), shows a fresh probe of Horizon (2026-11-08) for the pool in the first case, and links SDX-012.
6. How it was built: pipeline (history archive -> bootstrap -> Galexie replay -> `sdx serve` -> `sdx diff`), the per-bucket bootstrap table (21 buckets, timings), a per-ledger replay chart (pre-images checked per ledger, 0 mismatches).
7. Optional live mode: if `sdx serve` is running on `127.0.0.1:8000`, the order book panel can query it for the same pairs ("Local sdx server, ledger N"). Off by default; when the server is absent the panel says so. Nothing is ever simulated.

A docs site (`site/`, Astro + Starlight, re-themed with the kit tokens) for developers: overview, quickstart, CLI reference, HTTP API and Horizon differences, state format, differential check, status and limits.

## 3. Surface choice

Recommended surface kept: web app + docs site. The value is visual (two books side by side, an equality badge) for investors, and the product is a developer tool (CLI + Horizon-compatible API) for engineers.

## 4. Tasks this session

| # | Task | Ticket |
|---|---|---|
| 1 | Time-boxed SDX-012 investigation from recorded bodies + go-stellar-sdk source; record findings honestly | SDX-012 |
| 2 | `sdx serve`: CORS + exposed `Latest-Ledger` header so the browser live mode can read it; test | SDX-023 |
| 3 | `scripts/export-run-stats.py`: export bootstrap/replay stats from `data/state/` into `fixtures/runs/` with provenance | SDX-024 |
| 4 | `scripts/probe-pool.sh`: read-only Horizon probe of the dust pool; fixture with provenance | SDX-012 |
| 5 | `web/`: Vite + React + TS strict + Tailwind v4, kit tokens/components, `pnpm demo:data` from fixtures, tests | SDX-025 |
| 6 | `site/`: Astro + Starlight docs, kit theme | SDX-026 |
| 7 | DEMO.md, PITCH.md, DEPLOY.md, README status table, TASKS | SDX-027 |

## 5. Explicitly left out (and why)

- Path finding parity (SDX-012): cannot be re-tested without a state at the same ledger as a fresh Horizon response, and the state is 4 days behind. The surface shows the mismatch, not a workaround.
- Fresh live diff vs Horizon, re-bootstrap, long replay: forbidden by the session budget (multi-hour network jobs).
- Hosting, package publishing, release builds: later phase (DEPLOY.md lists them).
- SSE streaming, trade backfill, account-based path queries, protocol 30 XDR bump: SDX-016, -020, -021, -018 (unchanged).

## 6. Outcome (2026-11-08, end of session)

Done: tasks 1-7 above. SDX-012 was investigated but its cause was not found; the evidence is recorded in TASKS.md and shown in the demo. Details are in README "Status", DEMO.md and TASKS.md (SDX-023 to SDX-028).
