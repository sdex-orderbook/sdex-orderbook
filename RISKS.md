# Risks, gates, open questions

## Demand and absorb risks

| Risk | Evidence | Mitigation / what would change our mind |
|---|---|---|
| SDEX volume too small to matter | Register note: Aquarius ~99% of Stellar DEX volume [S: IDEA_REGISTER, unclear whether SDEX was counted]. Counter-evidence: Stellar Expert reports 47,883 XLM/USDC trades and 4.64 M XLM base volume in 24 h on 2026-11-03 [V] (SDEX + classic AMM; Stellar Expert does not say whether it counts path-payment hops) | Measure SDEX share from our own derived trades over a week (SDX-017) before investing further |
| SDF keeps Horizon alive indefinitely, so the gap never opens | Horizon v30.0.0 shipped 2026-10-21 [S] | Position as a drop-in for Horizon users who self-host or leave Horizon; diff evidence makes switching cheap |
| Routers keep private graphs (StellarBroker, Soroswap aggregator, LumAgg, WOWMAX) | [S: IDEA_REGISTER D] | We target Horizon `/paths` users (wallets, StellarRoute, Stellar Pathfinder), not routers |
| SDF builds an RPC-side replacement | None seen [U] | Watch stellar-rpc and stellar-docs; offer the code upstream (drafts only, G-OUTREACH) |

## Technical risks

| Risk | Status |
|---|---|
| Bucket early exit relies on bucket entries being sorted by entry type first | Holds for stellar-core's `LedgerEntryIdCmp` [I]; checked empirically by the replay pre-image checks (README) |
| Early-exited buckets are not hash-verified | The HAS is checked against the ledger header's `bucket_list_hash` (lake), and every replayed STATE pre-image of an offer/pool is compared with our state; a corrupted prefix would show as mismatches. A full-read mode (`early_exit=false`) exists in the library |
| `stellar-xdr =28.0.0` cannot decode protocol 30 meta | Protocol 30 mainnet vote 2026-11-23 [U: discovery notes]. Bump the pin when P30 lands (SDX-018) |
| Memory: holding full `LedgerEntry` per offer | Measured peak RSS in README; switch to compact records if it grows |
| Horizon behaviour drifts (it is maintained for protocol releases) | Port pinned to go-stellar-sdk f8efc493fca9; `sdx diff` re-run detects drift |
| Path results differ in ties (edge order, unstable sort) | Documented SPEC 5.4; diff reports `equal_ignoring_order` and `best_equal` separately |
| Path results differ beyond ties (measured 2026-11-04) | horizon.stellar.org does not use some classic pools; we do, and our answers then route through dust pools. Without pools we miss some Horizon multi-hop paths. Unexplained; SDX-012. `/paths` is not usable as a Horizon replacement until this closes |
| Bootstrap cost | ~82 min cold on a loaded box, mostly downloading classic accounts/trustlines; archive streams drop (Range resume added) |
| Trade aggregation covers only replayed ledgers | `GET /` reports `trades_cover_ledgers`; backfill ticket SDX-016 |
| `sdx serve` exposed to a network | Std-only server: timeouts, line/header caps, connection and in-flight caps (SDX-029) but no `/paths` deadline, no TLS, no per-client rate limit, CORS `*`. A hostile client can still make expensive `/paths` requests (503 beyond 4 at once) and any web page can read a loopback server. Local use only; reverse proxy with TLS, rate limits and timeouts before any hosting (DEPLOY.md 4) |
| Lake lag behind the network | Replay waits up to 120 s per missing object |

## Gates touched

- G-NOVELTY: no "first"/"novel" claims. `PRIOR-ART.md` dated 2026-11-03.
- G-LICENCE: Apache-2.0 ports from go-stellar-sdk recorded in project `NOTICE`. Root `NOTICE` and `docs/THIRD_PARTY.md` must also list them: cross-project request in `TASKS.md` (agents may not edit root docs this session).
- G-OUTREACH: nothing posted. Any upstream offer (e.g. to js-stellar-sdk#1620 or StellarRoute) would be drafted in `drafts/`.
- G-VALUE, G-SECRETS: not touched (read-only, no keys).
- G-ZK, G-DTCC, G-CRYPTO, G-LEGAL: not touched.

## Open questions

1. Is Horizon's `/order_book` response guaranteed to be from the ledger in its `Latest-Ledger` header? (The header comes from the ledger-state cache, the body from a repeatable-read transaction.) The diff records a previous-ledger comparison to detect off-by-one cases.
2. Do third-party history archives serve buckets faster than SDF's (0.61 MB/s single stream measured)?
3. Would wallets accept a service without `source_account` support, or is a trustline index (bootstrapped the same way, types 0-1) needed?
