# Durable RBAC static role grant revision

- Date: 2026-11-27
- Decision status: Accepted
- Implementation status: In progress
- Owners: RBAC (static grants), Auth (invite binding)
- Extends: [Membership revision](2026-11-27-rbac-membership-revision.md)
- Supersedes: None
- Superseded by: None

## Decision

A nonnegative signed 64-bit epoch on roles advances in the same transaction
as each actual role_permissions insert/delete/identity-changing update,
including cascade deletion. Role slug, tenant and system flag changes
advance it; presentation-only writes do not. Overflow and decrease fail
closed. Permission identity (id, tenant, resource, action) is immutable:
create a new permission and replace links instead of changing its meaning
in place. PostgreSQL forbids truncating role_permissions. Trusted maintenance
must not disable triggers or use SQLite REPLACE on parent roles/permissions
or conflict-update parent identities. Migration 000013 enforces permanent
user, role and permission UUID non-reuse in ordinary DML, including same-ID
SQLite replacement. Existing owner writers retain explicit insert-do-nothing
and delete/update. The append-only witness records only kind and UUID,
not live authority or a global generation, and cannot reconstruct identities
deleted before cutover. Natural-key replacement with a different UUID remains
outside this specific identity guarantee.

Auth locks the issuer user first, then all issuer membership role rows
and the exact destination role ordered by ID, before reading grants.
The ledger stores the canonical destination ID and sorted role ID/epoch
pairs, not a global cache generation. No missing-target fallback at
acceptance. A missing built-in role is materialized at issuance using the
RBAC bootstrap owner; a concurrent creator's grants are not reconciled by
the loser. Failure rolls back that initialization too. A missing custom
role is rejected. A replacement role ID or any bound grant change denies
the old invite even if current grants again match. Existing target
roles and issuer roles are held until commit. Migration 000006 adds a
PostgreSQL BEFORE ROW guard for role_permissions: lock old/new role IDs
ordered by UUID FOR NO KEY UPDATE NOWAIT, then referenced permissions for
insert/update FOR SHARE NOWAIT. Tenant validation follows the guard;
the AFTER revision increment cannot wait for a new surviving role parent.
DELETE locks only the revision parent, allowing a same-transaction role
cascade. Active parent RLS fails closed. Late contention raises 55P03 and
requires a whole-transaction retry. The migration also protects artifact
grant/receipt INSERT/UPDATE FK parents in actor, role, exact definition
order using KEY SHARE NOWAIT. It does not change receipt semantics, cascade
policy, or revision values. The normal host path already owns these parents.
No complete multi-row or parent-cascade lock-order guarantee is claimed;
SQLite keeps its single-writer contract. PostgreSQL contention regressions
are authored but not executed.

This covers static role grants, not artifact-defined permissions (separate
authorization plane) or module-policy revisions. No wire change. Database
rollback, disabled triggers and trusted administrator bypasses are not
protected. Legacy unbound pending invites are revoked at cutover; accepted
receipts and prior revocations remain. Drain old binaries before migrations;
no live schema downgrade. Source regressions are authored, compilation and
PostgreSQL concurrency/upgrade/rollback tests remain unrun.

## RBAC control-plane credential transaction extension (2026-11-27)

The GraphQL role-definition owner invokes a mandatory host mutation authority
port before role writes. The host reuses Auth's direct-session fence and
locks tenant FK protection, actor, session, static composition/lifecycle
policy, and the union of actor memberships and exact target role ID ordered
by UUID. Both admitted and live management/grant ceilings apply. Retained
dormant grants use the locked availability, not a cached preflight. Recheck
the credential deadline before commit; missing provider fails closed.
The same port guards the REST artifact grant/revoke owner transaction,
including exact idempotent retries, and the owner locks the target role
against deletion. No new public GraphQL fields or REST payloads.

Trusted low-level owner calls do not claim bearer authentication. Artifact
definition/runtime entitlement, policy interactions beyond static module
authority, tenant activity and global cross-owner lock ordering still
require verification. Source regressions authored; compile and PostgreSQL
contention tests not run.

## Standalone replacement ordering extension (2026-11-27)

PostgreSQL standalone replacement takes the target user fence, then the
existing target membership, destination and SuperAdmin continuity roles in
canonical UUID order before per-role checks. Bootstrap assignment fences
the user and exact system role before touching shared permission vocabulary;
a late role conflict fails via NOWAIT. Lazy missing-role creation and bulk
repair remain outside a complete cross-owner ordering proof. PostgreSQL
regression evidence is authored but unrun.
The PostgreSQL regression covers both historical opposite lock orders.
Compilation and execution evidence are pending.
