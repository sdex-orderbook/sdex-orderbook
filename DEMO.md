<!-- SPDX-License-Identifier: Apache-2.0 -->
# sdex-orderbook demo

Two surfaces, built from committed fixtures only (no network after the package install):

- **Web app** (`web/`): order books and trade aggregations rebuilt from ledger data, side by side with Horizon, plus the path-finding status, the bootstrap and replay evidence, and an optional live mode against a local `sdx serve`.
- **Docs site** (`site/`): sdx quickstart, CLI and HTTP API reference, state format, differential check, status and limits.

Every number on both comes from `fixtures/diff/` (two `sdx diff` runs against horizon.stellar.org, 2026-11-04), `fixtures/runs/` (bootstrap and replay statistics exported from the run logs) or `fixtures/probes/` (read-only Horizon probes, 2026-11-08). The only live read is the optional local-server panel, and it only runs when you press its button.

## 1. Requirements

- Node 24 (>= 22.18) and pnpm 10.22.0.
- Python 3 (only to serve the static files; any static server works).
- For the optional live mode: Rust 1.97.1 (pinned by `rust-toolchain.toml`) and a state directory in `data/state` (see step 5).

## 2. Build everything (clean checkout)

```sh
cd projects/sdex-orderbook
scripts/build-demo.sh            # pnpm install, demo:data, tests, web build, docs build -> demo-dist/
# offline, when the pnpm store already has the packages:
scripts/build-demo.sh --offline
```

What it runs, step by step:

```sh
cd web  && pnpm install --frozen-lockfile && pnpm demo:data && pnpm test && pnpm build && cd ..
cd site && pnpm install --frozen-lockfile && pnpm build && cd ..        # build also runs scripts/facts.mjs
rm -rf demo-dist && mkdir demo-dist && cp -r web/dist/. demo-dist/ && cp -r site/dist demo-dist/docs
```

`pnpm demo:data` aborts if its recount of any diff run disagrees with that run's `summary.json`.

## 3. Serve

```sh
python3 -m http.server 4173 --directory demo-dist
# web app:  http://localhost:4173/
# docs:     http://localhost:4173/docs/
```

For development: `cd web && pnpm dev` and `cd site && pnpm dev` (port 4321).

## 4. Investor demo script (3 to 5 minutes)

1. **Open http://localhost:4173/** (about 30 s). Point at the header badges: *Mainnet read-only*, *Unaudited*, *Path parity: not yet*. Say: "This rebuilds the Stellar DEX from raw ledger data, without Horizon, and checks itself against Horizon."
2. **Hero figure** (20 s). "12 of 12 order books identical to Horizon at the same ledger." Hover the provenance chip: recorded mainnet capture, 2026-11-04, `fixtures/diff`.
3. **Tiles and parity table** (30 s). 955,466 offers held; 1,472 ledgers replayed; 0 pre-image mismatches out of 525,948 checked; 159,171 trades derived. In the parity table: order book Pass 12 of 12, trade aggregations Pass 24 of 24, paths *Differs* 5 of 48. Say plainly that paths are not done.
4. **Click "Compare order books"** (45 s). The XLM / USDC case at ledger 65,305,917: left is our book, right is Horizon's response. Every row says *Same*; both SHA-256 values are equal. Pick another pair (for example AQUA or SHX) from the selector; press *Show all 20 levels*.
5. **Trade aggregations** (45 s). USDC / XLM, 1 minute, 99 buckets: candles computed from trades we derived from transaction results. Badge: 99 of 99 buckets identical. Hover a candle (OHLC, trade count, *Same* vs Horizon). Switch to *Table* to show our row and Horizon's row per bucket.
6. **Path finding** (45 s). The warning box says it does not match. Case 001: Horizon's best is 0.0008431 USDC, ours is 73.2089720 USDC through a thin 18/USDC pool. Scroll to the probe: four days later the pool had changed, Horizon routes through it, and our pool maths gives exactly Horizon's 0.9360584 USDC. "The maths is right; why Horizon ignored the pool on 11-04 is the open ticket."
7. **How the state was built** (30 s). Four steps; the integrity checks (bucket list hash matches the ledger header; 0 of 525,948 pre-images differ; per-bucket hash *Not run*, with the reason). Hover the replay chart; show the bucket chart (7.11 GB of the 9.62 GB is one level-10 bucket).
8. **Docs** (20 s). Click *Docs*: quickstart, CLI, HTTP API with every known Horizon difference.

## 5. Optional: live mode against your own server

Needs a state directory. Building one from scratch is a long, network-heavy job (about 82 minutes and 9.6 GB of bucket data on the recorded run), so do it before the meeting:

```sh
cargo build -p sik-sdex-orderbook-cli
target/debug/sdx bootstrap --out data/state            # long: history-archive buckets
target/debug/sdx replay --state data/state --to head   # Galexie lake to the network head
target/debug/sdx serve --state data/state --addr 127.0.0.1:8000
```

(On the build machine `data/state` already exists at ledger 65,306,175, 2026-11-04; loading it takes a few minutes with the debug build.)

Then in the web app, *Live mode with a local server* -> *Query local server*. The panel shows the book with a chip carrying the server's `Latest-Ledger`. The chip says *Live read* only if that ledger closed within 10 minutes of the read; a state a few days old (the usual case for the recorded `data/state`) is shown as *Recorded mainnet capture* with the ledger close date and its age, plus a "Not live" line; it only talks to 127.0.0.1 or localhost and only issues GETs. If no server answers, it says so and shows nothing else.

## 6. Checks before a demo

```sh
bash scripts/check.sh                         # Rust: fmt, clippy -D warnings, 51 offline tests
cd web && pnpm test && pnpm typecheck         # 12 data and library tests
node scripts/screenshot.mjs /tmp/sdx.png 1440 light   # needs google-chrome; optional
```
