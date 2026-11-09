# sdex-orderbook: instructions for agents

Follow `docs/AGENT_GUIDE.md` (programme root) and `docs/SESSION_BUILD_2026-11-03.md`, then read `README.md`, `SPEC.md`, `ARCHITECTURE.md`, `TASKS.md`, `TESTING.md`, `RISKS.md`, `PRIOR-ART.md`, `SOURCES.md`.

1. Work only inside `projects/sdex-orderbook/`. Cross-project needs go under "Cross-project requests" in `TASKS.md`.
2. Take the lowest-numbered `TODO` ticket whose dependencies are `DONE`; mark `IN-PROGRESS(date)` then `DONE(date, note)`.
3. Horizon compatibility is the contract. Any behaviour change in `book.rs`, `paths.rs`, `aggregate.rs` or `horizon.rs` must cite the Horizon/go-stellar-sdk source it follows (file + commit) and keep `cargo test` green on the captured diff fixtures. New differences go into SPEC 5.4/5.5.
4. Ported code (`paths.rs`, `pool_math.rs`, `price.rs`) stays attributed in `NOTICE`. Do not copy other third-party code (G-LICENCE).
5. Network is read-only: history archive, Galexie lake, Horizon GETs. Keep >= 600 ms between Horizon requests and <= 4 concurrent lake/archive downloads. Never sign or send a transaction.
6. Fixtures are captured by `scripts/capture-fixtures.sh` with `*.provenance.json` beside them. No hand-edited fixtures. Default tests are offline.
7. No TODO without a ticket id (`SDX-NNN`). Apache-2.0 SPDX header in every source file.
8. Do not call anything "first" or "novel" (G-NOVELTY); `PRIOR-ART.md` is the record.
9. Do not commit; the user commits.
