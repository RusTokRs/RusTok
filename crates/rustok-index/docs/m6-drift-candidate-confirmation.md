# M6 bounded drift candidate confirmation

Status: `source_complete_persistence_complete_lifecycle_complete_targeted_repair_boundary_complete`.

## Purpose

`IndexDriftCandidateConfirmer` converts one database-neutral stale-entity or orphan-link candidate
into a bounded typed confirmation outcome. It does not scan a page, loop over candidates, write a
finding, mutate Index storage, start a task, or repair data.

A candidate is only discovery evidence. Confirmation requires the materialized identity to remain
unchanged and the authoritative owner state to provide exact positive-version evidence.

## Dependencies and materialized bracketing

The confirmer uses only:

- the frozen `SharedIndexSourceRegistry` for exact one-key targeted loads;
- the optional frozen `SharedIndexSourceAbsenceRegistry` for explicit retained absence watermarks;
- one `IndexDriftCandidateMaterializedObserver` for narrow identity/version/link observations.

The production observer is `PostgresIndexDriftCandidateMaterializedObserver`. It reads no payload,
field map, schema fingerprint, graph aggregate, finding row, or owner-private record.

`confirm_candidate` observes the exact candidate before any owner call and again after provisional
confirmation. Changed materialized identity becomes `NotCandidate(MaterializedChanged)`.

## Stale entity confirmation

For `IndexDriftStaleEntityCandidate`, the confirmer:

1. verifies the materialized live row/version;
2. targeted-loads the exact source key;
3. treats an authoritative upsert as `NotCandidate(SourcePresent)`;
4. accepts an authoritative delete or admitted absence watermark only with a positive version;
5. requires the absence version to be at least the indexed source version;
6. repeats the same authoritative read and requires the same absence version;
7. repeats the materialized observation;
8. returns `Confirmed(MissingEntity)` with only the key and version evidence.

An empty ordinary targeted load is never interpreted as absence.

## Orphan-link confirmation

For `IndexDriftOrphanLinkCandidate`, the confirmer:

1. verifies exact materialized source/link/ordinal/target identity and target absence;
2. requires an authoritative source upsert with the same source version and exact link target;
3. requires authoritative target delete or admitted absence evidence;
4. repeats source-link, target-absence, and materialized observations;
5. returns `Confirmed(OrphanLink)` with only typed identity and positive version evidence.

A deleted/absent authoritative source is not an orphan-link candidate. Changed source/link/target
state becomes a typed `NotCandidate` outcome.

## Downstream boundaries

`PostgresIndexDriftConfirmedCandidateWriter` revalidates exact materialized state and records the
finding in one serializable transaction. See `m6-confirmed-candidate-finding-persistence.md`.

`IndexDriftFindingLifecycleService` provides authorization-gated resolve/ignore commands with
idempotent actor/action/reason audit. See `m6-drift-finding-lifecycle.md`.

`IndexDriftRepairService` now defines a separate authorization-gated targeted-repair orchestration.
Its PostgreSQL reservation store accepts a typed target only when it reproduces the exact confirmed
finding check identity, finding key, and expected/actual evidence commitments. See
`m6-targeted-drift-repair.md`.

Confirmation itself still grants no repair authority. Concrete repair evidence readers and mutation
owners remain separate composition work.

## Outcomes and failures

Confirmed outcomes expose only typed identity and positive source-version evidence. `NotCandidate`
reasons are closed. Dependency failures expose only retryable/permanent classification and a bounded
lowercase machine code; source failures, provider names, SQL, payloads, fields, and secrets are not
propagated.

## Deliberate limits

These slices do not add:

- page iteration, cursor persistence, background execution, scheduling, or restart state;
- GraphQL, HTTP, CLI, MCP, native-admin, or public continuation transport;
- a concrete targeted-repair evidence reader or owner;
- prepared-repair recovery policy;
- shadow, full, or automatic repair;
- retained PostgreSQL, owner-source, migration, concurrency, workflow, or CI evidence.

## Next implementation step

Compose one concrete bounded repair evidence reader and one concrete idempotent owner for the
smallest supported confirmed finding kind. Keep public transport and automatic iteration separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_candidate_confirmation -- --nocapture
cargo test -p rustok-index drift_confirmed_candidate_writer -- --nocapture
cargo test -p rustok-index drift_finding_lifecycle -- --nocapture
cargo test -p rustok-index drift_repair -- --nocapture
node scripts/verify/verify-index-drift-candidate-confirmation.mjs
node scripts/verify/verify-index-confirmed-candidate-persistence.mjs
node scripts/verify/verify-index-drift-finding-lifecycle.mjs
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite scenarios, workflows, or
CI were executed by the implementation agent.
