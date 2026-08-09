# Durable artifact-data snapshot and guarded restore

- Date: 2026-07-22
- Amended: 2026-08-09
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
`ModuleControlPlane`. Snapshot and restore authority bind the canonical
`(scope_id, stable data_owner_id, namespace_instance_id,
namespace_revision, data_contract_digest)` identity. Module slug, package
version, publisher display name, and installation ID are descriptive metadata,
never attach or restore authority. The owner locks that exact active namespace
instance under its authorization/RLS domain and transactionally stages bounded
structured records, logical object metadata, materialized index projections,
and the bound index-contract digest. It then copies immutable source object
bytes to private snapshot-owned storage keys and verifies size and SHA-256
digest before publishing a canonical logical manifest digest. Physical storage
keys never participate in the manifest and never cross the artifact capability
boundary.

A snapshot moves only from `staging` to `ready`. Before each object copy the
owner persists a durable copy intent binding operation, source digest/key,
destination staging/final identity, and manifest entry. Publication is
create-if-absent and a separate transaction records the verified reference.
Creation is idempotent, retry resumes the exact incomplete copy, and a
reconciler may tombstone an orphan only after proving no manifest, live target,
operation, upload session, incident, audit, or legal hold references it. Object
GC takes the namespace lifecycle lock and retains a source storage key while a
staging snapshot references it.
Per-operation bounds are 1,000 structured records, 64 objects, 8,192 index rows,
and 256 MiB of object bytes.

Restore is separately authorized and idempotent. It accepts only a `ready`
snapshot for the same stable data owner and exact compatible data-contract
identity, re-hashes the manifest and every private object, and never overwrites
an active non-empty target. Before purge, restore may target a proven empty
namespace instance at the expected revision. After purge, restore must create a
new opaque isolated namespace instance under the same stable data owner,
assemble and verify it while non-serving, then perform a separately authorized
active-reference CAS cutover. The old namespace tombstone is monotonic and is
never cleared or reused. One transaction publishes each target's logical
values, object metadata, index projections, index contract, namespace revision
CAS, audit operation, and outbox event; the later active-reference cutover is a
separate guarded step. The restore authorizer returns the exact target
`ArtifactDataQuota`; the owner revalidates that immutable quota and checks
structured count/bytes plus object count/bytes against the canonical manifest
before any restored namespace becomes active. Crash/retry before or after
cutover cannot expose two active instances or attach data by slug.

The existing `(tenant, module_slug, data_contract_revision, policy_revision)`
scope and active-empty-only restore are implementation gaps. The release-safety
cutover replaces their identity, snapshot rows, manifests, authorizers,
retention roots, restore paths, fixtures, and callers atomically; no slug-based
fallback or dual attach path remains.

## Consequences

- Bounded export remains an operator data-transfer feature and is not a backup
  protocol.
- A failed object copy leaves resumable staging state rather than a partially
  restorable snapshot.
- A pre-purge recovery point remains usable after purge only by restoring into
  a new namespace instance under the same stable data owner; it never resurrects
  the purged physical identity.
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
