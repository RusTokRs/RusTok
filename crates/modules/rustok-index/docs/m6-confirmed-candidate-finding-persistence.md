# M6 confirmed candidate finding persistence

Status: `source_complete_lifecycle_complete_targeted_repair_boundary_complete`.

## Purpose

`PostgresIndexDriftConfirmedCandidateWriter` persists one already confirmed stale-entity or
orphan-link inconsistency through the existing `index_consistency_findings` contract.

The adapter accepts only `IndexDriftConfirmedCandidate`. It cannot accept caller-provided check
names, digests, details JSON, finding UUIDs, timestamps, lifecycle state, actor identity, schedule,
cursor, or repair instruction.

## Transaction boundary

Every call opens one PostgreSQL `SERIALIZABLE READ WRITE` transaction.

The transaction performs this order:

1. derive the bounded finding request from typed confirmation evidence;
2. re-read and lock the exact materialized source identity/version;
3. for orphan links, re-read and lock the exact link identity and target state;
4. return `NotRecorded(MaterializedChanged)` without a write when the candidate no longer matches;
5. acquire the established deterministic finding-key advisory lock;
6. create, refresh, reopen, or suppress the finding through the existing finding table contract;
7. commit the materialized revalidation and finding write together.

A stale-entity row and an orphan source/link row are read with `FOR SHARE`. A present target row is
also read with `FOR SHARE`. An absent target remains a serializable predicate read. Storage or
serialization failures map to the retryable bounded `Storage` error without exposing SQL or database
causes.

## Deterministic identity and evidence

Missing-entity findings use check name `index.confirmed_missing_entity`, exact materialized entity
scope, and the established tenant/check/scope finding-key derivation.

Orphan-link findings use the exact source entity scope and:

`index.confirmed_orphan_link.<sha256>`

The suffix binds link name, ordinal, complete target identity, optional locale, and admitted target
absence version.

Expected and actual values are lowercase SHA-256 digests derived only from typed identity, indexed
source version, authoritative absence version, and separate expected/materialized state tags. The
persisted details value remains the bounded
`{ "contract": "index_drift_digest_finding_v1" }` (`index_drift_digest_finding_v1`) marker.

## Idempotent finding state

The adapter uses the same finding-key advisory-lock namespace and state semantics as the existing
Index writer:

- no row becomes `Created`;
- an open row becomes `Refreshed`;
- a resolved row becomes `Reopened`;
- an ignored row remains ignored and returns `Suppressed`.

Finding identity and `first_detected_at` are preserved. Unsupported stored identity or state fails
closed.

## Lifecycle and targeted repair follow-up

The separate lifecycle boundary provides authorization-gated resolve/ignore commands and immutable
audit. See `m6-drift-finding-lifecycle.md`.

The separate generic targeted-repair boundary now accepts a typed missing/orphan preimage only after
authorization. Its PostgreSQL reservation store reproduces this writer's exact check-name suffix,
finding-key derivation, and expected/actual evidence formulas before a repair can start. See
`m6-targeted-drift-repair.md`.

Repair does not mutate this writer's evidence and does not automatically resolve the finding. A new
detection may still reopen a resolved finding; ignored findings remain suppressed.

## Outputs and failures

The persistence output is only:

- `Recorded(IndexDriftFindingWriteOutcome)`; or
- `NotRecorded(MaterializedChanged)`.

Errors are a closed enum: unsupported backend, invalid evidence, retryable storage, or permanent
stored finding contract failure. No payload, SQL, database cause, actor, reason, or repair evidence is
returned.

## Composition and transport boundary

Persistence, lifecycle, and generic targeted repair remain unmounted from GraphQL, HTTP, CLI, MCP,
native admin, background workers, schedulers, and `ModuleRuntimeExtensions`.

## Deliberate limits

These slices still do not add:

- automatic candidate-page iteration or confirmation-to-write orchestration;
- public lifecycle or repair transport;
- a concrete repair evidence reader or mutation owner;
- prepared-repair recovery policy;
- retained PostgreSQL/concurrency execution evidence;
- shadow, full, or automatic repair.

## Next implementation step

Compose one concrete bounded repair evidence reader and one concrete idempotent owner for the
smallest supported confirmed finding kind. Keep automatic iteration and public transport separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_confirmed_candidate_writer -- --nocapture
cargo test -p rustok-index drift_finding_lifecycle -- --nocapture
cargo test -p rustok-index drift_repair -- --nocapture
node scripts/verify/verify-index-confirmed-candidate-persistence.mjs
node scripts/verify/verify-index-drift-finding-lifecycle.mjs
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite scenarios, workflows, or
CI were executed by the implementation agent.
