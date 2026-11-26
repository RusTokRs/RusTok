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
must not disable triggers, reuse role UUIDs, or use SQLite REPLACE on parent
roles/permissions or conflict-update parent identities. Existing owner
writers use explicit insert-do-nothing and delete/update.

Auth locks the issuer user first, then all issuer membership role rows
and the exact destination role ordered by ID, before reading grants.
The ledger stores the canonical destination ID and sorted role ID/epoch
pairs, not a global cache generation. No missing-target fallback at
acceptance. A missing built-in role is materialized at issuance using the
RBAC bootstrap owner; a concurrent creator's grants are not reconciled by
the loser. Failure rolls back that initialization too. A missing custom
role is rejected. A replacement role ID or any bound grant change denies
the old invite even if current grants again match. Existing target
roles and issuer roles are held until commit. Low-level writers using
opposite lock orders can deadlock; retry entire failed transactions.

This covers static role grants, not artifact-defined permissions (separate
authorization plane) or module-policy revisions. No wire change. Database
rollback, disabled triggers and trusted administrator bypasses are not
protected. Legacy unbound pending invites are revoked at cutover; accepted
receipts and prior revocations remain. Drain old binaries before migrations;
no live schema downgrade. Source regressions are authored, compilation and
PostgreSQL concurrency/upgrade/rollback tests remain unrun.
