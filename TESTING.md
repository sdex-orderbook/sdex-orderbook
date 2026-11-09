# Testing

## Commands

```
cd projects/sdex-orderbook
bash scripts/check.sh          # fmt --check, clippy -D warnings, cargo test (offline)
cargo test --workspace --offline
```

All default tests are offline. Nothing in `cargo test` or `pnpm test` touches the network.

```
cd web && pnpm install && pnpm demo:data && pnpm test && pnpm typecheck && pnpm build
cd site && pnpm install && pnpm build
```

## Layers

| Layer | Where | What it pins |
|---|---|---|
| Unit: arithmetic | `price.rs`, `pool_math.rs`, `amount.rs` | Go `price` rounding, pool payout/expectation and slippage, Horizon amount strings, `FloatString(7)`, Postgres-avg emulation (vectors from a captured Horizon record) |
| Unit: model | `state.rs`, `book.rs`, `paths.rs`, `aggregate.rs`, `snapshot.rs`, `bucket.rs`, `lake.rs`, `horizon.rs`, `trades.rs` | book order (update moves behind equal price), level grouping and bid inversion, strict-send/receive on a synthetic graph (book vs pool choice, two hops, balance validation, ignored seller), Horizon path sort/filter, minute rows/orientation/tie rule, bucket windowing/limit/order, bucket shadowing and early exit, extract cache, state save/load + tamper detection, query parsing |
| Fixture: real buckets | `crates/core/tests/fixtures.rs` | level-0 live buckets of a mainnet checkpoint read to EOF with SHA-256 verified against their names |
| Fixture: real ledgers | `crates/core/tests/fixtures.rs` | Galexie objects decode; derived trades equal Horizon `/ledgers/N/trades` for the same ledgers (amounts, assets, price n/d, base/counter) |
| Fixture: diff | `crates/core/tests/fixtures.rs` | every recorded order-book case is recomputed from its stored offers (XDR) and must reproduce our recorded answer; cases recorded `equal` must still equal Horizon's body. Trade-aggregation cases are recomputed from their stored trades the same way |
| CLI / HTTP | `crates/cli/tests/cli.rs` | the `sdx` binary on a synthetic state dir: order-book and strict-send output, exit code 2 on bad input, HTTP server status codes, `Latest-Ledger` and CORS headers, 400 for `source_account` |
| Unit: probe vector | `pool_math.rs` `payout_matches_horizon_probe_2026_11_08` | CAP-38 payout equals Horizon's direct strict-send record for pool 05442dce... (`fixtures/probes/20261108-pool-18-usdc`) |
| Web: data | `web/test/data.test.ts` (`cd web && pnpm test`) | `src/data/demo.json` re-checked against every `fixtures/diff/*/summary.json`; README figures (12/12, 24/24, 5/48, 12/24, 1,472 ledgers, 525,948 pre-images, 0 mismatches); hashes equal exactly when a case is recorded equal |
| Web: library | `web/test/lib.test.ts` | asset parsing, Horizon query strings, live mode only for loopback, ledger taken from `Latest-Ledger`, refuses a server that is not sdx |

Synthetic data is labelled synthetic in each test. Real data lives only under `fixtures/` with a `*.provenance.json` beside it.

## Fixtures and how to recapture

| Path | Captured by | Contents |
|---|---|---|
| `fixtures/buckets/` | `scripts/capture-fixtures.sh buckets <checkpoint>` | HAS JSON + level-0 curr/snap buckets (`.xdr.gz`) |
| `fixtures/lake/` | `scripts/capture-fixtures.sh lake <first> <count>` (count <= 10) | `mirror/` in the lake layout (usable as `--lake file://.../mirror`), `horizon-trades-<seq>.json` |
| `fixtures/diff/<run>/` | `sdx diff --state <dir> --out fixtures/diff/<run>`, then `zstd -19 --rm fixtures/diff/<run>/cases/*.json` (case files are read compressed) | `cases.json`, `summary.json`, `cases/NNN-<kind>.json` (request, ledger, Horizon body, our body, comparison, order-book offers as hex XDR, aggregation trades), `diff.provenance.json` |

| `fixtures/runs/<name>/` | `python3 -I scripts/export-run-stats.py --state data/state --out fixtures/runs/<name>` | `run-stats.json` (bootstrap per-bucket stats, HAS check, de-duplicated per-ledger replay stats), `run.provenance.json` (input SHA-256s) |
| `fixtures/probes/<name>/` | `scripts/probe-pool.sh <A> <B> <amount> fixtures/probes/<name>` | Horizon `/liquidity_pools?reserves=`, single-hop `/paths/strict-send`, `/order_book`, bodies and headers verbatim, `probe.provenance.json` |

Rules: never hand-edit a fixture; recapture. No private data or keys (all inputs are public mainnet data).

## Live checks (manual, read-only)

```
sdx bootstrap --out data/state                      # history archive, minutes to an hour
sdx replay --state data/state --to head             # Galexie lake
sdx diff --state data/state --out fixtures/diff/$(date -u +%Y%m%dT%H%MZ) --auto 8
```

Rate limits: >= 600 ms between Horizon requests (built in), <= 4 concurrent archive/lake downloads.

## Mutation-style checks to run by hand before changing ported code

- Flip `>=` to `>` in `pool_math::calculate_pool_expectation` (`disbursed >= reserve_b`): `expectation_basic` must fail.
- Remove the bid inversion in `book.rs`: `groups_levels_and_inverts_bids` and the diff fixture test must fail.
- Change `positive_min` to plain `min`: `strict_receive_direct_and_balance` must fail.
- Make `Row::merge` keep the earlier tie: `tie_keeps_later_representation` must fail.
