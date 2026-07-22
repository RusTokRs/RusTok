# Durable artifact-data snapshot and guarded restore

- Date: 2026-07-22
- Status: Accepted

## Context

The artifact data owner already provides bounded structured-value export pages,
private object storage, namespace revision locking, purge tombstones, and
retention-aware object GC. Export pages cannot form a transactionally consistent
backup across pages, and object metadata alone cannot recover private object
bytes. Treating export as backup would also expose lifecycle races and leave no
durable manifest or restore audit identity.

## Decision

Artifact-data backup is a distinct owner-only snapshot operation composed by
`ModuleControlPlane`. The owner locks one exact active tenant/module/data-contract
namespace revision under tenant RLS and transactionally stages bounded structured
records, logical object metadata, materialized index projections, and the bound
index-contract digest. It then copies immutable source object bytes to private
snapshot-owned storage keys and verifies size and SHA-256 digest before publishing
a canonical logical manifest digest. Physical storage keys never participate in
the manifest and never cross the artifact capability boundary.

A snapshot moves only from `staging` to `ready`. Creation is idempotent and a
retry resumes incomplete object copies. Object GC takes the namespace lifecycle
lock and retains a source storage key while a staging snapshot references it.
Per-operation bounds are 1,000 structured records, 64 objects, 8,192 index rows,
and 256 MiB of object bytes.

Restore is separately authorized and idempotent. It accepts only a `ready`
snapshot for the same tenant/module/data-contract identity, re-hashes the
manifest and every private object, and requires an empty active target at the
expected namespace revision. One transaction restores logical values, object
metadata, index projections, the index contract, namespace revision CAS, audit
operation, and outbox event. Restore never clears a purge tombstone and never
replaces live data.

## Consequences

- Bounded export remains an operator data-transfer feature and is not a backup
  protocol.
- A failed object copy leaves resumable staging state rather than a partially
  restorable snapshot.
- Snapshot retention time and legal-hold state have an independent optimistic
  revision. Authorized idempotent commands may extend the deadline and apply or
  release legal hold, but never shorten retention.
- Collection requires separate host authorization, expiry, no legal hold, and an explicit durable policy
  snapshot with neither audit nor rollback hold. The owner persists an
  immutable `collecting` decision before deleting bytes, resumes it after
  interruption, and preserves audit rows after manifest deletion. Missing
  policy fails closed.
- Full control-plane database disaster recovery, CAS reconstruction, and outbox
  replay drills remain the separate Phase 11 platform recovery scope.
