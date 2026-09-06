# M6 concrete missing-entity drift repair

Status: `source_complete_recovery_aware_owner_execution_pending`.

## Purpose

This slice composes the first concrete owner path behind `IndexDriftRepairService`.
It supports exactly one confirmed finding kind:

- `index.confirmed_missing_entity`.

The path removes one stale live Index entity after the authoritative source proves the exact entity
is absent at a strictly newer source version. It does not support orphan-link repair, scan findings,
accept mutation JSON, mount a transport, or resolve the finding lifecycle row.

## Internal composition

`materialize_postgres_index_drift_missing_entity_repair_service` requires explicit:

- `Arc<dyn IndexDriftRepairAuthorizer>`;
- PostgreSQL connection;
- `Arc<SchemaRegistry>`;
- frozen `SharedIndexSourceRegistry`;
- frozen `SharedIndexSourceAbsenceRegistry`.

It composes:

- `PostgresIndexDriftMissingEntityEvidenceReader`;
- one `PostgresIndexDriftMissingEntityRepairOwner` behind
  `RecoveryAwareIndexDriftRepairOwner`;
- the durable `PostgresIndexDriftRepairStore` behind
  `RecoveryAwareIndexDriftRepairStore` and a missing-entity-only gate;
- the generic `IndexDriftRepairService`.

The helper is exported at crate level but is not inserted into `ModuleRuntimeExtensions`.
No default authorizer is provided.

## Unsupported target gate

The concrete store wrapper rejects every target except `MissingEntity` before delegating to the
generic reservation store. An orphan-link command therefore cannot create a durable `prepared`
reservation through this composition.

The generic target commitment check still runs for admitted missing-entity commands and verifies the
exact finding key, check name, scope, expected digest, actual digest, and details marker.

## Evidence capture

Before and after evidence use the same bounded read sequence:

1. targeted-load the exact owner key through `SharedIndexSourceRegistry`;
2. when ordinary load is empty, require an exact retained watermark from
   `SharedIndexSourceAbsenceRegistry`;
3. read only `source_version` and `is_deleted` from the exact `index_entities` identity;
4. repeat the owner read;
5. reject an owner change as retryable rather than admitting mixed evidence;
6. derive a domain-separated SHA-256 digest from typed identity and observed state.

No source payload, field map, link graph, schema fingerprint, finding details, SQL cause, or owner
failure code crosses the evidence boundary.

A source `Delete` mutation is itself retained absence evidence. An empty ordinary load is never
absence proof without the admitted absence registry.

### Before phase

Before evidence is `Repairable` when either:

- owner absence equals the committed target absence version, the exact Index row is live at the
  committed indexed version, and the absence version is strictly newer; or
- owner absence equals the committed target absence version and the exact Index row is already the
  tombstone at that version.

The second case is the crash-retry path. A process may fail after the idempotent delete commits but
before after-evidence or repair receipt persistence. On exact retry, the tombstone remains
`Repairable` during the before phase so the same command UUID reaches the mutation inbox duplicate
path. This does not itself claim convergence.

The strict version inequality for a live row preserves the existing monotonic mutation contract.
Equal-version repair does not receive a special bypass and is classified `Changed`.

### After phase

After evidence is `Converged` only when:

- owner absence remains at the exact committed absence version;
- the exact Index row is a tombstone at that same version.

A physically missing row, changed version, live row, owner upsert, or changed absence proof is
classified `Changed` rather than being treated as successful repair.

The same tombstone is therefore phase-sensitive: `Repairable` before an idempotent retry and
`Converged` only after the owner call. This preserves the generic service order and prevents a retry
from being terminally misclassified as `NotRepaired(before_not_repairable)`.

## Idempotent mutation owner

The owner constructs exactly one typed mutation:

`IndexMutation::Delete { event_id: command_id, key, source_version: absence_source_version }`

It submits the mutation through:

- `MutationDelivery::from_event("index_drift_repair_missing_entity", mutation)`;
- `PostgresMutationStore::apply(schema_registry, delivery)`.

The durable repair command UUID is therefore also the mutation event and inbox delivery identity.
A retry after mutation commit but before repair receipt persistence resolves through the existing
inbox duplicate contract rather than issuing an unrelated mutation.

The owner returns a bounded SHA-256 receipt digest. The digest binds command UUID, finding UUID,
entity identity, committed versions, and the typed mutation result (`Applied`, `Duplicate`, or
`StaleIgnored`). It exposes no mutation payload or database cause.

`StaleIgnored` is not trusted as convergence by itself. The generic service still requires admitted
after evidence to be `Converged` before recording `Repaired`.

## Recovery-aware admission

Every newly reserved command receives an immutable revision-0 `active` recovery decision. A legacy
or crash-stranded `prepared` command without a decision fails with
`index_drift_repair_recovery_required` until an independently authorized recovery `Resume` is
recorded. `paused` and `abandoned` commands fail closed.

`RecoveryAwareIndexDriftRepairOwner` holds the exact tenant/command PostgreSQL advisory fence while
checking payload identity and latest recovery state and while delegating the mutation owner. An
operator pause or abandon therefore cannot enter concurrently before the side effect; it either wins
before owner admission or waits until the already admitted owner call returns.

Completion is gated twice: by `RecoveryAwareIndexDriftRepairStore` and by the database trigger added
in migration `m20260806_000008_add_index_finding_repair_recovery`. A pause or abandon that wins after
the owner call but before receipt persistence prevents `prepared -> completed`. The implementation
does not infer whether the side effect happened; authorized resume preserves the same mutation UUID
and re-enters the inbox idempotency path.

The complete recovery contract is documented in `m6-prepared-repair-recovery.md`.

## Failure mapping

Source and absence provider failures retain only retryable/permanent classification. PostgreSQL read
failures and transient mutation conflicts are retryable. Invalid source identity, malformed stored
versions, delivery conflicts, schema validation errors, unsupported backends, and non-active
recovery state fail permanently with bounded machine codes.

A retryable failure leaves the generic durable reservation in `prepared` state. Exact retry proceeds
only while the latest recovery state is `active`.

## Deliberate limits

This slice does not add:

- orphan-link repair;
- a public or allow-all authorizer;
- GraphQL, HTTP, CLI, MCP, native-admin, or module-runtime composition;
- scheduler, worker, candidate-page loop, or automatic repair;
- automatic lifecycle transition after convergence;
- time-derived lease expiry or automatic ownership inference;
- cancellation after an owner call has acquired the recovery fence;
- retained PostgreSQL, source-owner, crash-window, workflow, or CI evidence.

## Next implementation step

Compose one concrete orphan-link evidence reader and idempotent mutation owner behind the same
recovery-aware boundary. Preserve exact source link, ordinal, target identity, target absence proof,
and durable repair command UUID.

Keep public transport and automatic finding iteration separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_missing_entity_repair -- --nocapture
node scripts/verify/verify-index-missing-entity-repair-composition.mjs
node scripts/verify/verify-index-prepared-repair-recovery.mjs
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite scenarios,
workflows, or CI were executed by the implementation agent.
