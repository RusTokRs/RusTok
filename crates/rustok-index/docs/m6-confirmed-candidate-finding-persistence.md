# M6 confirmed candidate finding persistence

Status: `source_complete_lifecycle_complete_repair_pending`.

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

## Deterministic identity

Missing-entity findings use:

- check name `index.confirmed_missing_entity`;
- the exact materialized entity finding scope;
- the existing tenant/check/scope finding-key derivation.

Orphan-link findings use the exact source entity scope and a bounded check name:

`index.confirmed_orphan_link.<sha256>`

The SHA-256 identity binds link name, ordinal, complete target identity, optional locale, and
admitted target absence version. Different orphan identities therefore cannot collapse to the same
finding key under one source entity scope.

## Evidence digests

Expected and actual digests are lowercase SHA-256 values derived only from typed fields.
Length-prefixed components and separate domain/state tags bind exact identity, indexed source
version, authoritative absence version, and expected versus materialized state.

No caller evidence blob or raw owner/index record is accepted or persisted. The persisted `details`
value remains the established bounded `{ "contract": "index_drift_digest_finding_v1" }` marker.

## Idempotent state behavior

The adapter uses the same finding-key advisory-lock namespace and state semantics as the existing
Index finding writer:

- no row becomes `Created`;
- an open row becomes `Refreshed`;
- a resolved row becomes `Reopened`;
- an ignored row remains ignored and returns `Suppressed`.

Finding identity and `first_detected_at` are preserved. Unsupported stored identity or state fails
closed with `FindingContract`.

## Lifecycle follow-up

The separate `IndexDriftFindingLifecycleService` and
`PostgresIndexDriftFindingLifecycleStore` now provide authorized open-to-resolved and open-to-ignored
commands with explicit state preconditions and append-only actor/action/reason audit. See
`m6-drift-finding-lifecycle.md`.

Finding recording still does not accept lifecycle authority. A new detection may reopen a resolved
finding through the existing writer; ignored findings remain suppressed.

## Outputs and failures

The output is only:

- `Recorded(IndexDriftFindingWriteOutcome)`; or
- `NotRecorded(MaterializedChanged)`.

Errors are a closed enum: unsupported backend, invalid evidence, retryable storage, or permanent
stored finding contract failure. No payload, SQL, database cause, actor, reason, or repair evidence is
returned.

## Composition and transport boundary

`materialize_postgres_index_drift_confirmed_candidate_writer` validates PostgreSQL and returns the
internal writer. It executes no SQL during composition and does not insert the writer into
`ModuleRuntimeExtensions`.

Finding persistence and lifecycle remain unmounted from GraphQL, HTTP, CLI, MCP, native admin,
background workers, and schedulers.

## Deliberate limits

These slices do not add:

- automatic candidate-page iteration or confirmation-to-write orchestration;
- lifecycle public transport or audit inspection;
- retained PostgreSQL/concurrency execution evidence;
- targeted, shadow, full, or automatic repair.

## Next implementation step

Add one internal targeted repair boundary with authorized operator capability and admitted before and
after evidence. Keep automatic iteration and public transport separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_confirmed_candidate_writer -- --nocapture
cargo test -p rustok-index drift_finding_lifecycle -- --nocapture
node scripts/verify/verify-index-confirmed-candidate-persistence.mjs
node scripts/verify/verify-index-drift-finding-lifecycle.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, migrations, PostgreSQL scenarios, workflows, or CI were
executed by the implementation agent.
