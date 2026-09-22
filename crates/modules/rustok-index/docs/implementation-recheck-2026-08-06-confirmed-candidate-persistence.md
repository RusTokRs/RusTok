# Confirmed candidate persistence recheck — 2026-08-06

Status: `source_reviewed_unvalidated`.

Audited baseline: `main@b18558e3f8443135a403780d1ef75bc4d14dba6d`.

## Reviewed boundary

The review covered:

- `PostgresIndexDriftConfirmedCandidateWriter`;
- its PostgreSQL exports and root crate exports;
- the established finding-key, finding-scope, details, and lifecycle-state storage contract;
- candidate, reader, confirmation, and persistence documentation;
- aggregate and slice-specific static guards.

The reviewed slice accepts exactly one `IndexDriftConfirmedCandidate`. It adds no candidate-page
iteration, source call, public transport, runtime-extension insertion, scheduler, lifecycle command,
or repair authority.

## Evidence conclusion

Missing-entity finding identity is the exact entity scope plus the fixed
`index.confirmed_missing_entity` check name.

Orphan-link finding identity uses the exact source entity scope plus a bounded SHA-256 check suffix
that binds link name, ordinal, complete typed target identity, optional target locale, and target
absence source version.

Expected and actual SHA-256 evidence use separate domains and state tags. All components are
length-prefixed and derived only from typed confirmation fields. The adapter cannot accept raw JSON,
caller-selected digests, check names, finding IDs, timestamps, or stored lifecycle state.

## Write-time revalidation conclusion

Every record attempt uses one PostgreSQL `SERIALIZABLE READ WRITE` transaction.

Before acquiring the finding-key lock or changing a finding row, the adapter rechecks:

- stale entity: exact tenant/schema/entity/locale, live state, and indexed source version;
- orphan source: exact tenant/schema/entity/locale, live state, and indexed source version;
- orphan link: exact source version, link name, ordinal, and typed target;
- orphan target: absent row or positive-version tombstone.

Existing rows are locked with `FOR SHARE`. An absent target remains a serializable predicate read.
A changed candidate returns `NotRecorded(MaterializedChanged)` and rolls back without touching the
finding table. Query, serialization, or commit conflicts map to retryable `Storage` without exposing
database causes.

## Finding storage conclusion

The adapter retains the established Index finding contract:

- the same deterministic tenant/check/scope finding key;
- the same `index-drift-finding` advisory-lock namespace;
- the same fixed `index_drift_digest_finding_v1` details marker;
- `Created` for a new finding;
- `Refreshed` for an open finding;
- `Reopened` for a resolved finding;
- `Suppressed` while an ignored finding remains ignored.

Finding identity and first-detected time are preserved. Stored check/scope drift or unsupported state
fails closed as `FindingContract`.

The transactional state-machine logic is a narrow Index-owned extension of the existing writer
contract because the existing writer does not expose its transaction-internal method across sibling
modules. Static guards bind the shared advisory namespace, details marker, and four state outcomes.
No second table or independent lifecycle model was introduced.

## Concurrency conclusion

Owner-source confirmation and finding persistence do not share a distributed transaction. The
confirmation boundary double-reads owner evidence and brackets it with materialized observations;
the persistence boundary then serializably revalidates materialized Index state immediately before
the write.

This prevents persistence after known materialized drift but does not claim retained atomic evidence
across PostgreSQL and every owner source. A later owner change can make an open finding stale; bounded
reconciliation and future lifecycle/repair policy must handle that explicitly.

## Open boundaries

Still open:

- fail-closed resolve and ignore commands with actor/reason audit;
- internal orchestration from candidate pages through confirmation and persistence;
- authorization and any operator transport;
- targeted repair and before/after admitted evidence;
- retained PostgreSQL, serialization-conflict, restart, workflow, and CI evidence.

## Validation disclosure

No tests, JavaScript verifiers, formatting, Cargo commands, PostgreSQL scenarios, workflows, or CI
were run. Compilation and runtime behavior are not claimed.
