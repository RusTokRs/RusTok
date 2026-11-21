# forum 21a topic move owner

Status: source-ready / unvalidated.

This owner note records the source boundary checked by `verify-forum-topic-move-owner.mjs`. It is a maintainer handoff, not evidence of execution: tests, Cargo, formatting, and CI were not run by the implementation agent, and runtime execution or evidence admission remains maintainer-owned.

## Source contract

- Owner note for `forum 21a topic move owner`.
- Verifier: `scripts/verify/verify-forum-topic-move-owner.mjs`.

## Maintainer boundary

- Source markers and contract paths below are retained for maintainer review only.
- No runtime result, migration application, or admission decision is claimed here.
- `source_ready_maintainer_execution_pending`
- Idempotency contract
- One atomic owner transaction
- Semantic event and projection invalidation
- Persistence guards
- Regression coverage
- canonical FORUM-21 ledger entry is `in_progress`
- No command above was run by the implementation agent
