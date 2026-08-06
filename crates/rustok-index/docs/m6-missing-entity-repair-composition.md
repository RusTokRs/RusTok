# M6 concrete missing-entity drift repair

Status: `source_complete_recovery_policy_pending`.

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
- one `PostgresIndexDriftMissingEntityRepairOwner`;
- the existing durable `PostgresIndexDriftRepairStore` behind a missing-entity-only gate;
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

Before and after evidence use the same bounded algorithm:

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

### Repairable

Before evidence is `Repairable` only when:

- owner absence version equals the committed finding target absence version;
- the exact Index row is live at the committed indexed source version;
- the absence version is strictly greater than the indexed version.

The strict inequality preserves the existing monotonic mutation contract. Equal-version repair does
not receive a special bypass and is classified `Changed`.

### Converged

After evidence is `Converged` only when:

- owner absence remains at the exact committed absence version;
- the exact Index row is a tombstone at that same version.

A physically missing row, changed version, live row, owner upsert, or changed absence proof is
classified `Changed` rather than being treated as successful repair.

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

## Failure mapping

Source and absence provider failures retain only retryable/permanent classification. PostgreSQL read
failures and transient mutation conflicts are retryable. Invalid source identity, malformed stored
versions, delivery conflicts, schema validation errors, and unsupported backends fail permanently
with bounded machine codes.

A retryable failure leaves the generic durable reservation in `prepared` state for exact command
retry.

## Deliberate limits

This slice does not add:

- orphan-link repair;
- a public or allow-all authorizer;
- GraphQL, HTTP, CLI, MCP, native-admin, or module-runtime composition;
- scheduler, worker, candidate-page loop, or automatic repair;
- automatic lifecycle transition after convergence;
- prepared-command lease, expiry, abandonment, takeover, or operator recovery;
- retained PostgreSQL, source-owner, crash-window, workflow, or CI evidence.

## Next implementation step

Add a fail-closed recovery policy for durable `prepared` repair commands. The policy must distinguish
an actively owned attempt from an abandoned attempt, preserve command payload identity, require an
authorized operator decision, and never silently replay or discard an ambiguous owner side effect.
Keep orphan-link repair and public transport separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_missing_entity_repair -- --nocapture
node scripts/verify/verify-index-missing-entity-repair-composition.mjs
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were run by
the implementation agent.
