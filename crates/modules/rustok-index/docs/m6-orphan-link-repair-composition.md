# M6 concrete orphan-link drift repair

Status: `source_complete_owner_execution_pending`.

## Purpose

This slice composes one bounded repair path for exact confirmed orphan-link findings:

- `index.confirmed_orphan_link.<sha256>`.

It removes one exact materialized `index_links` row while preserving the source entity row, source
version, payload, all other links, finding history, and recovery history. It does not scan findings,
accept arbitrary SQL or JSON, mount a transport, or resolve the finding lifecycle row.

## Internal composition

`materialize_postgres_index_drift_orphan_link_repair_service` requires an explicit repair authorizer,
a PostgreSQL connection, frozen source and absence registries, and an owner-supplied
`PostgresIndexOrphanLinkAuthorityFence`. No destructive owner can be constructed without this
capability. It composes:

- `PostgresIndexDriftOrphanLinkEvidenceReader`;
- `PostgresIndexDriftOrphanLinkRepairOwner`;
- `PostgresIndexOrphanLinkMutationStore` as the typed persistence owner;
- the existing recovery-aware owner and repair-store fences;
- an orphan-link-only reservation gate;
- the generic `IndexDriftRepairService`.

The helper is exported from the crate but is not registered in `ModuleRuntimeExtensions`.

## Exact target commitment

The generic repair store first re-derives the persisted orphan check name, identity suffix, finding
key, expected digest, actual digest, entity scope, and details contract. The concrete path then
requires the same typed source key, indexed source version, link name, ordinal, linked target, and
target absence version at every evidence and owner boundary.

A changed source version, link name, ordinal, target schema, target entity, target locale, target
absence version, or finding identity fails closed.

## Evidence capture

For fresh repair and for post-commit convergence, evidence uses the same bounded sequence:

1. load the exact source entity from its authoritative source;
2. load the exact linked target or retained target-absence watermark;
3. read one repeatable-read materialized snapshot containing the source entity, the exact link
   ordinal, and the command-bound inbox delivery;
4. repeat both authoritative reads;
5. reject moving authority as retryable rather than admitting mixed evidence.

The source authority must remain present at the exact indexed version and must still contain the
exact link at the exact ordinal. The linked target must remain absent at the exact committed absence
version. The materialized source entity must remain live at the exact indexed version.

### Before phase

Fresh before evidence is `Repairable` only when the exact materialized link exists, current
authority matches, and no command-bound delivery exists.

An exact `applied` delivery can instead supply before evidence for read-only crash recovery.
Link removal and inbox completion share one transaction, so that delivery proves this command
removed the edge. It authorizes recovery of the receipt, not a second deletion. If the witness
vanishes, recovery fails closed. A physically absent link without that proof is never admitted
as repair progress.

### After phase

After evidence is `Converged` only when:

- authoritative source/link identity remains unchanged;
- target absence remains exact;
- the source entity remains live at the exact indexed version;
- the exact materialized link is absent;
- the exact command-bound inbox delivery is `applied` with the expected payload digest.

Changed source or target authority, a restored edge, or an absent inbox delivery does not prove
convergence. If the exact original delivery is known to be applied but convergence changed, retain
the owner's receipt in `NotRepaired(after_not_converged)` without repeating the deletion.

## Typed link-removal owner

The repair owner does not issue SQL. It constructs one typed command-bound removal and delegates to
`PostgresIndexOrphanLinkMutationStore`.

The mutation digest binds:

- repair command UUID;
- source entity identity and exact source version;
- link name and ordinal;
- complete linked target identity;
- committed target absence version.

The store uses the existing `index_inbox` identity `(tenant_id, source_name, delivery_id)` with:

- source name `index_drift_repair_orphan_link`;
- delivery ID equal to the durable repair command UUID;
- the existing `delete` storage category;
- the domain-separated typed removal digest as `payload_hash`.

The `delete` category is only the existing inbox storage family. The source name and payload digest
bind the operation to exact link removal and prevent confusion with entity tombstones.

Within one read-committed transaction the store:

1. requires the owner fence to revalidate and hold the exact source revision/edge and target
   absence until this transaction ends; owners must coordinate writes or prove immutability;
2. reserves or replays the exact inbox delivery;
3. acquires canonical Index entity locks for both keys in a stable order, including missing targets;
4. verifies the exact live source, absence of a newer target, and unchanged target/ordinal;
5. deletes exactly that row without rewriting the source entity or other links;
6. marks the inbox delivery applied.

Double-reading authority is diagnostic, not a write fence. Independently mutable owners without
a transaction-safe fence are not admitted. A later target revival must advance affected source
revisions to republish those edges, or the owner must guarantee permanent absence.
A duplicate command returns the existing applied delivery. Payload reuse under the same delivery ID,
pending/rejected delivery state, source movement, or link substitution fails closed.

Ordinals above the removed row are intentionally not rewritten. Their stable stored identities and
relative query order remain intact, while the removed target disappears. A later authoritative
higher-version upsert remains the only operation allowed to replace the full link set.

## Recovery and crash boundary

`RecoveryAwareIndexDriftRepairOwner` holds the exact repair-command advisory fence across the owner
call. `RecoveryAwareIndexDriftRepairStore` and the database completion trigger require the latest
recovery decision to remain `active` before a receipt can be committed.

Pause or abandon therefore wins before owner admission or waits for the already admitted owner call.
If it wins after the link transaction but before repair completion, completion fails closed. An
authorized resume reuses the original command UUID and reaches the exact inbox duplicate path.

## Deliberate limits

This slice does not add public GraphQL/HTTP/CLI/MCP/native-admin transport, an allow-all authorizer,
automatic finding iteration, a scheduler, a worker, time-derived leases, lifecycle auto-resolution,
full-link-set rewriting, or retained PostgreSQL/concurrency evidence.

## Next implementation step

Retain migration, crash-window, inbox-idempotency, recovery-race, and PostgreSQL concurrency evidence
for both concrete repair owners before adding public authorization transport or automatic iteration.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_repair -- --nocapture
cargo test -p rustok-index drift_missing_entity_repair -- --nocapture
cargo test -p rustok-index drift_orphan_link_repair -- --nocapture
node scripts/verify/verify-index-orphan-link-repair-composition.mjs
node scripts/verify/verify-index-prepared-repair-recovery.mjs
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, PostgreSQL scenarios, workflows, or CI were run by
the implementation agent.
