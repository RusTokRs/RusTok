# forum moderation revision concurrency contract

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-moderation-revision-concurrency-postgres.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum moderation revision concurrency contract`.
- Verifier: `scripts/verify/verify-forum-moderation-revision-concurrency-postgres.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- Overlap construction
- Safe PostgreSQL outcomes
- owner_operation_receipts
- same stale reviewed revision
- Topic lock assertions
- Reply hide assertions
- fail-closed
- cargo test -p rustok-forum --test moderation_revision_concurrency_postgres
