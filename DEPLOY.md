<!-- SPDX-License-Identifier: Apache-2.0 -->
# sdex-orderbook: deployment checklist (later phase)

Status (2026-11-09): section 1 is deployed by the user (GitHub Pages from `.github/workflows/pages.yml`, repo `sdex-orderbook/sdex-orderbook`); the project-page base fix is committed but not yet verified live (SDX-031). Sections 2-4 are not done: the VPS files exist (target host: Oracle Cloud Always Free ARM) but nothing is hosted (SDX-032), and the release workflow has not run (SDX-033). Agents do not publish, deploy or push (session rules); the user does.

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
| Host | Oracle Cloud Always Free, Ampere A1 Arm VM (`VM.Standard.A1.Flex`, Ubuntu aarch64), no cost. Free-tier limits are from memory, check Oracle's current terms; the home region is chosen at signup and cannot be changed, and Arm capacity can be unavailable in some regions. Render's free plan was rejected: 512 MB RAM (bootstrap peaks at 1.83 GB), no persistent disk, sleeps when idle |
| Oracle setup | see "Oracle setup" below |
| Health | `GET /` returns ledger, close time, offers, pools and `trades_cover_ledgers`. Alert if the ledger lags the network. |
| Not to expose | `sdx diff` against Horizon from a public box: it is a manual, rate-limited check (>= 600 ms between Horizon requests). |

### Oracle setup (SDX-032; written 2026-11-09, not yet run)

1. Create the Always Free account and a `VM.Standard.A1.Flex` instance (Ubuntu, 2 OCPU / 8 GB is enough; 4 / 24 is the free maximum). Add your SSH key.
2. Network: in the VCN security list add ingress for TCP 80 and 443. The Ubuntu image also ships host firewall rules that reject them: allow both with `iptables` (and persist) before certbot.
3. DNS: point a name (a subdomain, or a free DuckDNS name) at the instance's public IP.
4. Build on the box (Arm, so no release binary from `release.yml`):
   ```
   sudo apt-get update && sudo apt-get install -y build-essential pkg-config git nginx certbot python3-certbot-nginx
   curl https://sh.rustup.rs -sSf | sh -s -- -y && . ~/.cargo/env
   git clone https://github.com/sdex-orderbook/sdex-orderbook.git && cd sdex-orderbook
   cargo build --release --locked -p sik-sdex-orderbook-cli     # rust-toolchain.toml pins 1.97.1
   sudo useradd -r -m sdx && sudo install -d -o sdx /opt/sdx /var/lib/sdx
   sudo install target/release/sdx /opt/sdx/sdx
   ```
5. State, once (about 82 min bootstrap, then catch-up), in `tmux`, as the `sdx` user: `sdx bootstrap --out /var/lib/sdx/state` then `sdx replay --state /var/lib/sdx/state --to head`. Respect the limits: at most 4 concurrent downloads.
6. Service: `sudo cp deploy/sdx.service /etc/systemd/system/ && sudo systemctl enable --now sdx`.
7. Proxy and TLS: copy `deploy/nginx-sdx.conf` to `/etc/nginx/conf.d/sdx.conf`, set `server_name`, `sudo nginx -t && sudo systemctl reload nginx`, then `sudo certbot --nginx -d <name>`.
8. Smoke test: `curl 'https://<name>/order_book?selling_asset_type=native&buying_asset_type=credit_alphanum4&buying_asset_code=USDC&buying_asset_issuer=GA5ZSEJYB37JRC5AVCIA5MOP4RHTM335X2KGX3IHOJAPP5RE34K4KZVN'` returns a book with a recent `Latest-Ledger`. Then record the URL in README and set SDX-032 to DONE.

## 5. Scheduled evidence (after hosting)

- Nightly `sdx diff` with a fixed case set and a trend file (SDX-015). Its outputs go to `fixtures/diff/<UTC stamp>/` with `diff.provenance.json`.
- Re-export run statistics after a new bootstrap or replay: `python3 -I scripts/export-run-stats.py --state data/state --out fixtures/runs/<name>`.

## 6. Testnet

Not applicable. sdx reads mainnet only. A testnet mode would need `--archive` and `--lake` pointing at testnet sources and has not been tried.
