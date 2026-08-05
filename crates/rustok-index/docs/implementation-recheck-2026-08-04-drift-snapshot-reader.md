# `rustok-index` implementation recheck — 2026-08-04 drift snapshot reader

Audited baseline: `main@5da25b28be5e1bf4f9cd9802337a3efa560179a4`.

## Rechecked cursor

PR #2972 completed locale-optional finding persistence. The next open plan item was the first
production `IndexDriftSnapshotReader` for one exact entity under a truthful consistency boundary.

The existing owner source registry already provides bounded targeted loads and retained positive
source versions, while materialized Index state is stored in PostgreSQL `index_entities` and
`index_links`. The source trait does not accept a caller-owned transaction, so claiming one shared
PostgreSQL snapshot across arbitrary owner adapters would be false.

## Selected boundary

This slice uses a source-version fence instead of inventing cross-adapter transaction semantics:

- exact owner state before the materialized read;
- one PostgreSQL `REPEATABLE READ READ ONLY` materialized snapshot;
- exact owner state again while that transaction remains open;
- acceptance only when the complete typed owner state is unchanged and has a positive version.

The opaque boundary binds the PostgreSQL snapshot token and accepted typed owner state. Missing
source output remains rejected until a tombstone or another explicit absence watermark exists.

## Source completed in this slice

- production `PostgresIndexDriftSnapshotReader`;
- immutable source/schema registry composition boundary;
- exact materialized entity, payload, delete, and link reconstruction;
- registered schema-fingerprint and link-order validation;
- bounded retryable/permanent dependency classifications;
- source-version change detection and unwatermarked-absence rejection;
- environment-gated real-migration PostgreSQL harness;
- architecture documentation, plan actualization, and static verifier coverage.

## Deliberate limits

This recheck does not claim or add:

- server operator or public transport composition;
- an exact-entity command that invokes producer and writer;
- source absence without a retained positive-version delete;
- discovery scans, orphan diagnosis, or automatic finding closure;
- resolve/ignore commands or actor/reason audit;
- targeted/full/shadow repair;
- retained execution evidence.

## Next cursor

Compose the reader, existing digest producer, and finding writer inside the guarded server operator
for one authorized exact `EntityKey`. Keep discovery, lifecycle commands, and repair outside that
slice. Add an explicit owner absence-watermark contract before treating an empty targeted load as
truthful `Missing`.

## Verification ownership

The implementation agent did not run formatting, Cargo checks, tests, JavaScript verifiers,
PostgreSQL scenarios, workflows, or CI. Owner commands are listed in
`m6-postgres-drift-snapshot-reader.md` and the current plan overlay.
