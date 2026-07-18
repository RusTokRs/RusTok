# Artifact declarative DDL boundary

- Date: 2026-07-18
- Status: Accepted

## Context

Untrusted marketplace artifacts may store bounded structured values through the
owner-owned data broker. Allowing an artifact descriptor to declare SQL or DDL
would cross that boundary: it could affect schema isolation, lock availability,
tenant rollout, backup, rollback, and data retention. The current descriptor
and runtime must not imply that such declarations are supported.

## Decision

V1 forbids declarative DDL, arbitrary SQL, native migrations, and physical
storage paths in artifact descriptors. Artifact migrations remain limited to
the admitted bounded `data_upgrade` binding and brokered structured values.
Static-promoted modules continue to use reviewed owner-provided
`MigrationSource` migrations in a distribution build.

Any future declarative DDL feature requires a new accepted ADR and a separate
implementation plan that proves all of the following before admission is
enabled:

- an allow-listed operation vocabulary and schema-per-tenant/module isolation;
- bounded lock acquisition, timeout, contention, and cancellation behavior;
- durable rollout/checkpoint/recovery state with no transaction held across
  sandbox work;
- reversible, compensating, and prohibited rollback policy plus backup/restore
  evidence;
- no cross-module references, unrestricted indexes, triggers, functions, or
  host-managed table access;
- tenant-by-tenant rollout controls, retention/legal-hold integration, audit,
  and authorization; and
- threat-model fixtures covering privilege escalation, lock exhaustion,
  rollback failure, and cross-tenant access.

## Consequences

`rustok-modules` continues to reject unknown persistence metadata and exposes
no DDL capability to artifact sandboxes. A host cannot add a descriptor-level
escape hatch or translate a marketplace payload into a direct migration. The
absence of declarative DDL is an intentional platform contract, not a missing
adapter.
