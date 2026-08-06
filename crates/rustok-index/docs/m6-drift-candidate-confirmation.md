# M6 bounded drift candidate confirmation

Status: `source_complete_persistence_complete_lifecycle_complete_repair_pending`.

## Purpose

`IndexDriftCandidateConfirmer` converts one database-neutral stale-entity or orphan-link candidate
into a bounded typed confirmation outcome. It does not scan a page, loop over candidates, write a
finding, mutate Index storage, start a task, or repair data.

A candidate is only discovery evidence. Confirmation requires the materialized identity to remain
unchanged and the authoritative owner state to provide exact positive-version evidence.

## Dependencies

The confirmer uses only:

- the frozen `SharedIndexSourceRegistry` for exact one-key targeted loads;
- the optional frozen `SharedIndexSourceAbsenceRegistry` for explicit retained absence watermarks;
- one `IndexDriftCandidateMaterializedObserver` for narrow identity/version/link observations.

The production observer is `PostgresIndexDriftCandidateMaterializedObserver`. It reads no payload,
field map, schema fingerprint, graph aggregate, finding row, or owner-private record.

`materialize_postgres_index_drift_candidate_confirmer` constructs the internal boundary from frozen
registries and a PostgreSQL connection. It performs no SQL during composition and does not insert the
confirmer into runtime extensions.

## Materialized bracketing

`confirm_candidate` observes the exact candidate before any owner call. A changed or missing
materialized identity returns `NotCandidate(MaterializedChanged)`.

Only a provisionally confirmed candidate is observed again after owner evidence. If the exact
materialized entity/version/link/target-absence shape changed, the result is downgraded to
`NotCandidate(MaterializedChanged)`.

The PostgreSQL observer performs one exact query per observation:

- stale entity: exact tenant/schema/entity/locale, live row, and identical indexed source version;
- orphan link: exact live source entity/version, link name, ordinal, complete typed target identity,
  and target row still absent or represented by a positive-version tombstone.

## Stale entity confirmation

For `IndexDriftStaleEntityCandidate`, the confirmer:

1. verifies the materialized live row/version;
2. targeted-loads the exact source key;
3. treats an authoritative upsert as `NotCandidate(SourcePresent)`;
4. accepts an authoritative delete or admitted absence watermark only with a positive version;
5. requires the absence version to be at least the indexed source version;
6. repeats the same authoritative read and requires the same absence version;
7. repeats the materialized observation;
8. returns `Confirmed(MissingEntity)` with only the key, indexed version, and absence version.

An empty ordinary targeted load is never interpreted as absence. Missing provider capability,
missing watermark, malformed owner state, or scope drift fails closed.

## Orphan-link confirmation

For `IndexDriftOrphanLinkCandidate`, the confirmer:

1. verifies the exact materialized source entity/version/link/ordinal/target and target absence;
2. targeted-loads the exact source key;
3. requires an authoritative upsert with the same source version and link target at the same ordinal;
4. targeted-loads the exact target key under the source tenant;
5. treats an authoritative target upsert as `NotCandidate(TargetPresent)`;
6. accepts only an authoritative target delete or admitted target absence watermark;
7. repeats source-link and target-absence evidence;
8. repeats the materialized observation;
9. returns `Confirmed(OrphanLink)` with only typed identity and target absence version.

A deleted or absent authoritative source entity is not an orphan-link finding candidate. A changed
source version or link becomes a typed `NotCandidate` outcome.

## Outcomes and failures

Confirmed outcomes expose only typed identity and positive source-version evidence. `NotCandidate`
reasons are a closed enum. Dependency failures expose only retryable/permanent classification and a
bounded lowercase machine code; source failure codes, provider names, SQL, causes, payloads, fields,
and secret values are not propagated.

## Downstream boundaries

`PostgresIndexDriftConfirmedCandidateWriter` revalidates exact materialized state and records the
finding in one serializable transaction. The persistence contract is documented in
`m6-confirmed-candidate-finding-persistence.md`.

`IndexDriftFindingLifecycleService` and its PostgreSQL store now add authorization-gated
open-to-resolved/open-to-ignored commands with idempotent actor/action/reason audit. The lifecycle
contract is documented in `m6-drift-finding-lifecycle.md`.

Neither persistence nor lifecycle turns confirmation evidence into repair authority.

## Deliberate limits

The confirmation, persistence, and lifecycle slices do not add:

- page iteration, cursor persistence, background execution, scheduling, or restart state;
- GraphQL, HTTP, CLI, MCP, native-admin, or public continuation transport;
- targeted, shadow, full, or automatic repair;
- retained PostgreSQL, owner-source, migration, concurrency, workflow, or CI evidence.

## Next implementation step

Add one internal targeted repair boundary with authorized operator capability, finding-specific owner
selection, and admitted before/after evidence. Keep public transport and automatic repair out of that
slice.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_candidate_confirmation -- --nocapture
cargo test -p rustok-index drift_confirmed_candidate_writer -- --nocapture
cargo test -p rustok-index drift_finding_lifecycle -- --nocapture
node scripts/verify/verify-index-drift-candidate-confirmation.mjs
node scripts/verify/verify-index-confirmed-candidate-persistence.mjs
node scripts/verify/verify-index-drift-finding-lifecycle.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, verifiers, formatting, Cargo checks, migrations, PostgreSQL scenarios, workflows, or CI were
executed by the implementation agent.
