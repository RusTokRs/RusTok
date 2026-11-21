# forum moderation terminal concurrency contract

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-moderation-terminal-concurrency-postgres.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum moderation terminal concurrency contract`.
- Verifier: `scripts/verify/verify-forum-moderation-terminal-concurrency-postgres.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- Deterministic overlap
- RejectPublication race
- Remove + accepted solution race
- no-partial-effect guarantee
- forum.moderation_subject_revision_conflict
- cargo test -p rustok-forum --test moderation_terminal_concurrency_postgres
