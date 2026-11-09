# sdex-orderbook architecture

## Language: Rust

Chosen over TypeScript for three reasons:

1. **XDR volume.** Bootstrap streams gigabytes of bucket XDR and replay decodes every `LedgerCloseMeta`. The programme measured JS XDR decode at 4-8 s per ledger in the activity ledger [S: `docs/ARCHITECTURE.md` language policy]; a mainnet ledger closes every ~5-6 s, so a TS replayer could not keep up. `stellar-xdr` (Rust) decodes a ledger in milliseconds (measured numbers in README).
2. **Exact integer maths.** Path finding and pool maths need 128- and 256-bit intermediates (stellar-core rounding); Rust has `u128` and `ethnum::U256` without boxing.
3. **Precedent.** `holders-at-ledger` already reads the same Galexie lake in Rust with `stellar-xdr =28.0.0`; we use the same pin (decodes protocol 29 meta).

No third-party web framework: the HTTP server is ~120 lines on `std::net` (local, read-only, GET only), which keeps the dependency set to `stellar-xdr`, `stellar-strkey`, `flate2`, `zstd`, `ureq`, `ethnum`, `serde`, `sha2`, `clap`.

## Bootstrap choice: history-archive buckets (+ lake for replay)

Options considered (task brief item 1):

| Option | Horizon-free? | Cost | Verdict |
|---|---|---|---|
| History-archive bucket list at a checkpoint | yes (SDF archive or any third-party archive; content-addressed, hash-checked) | 4.83 GB compressed for the full live list at checkpoint 65,290,111 [M: HEAD of 21 buckets, 2026-11-03]; with early exit we download only the classic prefix of each bucket | **chosen** |
| Galexie lake from genesis | yes | replaying ~65 M ledgers; infeasible for a first version | rejected |
| Galexie lake + an initial snapshot from Horizon `/offers` | no (Horizon is what we replace) | cheap | rejected for the product path |
| Hubble BigQuery offers table | no login allowed for agents; not ledger-exact by default | | rejected |
| RPC `getLedgerEntries` | needs every offer key in advance; no enumeration | | impossible |

So: buckets give the state at checkpoint C; the lake gives `LedgerCloseMeta` for C+1, C+2, ... and keeps it current. `sdx bootstrap` checks the HAS against the checkpoint ledger header's `bucket_list_hash` from the lake (`bucket::bucket_list_hash`); it matched at 65,304,703 [M].

Key trick: bucket entries are sorted by ledger key and the key's first field is the entry type, so in every bucket offers (type 2) and pools (type 5) come before all Soroban state (types 6-9). We stop reading each bucket at the first type > 5 and never download contract data. Measured effect (README): small. Classic accounts and trustlines are 88% of live records and sit before offers, so ~90% of the largest bucket is still downloaded [M, 2026-11-04]. The real saving is the content-addressed extract cache: a later checkpoint re-downloads only changed buckets. A cheaper bootstrap would need a source that is not sorted with accounts first (open question).

## Modules (`crates/core`, lib `sdx_core`)

```
asset.rs      canonical asset strings (go-stellar-sdk Asset.String form), Horizon param parsing, JSON fields
amount.rs     stroops <-> "12.3456789"
price.rs      exact rationals; ConvertToBuyingUnits & friends (port); FloatString(7); Postgres-avg emulation
pool_math.rs  constant-product payout/expectation + rounding slippage (port, U256)
state.rs      BookState: offers (HashMap by id) + per-pair BTreeSet books + adjacency; pools by id/pair
bucket.rs     HAS fetch, bucket streaming with peek/early-exit, shadowing assembly, extract cache, bootstrap()
lake.rs       Galexie object keys, HTTP GET helpers (ureq, retries), parallel fetch, zstd+XDR decode
ledger.rs     LedgerView over LCM V0/V1/V2; apply() in core apply order with STATE pre-image checks
trades.rs     ClaimAtom -> Trade (Horizon trade-processor rules), TOID
aggregate.rs  minute rows, orientation, Horizon bucket/page algorithm, JSON
book.rs       /order_book levels
paths.rs      strict-send / strict-receive search (port of exp/orderbook)
horizon.rs    query parsing, validation, Horizon response shapes, problem JSON
snapshot.rs   state directory save/load (sha256-checked), trades.jsonl, replay.jsonl
```

`crates/cli` (bin `sdx`): `app.rs` (state + minute rows + replay loop), `http.rs` (server + pure `route()`), `diff.rs` (Horizon differential checker), `main.rs` (commands).

## Data flow

```
history archive ──HAS──► bucket::bootstrap ──extracts (cached)──► assemble ──► BookState@C ──► state dir
                                                                                   │
Galexie lake ──LCM C+1..N──► ledger::apply ──changes──► BookState@N ──────────────►┤ save every 256 ledgers
                                   └──results──► trades::from_view ──► trades.jsonl + aggregate::Minutes
                                                                                   │
         sdx serve / CLI ◄── horizon.rs (book.rs, paths.rs, aggregate.rs) ◄────────┘
         sdx diff ── same answers vs horizon.stellar.org at Latest-Ledger ──► fixtures/diff/<run>/
```

## Decisions

1. **Port, do not reinvent, the path engine.** Horizon's semantics are the compatibility target, so we translated go-stellar-sdk's `search` line by line (Apache-2.0, NOTICE) rather than writing a "better" router.
2. **Hold full `LedgerEntry` values.** Costs memory (README has the measured peak) but lets every STATE pre-image in replay be compared byte-for-byte with our state; that is the strongest offline correctness check of the bootstrap.
3. **Trades are derived, not fetched.** `/trade_aggregations` only covers ledgers we replayed; a backfill is a later task (SDX-016), since the lake has every ledger.
4. **No accounts/trustlines.** Keeping the classic account state would multiply bootstrap size (accounts and trustlines are most of the classic prefix). So `source_account`/`destination_account` are refused (SPEC 5.4).
5. **Deterministic ordering.** Books are BTreeSets, adjacency is sorted, state files are written in id order.

## Not done in v1

Soroban AMMs; account-aware path parameters; SSE streaming; trade backfill before the bootstrap checkpoint; HAS-vs-ledger-header check wiring; persistence of book insertion order across restarts.
