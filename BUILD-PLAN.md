# Build plan

Estimates are agent estimates [E] (elapsed agent time, not measured unless marked [M]).

## M1. First working version (this session, 2026-11-03)

Acceptance:
- `sdx bootstrap` builds offers + pools at a mainnet checkpoint from history-archive buckets; time, bytes and peak memory recorded [M].
- `sdx replay` applies Galexie ledgers with zero STATE pre-image mismatches over the replayed range [M].
- `sdx order-book`, `strict-send`, `strict-receive`, `trade-aggregations` and `sdx serve` answer with Horizon shapes.
- `sdx diff` compares against horizon.stellar.org at matching ledgers; results and mismatches stored as fixtures [M].
- Offline tests over captured fixtures with provenance; `scripts/check.sh` green.

Status: see README "Status".

## M2. Correctness hardening (~1 week [E])

- Explain or fix every mismatch class in the M1 diff (SDX-012).
- Nightly diff run with a fixed case set (SDX-015), trend file.
- Full-read bucket mode on a schedule to hash-verify every bucket (SDX-013).
- Persist book insertion order so restarts keep Horizon's equal-price order (SDX-019).

## M3. Coverage (~1-2 weeks [E])

- Trade backfill: derive trades for any past ledger range from the lake (SDX-016).
- Optional trustline index (bootstrap entry types 0-1 for chosen accounts or all) to support `source_account` / `destination_account` (SDX-020).
- SSE streaming for `/order_book` (SDX-021).
- Protocol 30 XDR pin bump when mainnet upgrades (SDX-018).

## M4. Adoption evidence (user-led)

- SDEX share measurement from derived trades (SDX-017) to answer the "volume too small" risk.
- Drafts (not posted) for js-stellar-sdk#1620 and StellarRoute showing a base-URL switch (SDX-022).
