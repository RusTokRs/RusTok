# Static module lifecycle revision aggregate

- Date: 2026-08-20
- Status: Accepted

## Context

`tenant_modules` is an explicit tenant-override projection, not a lifecycle
aggregate. An absent row means that a module inherits its distribution default,
so a row revision cannot provide a concurrency precondition for that state.
The lifecycle journal provides recovery and idempotency for hook transitions,
but it neither serializes settings writes with toggles nor makes a stale
revision fail before a second pre-hook can run.

Static platform-native modules require one owner-authoritative concurrency
boundary for enablement intent, normalized settings, post-hook retry, and
compensation. Artifact installation lifecycle remains a separate aggregate
because its identity, admission, and retention contracts are different.

## Decision

For SQLite, the tenant owner publishes the read-only `tenant_identity_keys`
database projection containing only the stored tenant primary key. The static
module owner may use that projection in the same statement as its own rows to
detect ambiguous UUID representations and orphaned state; it must not query the
tenant owner's private tables. The projection preserves the SQLite storage
class and does not authorize or resolve a tenant. Missing projection or
ambiguous identity must fail closed. This narrow persistence boundary is needed
because independently resolved reads cannot establish a single SQLite snapshot.
It does not settle the representation used by lifecycle writes or migrate old
rows; those require a separate complete cutover.

The SQLite lifecycle journal additionally enforces logical uniqueness of a
tenant and idempotency UUID across native BLOB and canonical text encodings using an
owner migration. The migration fails when existing identities collide; it
does not choose a receipt or rewrite history. This index fences older writers
after migration, but does not itself authorize a tenant, repair malformed data,
or settle the UUID representation of the other lifecycle tables.

`rustok-modules` owns a durable `module_static_tenant_lifecycle` aggregate for
each `(tenant_id, module_slug)`. It stores a monotonic revision independently
of `tenant_modules`, so the inherited/default state has revision `0` before a
first durable aggregate row is created.

Every static lifecycle write supplies an authenticated tenant and actor UUID,
a non-nil idempotency UUID, and the reviewed expected revision. The owner
admits an exact idempotent replay before evaluating the revision. A fresh
command atomically claims the aggregate only when the expected revision
matches and no other operation is active. The claim remains held while a
pre-hook or post-hook runs; a different command fails closed instead of
dispatching a concurrent hook. The operation that changes explicit intent or
settings advances the aggregate revision and clears the claim in the same
transaction as its tenant-state write and policy-revision transition.

Post-hook retry acquires and releases the same aggregate claim without changing
the revision. Compensation is a new inverse lifecycle transition and advances
the revision on its own successful state commit. A process-loss recovery must
resume with the same idempotency identity; the owner never silently abandons an
unknown active hook and starts another command for that aggregate.

The owner exposes the aggregate revision in static module read snapshots. The
canonical GraphQL and native/admin contracts echo that value in every mutation
and return the resulting revision. Settings use the shared owner-operation
receipt ledger for exact replay; hook transitions retain `module_operations`
for their lifecycle recovery evidence.

For static settings, replay identity may depend on the schema that normalized
the original request. The settings architecture ADR requires immutable schema
evidence and defines explicit reconciliation for historical receipts whose
identity cannot be proven after that evidence is lost. Such a case must not be
treated as permission to execute the command again or delete its receipt.

## Consequences

- `tenant_modules` remains only the explicit enablement/settings projection;
  inherited state does not require a synthetic override row.
- Revision conflicts and active-operation conflicts are observable,
  non-retryable control-plane outcomes that require a refreshed snapshot.
- Static toggles, settings, retries, and compensations cannot bypass the same
  owner aggregate through a server or admin-local write path.
- The durable aggregate is not shared with dynamic artifact tenant lifecycle;
  the two owners expose separate snapshots and revisions.

## Related documents

- [Module control-plane consolidation plan](../docs/modules/module-control-plane-consolidation-plan.md)
- [Lifecycle hook phases and retry contract](./2026-05-22-module-lifecycle-hook-phases-and-retry-contract.md)
- [Shared owner-operation receipt ledger](./2026-08-03-owner-operation-receipts.md)
