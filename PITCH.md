<!-- SPDX-License-Identifier: Apache-2.0 -->
# sdex-orderbook (sdx): investor brief

Tags: [P] primary source, [M] measured by this project, [S] secondary, [V] vendor, [E] estimate, [U] unverified. Dates are when read or measured.

## Problem

- Stellar's DEX data (order books, payment paths, OHLC trade aggregations) is served publicly by Horizon. SDF's docs say: "Horizon is nearing end-of-life and will eventually be deprecated in favor of Stellar RPC and Portfolio APIs ... it won't receive new feature development." [P: developers.stellar.org/docs/data/apis, read 2026-11-08]
- SDF's Horizon-to-RPC endpoint mapping lists no RPC method for order books, payment paths or trade aggregations. [P: developers.stellar.org/docs/data/apis/migrate-from-horizon-to-rpc, endpoint mapping table, read 2026-11-08]
- Builders still depend on those endpoints. StellarRoute's indexer syncs SDEX order books from Horizon, and Stellar Pathfinder calls Horizon's strict-send API. [P: their READMEs, 2026-11-03]
- Hosted alternatives are partial or closed. Stellar Expert's candles endpoint returns HTTP 402. Routers such as StellarBroker keep their routing graphs private. [P: probed 2026-11-03]
- The market is active. In the 1,472 mainnet ledgers we replayed (about 2 hours on 2026-11-04), the network produced 159,171 trades. These cover all pairs and include path-payment hops and classic pool trades. [M]

## What it does

`sdx` rebuilds the SDEX from ledger data alone. It bootstraps offers and classic pools from history-archive buckets, then replays every ledger from the public Galexie lake. It derives trades from transaction results and serves Horizon's `/order_book`, `/trade_aggregations` and `/paths` request and response shapes. It is a Rust library, a CLI and a local HTTP server. It is read-only, and anyone can run and audit it. A differential checker (`sdx diff`) proves each answer against Horizon at the same ledger.

## Who it is for

- Wallets, SDKs and market-data services that call Horizon's DEX endpoints today. They can switch base URL to a server they run themselves.
- Teams moving to RPC that still need DEX data.
- Auditors and analysts who want DEX numbers checked against the ledger, not taken from an API.

## Working today (measured, mainnet, read-only)

| Claim | Number | Evidence |
|---|---|---|
| Order books identical to horizon.stellar.org at the same ledger | **12 of 12** (8 pairs, 20 levels per side) | [M] `fixtures/diff/`, 2026-11-04 |
| Trade-aggregation windows identical (1 and 5 min, every field) | **24 of 24** | [M] same |
| Ledgers replayed; offer and pool pre-images checked; mismatches | 1,472; 525,948; **0** | [M] `fixtures/runs/` |
| State at the last replayed ledger (65,306,175) | 955,466 offers, 40,273 classic pools | [M] |
| Bootstrap: bucket-list hash vs checkpoint ledger header | match (checkpoint 65,304,703) | [M] |
| Bootstrap cost (cold, shared 4-core box, debug build) | about 82 min; 9.62 GB read; 1.83 GB peak memory | [M] README |
| Path finding identical to Horizon | **5 of 48: not working yet** | [M] SDX-012 |

The web demo shows every case side by side with Horizon, including the ones that fail.

## Why Stellar

Stellar has a native order book and classic AMMs at the protocol level. The data needed to rebuild them is public and verifiable: content-addressed history-archive buckets, a hash-chained bucket list in each ledger header, and a public per-ledger metadata lake on AWS. A third party can therefore reproduce Horizon's DEX views with no privileged access, and prove it ledger by ledger. That is what `sdx diff` does.

## Roadmap

1. **Path parity (SDX-012).** Find why Horizon ignored some classic pools on 2026-11-04. A follow-up probe on 2026-11-08 shows our pool maths matches Horizon exactly once Horizon routes through such a pool. Then run a nightly diff with a trend file (SDX-015).
2. **Hardening.** Full-read bucket hashes (SDX-013), stable equal-price order across restarts (SDX-019), a release build and a `--follow` soak test.
3. **Coverage.** Trade backfill before the bootstrap checkpoint (SDX-016), `source_account` support through a trustline index (SDX-020), SSE streaming (SDX-021), and protocol 30 XDR when mainnet upgrades (SDX-018).
4. **Adoption evidence.** Measure the SDEX share of DEX volume from derived trades (SDX-017). Draft, not post, base-URL switch examples for Horizon clients (SDX-022, user-led outreach).

## Risks and gates

- **Paths are not usable yet.** This is shown in the product, not hidden.
- **Demand.** SDEX volume relative to Soroban AMMs is unmeasured ([S] notes suggest AMMs dominate). SDX-017 measures it before more investment. SDF may also extend Horizon's life or build an RPC-side replacement ([U]).
- **Unaudited. Read-only on mainnet (G-VALUE).** There is no signing or submission path. No outreach is done by agents (G-OUTREACH). There are no novelty claims; see PRIOR-ART.md (2026-11-03): no open Horizon-free equivalent was found, and there are partial overlaps for trades and OHLCV.
- **Bootstrap cost and lake dependency.** About 82 minutes cold. The lake is a public AWS bucket; any archive or lake mirror can be used (`file://` works).

## What launch needs

- **Hosting:** a static host for the demo and docs, and one server or container for `sdx serve --follow`. It needs a few GB of disk for bucket and lake caches and about 2 GB of RAM ([M] peak 1.83 GB).
- **Packages:** crates.io publish of `sik-sdex-orderbook-core` and `sik-sdex-orderbook-cli`, and release binaries. See DEPLOY.md.
- **Audit:** an external review of the replay and trade-derivation logic and of the Horizon-compatibility claims. Path parity must be fixed first.
- **Licence housekeeping:** go-stellar-sdk ports are attributed in NOTICE (Apache-2.0). The root NOTICE entry is pending (cross-project request).
