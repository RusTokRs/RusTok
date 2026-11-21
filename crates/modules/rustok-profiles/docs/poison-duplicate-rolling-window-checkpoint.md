# poison duplicate rolling window checkpoint

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-iggy-dlq-duplicate-rolling-window.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `poison duplicate rolling window checkpoint`.
- Verifier: `scripts/verify/verify-iggy-dlq-duplicate-rolling-window.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- required marker: Profiles never authorizes
- required marker: cross-cycle
- required marker: history_truncated
- required marker: process-local per-partition cursor
- required marker: moving scanner and server integration are source-complete
