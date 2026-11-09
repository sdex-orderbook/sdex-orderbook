<!-- SPDX-License-Identifier: Apache-2.0 -->
# sdex-orderbook: deployment checklist (later phase)

Status (2026-11-09): section 1 is deployed by the user (GitHub Pages from `.github/workflows/pages.yml`, repo `sdex-orderbook/sdex-orderbook`); the project-page base fix is committed but not yet verified live (SDX-031). Sections 2-4 are not done: the Render blueprint and VPS files exist but nothing is built or hosted (SDX-032), and the release workflow has not run (SDX-033). Agents do not publish, deploy or push (session rules); the user does.

## What must never happen

- **No mainnet writes, ever (G-VALUE).** sdx has no signing or submission code path. Keep it that way. A change that adds one is out of scope for this project.
- **No keys.** sdx needs none. No testnet contracts exist or are needed: this project deploys no contracts (testnet or mainnet).
- **No claim of path-finding parity** in any listing, README or marketing until SDX-012 closes and a fresh `sdx diff` shows it.
- **No "first" or "novel"** wording (G-NOVELTY). See PRIOR-ART.md.

## 1. Static demo and docs (web app + docs site)

| Item | Value |
|---|---|
| Build | `scripts/build-demo.sh` (web: `pnpm install --frozen-lockfile && pnpm demo:data && pnpm test && pnpm build`; site: `pnpm install --frozen-lockfile && pnpm build`) |
| Output | `demo-dist/` (web app at `/`, docs at `/docs/`) |
| Host | any static host (GitHub Pages, Cloudflare Pages, Netlify, S3 + CDN). No server code, no secrets. Current: GitHub Pages (workflow source), repo `sdex-orderbook/sdex-orderbook`, so a **project page**: app at `https://sdex-orderbook.github.io/sdex-orderbook/`, docs at `.../sdex-orderbook/docs/`. The bare root `https://sdex-orderbook.github.io/` does not serve the assets. A repo named `sdex-orderbook.github.io` would serve from the root and would not need `SDX_DOCS_BASE`. |
| Env vars | `VITE_DOCS_URL` (web, default `docs/`): docs link if the docs live elsewhere. `SDX_DOCS_BASE` (site, default `/docs`): base path of the docs site. Set it to `/` if the docs get their own domain. `pages.yml` sets `SDX_DOCS_BASE=/sdex-orderbook/docs` for the project-page path. |
| Data | `web/src/data/demo.json` and `site/src/data/facts.json` are generated from `fixtures/` at build time. Regenerate after new diff runs. |
| Before publishing | `cd web && pnpm test && pnpm typecheck`, then look at both themes and a 390 px width (`node scripts/screenshot.mjs`). |

The live panel only talks to `127.0.0.1` or `localhost`. On a hosted HTTPS page, browsers may block plain-HTTP requests to localhost. The panel then shows its "no local sdx server answered" state, which is correct behaviour.

## 2. Crates (crates.io)

| Crate | Path | Notes |
|---|---|---|
| `sik-sdex-orderbook-core` | `crates/core` | library: buckets, lake, state, trades, aggregations, order book, paths |
| `sik-sdex-orderbook-cli` | `crates/cli` | binary `sdx` |

Before `cargo publish`:

1. Set `version`, `description`, `repository`, `documentation`, `keywords` and `readme` in both `Cargo.toml`s. Keep `license = "Apache-2.0"`.
2. Make the CLI depend on the core crate by version, not only by path.
3. Ship `NOTICE` in both crates (`include`): go-stellar-sdk ports, Apache-2.0. Also add the root `NOTICE` and `docs/THIRD_PARTY.md` entries (cross-project request in TASKS.md).
4. Check that `stellar-xdr` is still pinned to `=28.0.0` (protocol 29). Bump it when protocol 30 reaches mainnet (SDX-018). Mainnet vote 2026-11-23 [U].
5. `bash scripts/check.sh`, then `cargo publish --dry-run -p sik-sdex-orderbook-core` and `-p sik-sdex-orderbook-cli`.
6. Measure a release build (`cargo build --release`) and record bootstrap and replay times. They are not measured yet.

## 3. Release binaries / GitHub release

- `.github/workflows/release.yml` builds `sdx` for x86_64-linux on a `v*` tag and attaches a tarball with a SHA-256 checksum (written 2026-11-09, never run: SDX-033). aarch64-apple-darwin is not covered.
- The release notes must state the parity table from README (order book and aggregations identical on recorded captures; paths do not match).

## 4. Hosted `sdx serve` (optional, after SDX-012)

| Item | Value |
|---|---|
| Command | `sdx bootstrap --out /data/state --cache /data/bucket-cache` once, then `sdx serve --state /data/state --addr 0.0.0.0:8000 --follow` |
| Resources | about 2 GB RAM minimum ([M] peak 1.83 GB bootstrap, 1.5 GB replay), 4 GB for headroom; disk for `/data` (state about 115 MB; bucket extract cache about 230 MB; optional lake cache), so 10 GB is plenty. The 9.6 GB of buckets is streamed, not stored |
| Network | outbound HTTPS to `history.stellar.org` (bootstrap) and `aws-public-blockchain.s3.amazonaws.com` (lake). No inbound except the API port. |
| Front | a reverse proxy with TLS and rate limiting. The built-in server is `std::net`, one thread per connection, GET only. It has 10 s socket timeouts, 8 KiB line and 64-header caps, at most 64 connections and 4 concurrent route computations (503 beyond that) (SDX-029), but no per-request deadline on `/paths`, no TLS and no per-client rate limit, and it answers `Access-Control-Allow-Origin: *`. It MUST NOT be exposed directly: run it bound to 127.0.0.1 behind a reverse proxy that adds TLS, per-client rate limits, a request timeout and a body/URL size limit. |
| VPS files | `deploy/sdx.service` (systemd, binds 127.0.0.1:8000) and `deploy/nginx-sdx.conf` (rate limit 5 req/s, 15 s timeouts; replace `api.example.org`, then certbot). Not tested on a host (SDX-032) |
| Render files | `Dockerfile`, `deploy/render-entrypoint.sh`, `deploy/placeholder.py`, `render.yaml` (Docker web service, persistent disk at `/var/lib/sdx`). First boot answers 503 JSON on `$PORT` while it runs `sdx bootstrap` (about 82 min) and `sdx replay --to head`; later boots go straight to `sdx serve --addr 0.0.0.0:$PORT --follow`. Render adds TLS but no rate limit or `/paths` deadline: put Cloudflare in front. Image tag `rust:1.97.1-bookworm` and the plan name are unverified; no image has been built (SDX-032). No `healthCheckPath` is set, because `GET /` is 503 during bootstrap |
| Health | `GET /` returns ledger, close time, offers, pools and `trades_cover_ledgers`. Alert if the ledger lags the network. |
| Not to expose | `sdx diff` against Horizon from a public box: it is a manual, rate-limited check (>= 600 ms between Horizon requests). |

## 5. Scheduled evidence (after hosting)

- Nightly `sdx diff` with a fixed case set and a trend file (SDX-015). Its outputs go to `fixtures/diff/<UTC stamp>/` with `diff.provenance.json`.
- Re-export run statistics after a new bootstrap or replay: `python3 -I scripts/export-run-stats.py --state data/state --out fixtures/runs/<name>`.

## 6. Testnet

Not applicable. sdx reads mainnet only. A testnet mode would need `--archive` and `--lake` pointing at testnet sources and has not been tried.
