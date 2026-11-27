# Durable RBAC membership revision

- Date: 2026-11-27
- Decision status: Accepted
- Implementation status: In progress
- Owners: RBAC (membership history), Auth (invitation binding)
- Extends: [RBAC source of truth](2026-02-26-rbac-relation-source-of-truth-cutover.md), [Account recovery revision](2026-11-26-auth-recovery-revision.md)
- Supersedes: None
- Superseded by: None

## Context

Removal and reassignment of the same role can revive an older invitation.
Current grants and global cache generation are not a per-user history.

## Decision

RBAC owns a nonnegative signed 64-bit membership epoch on the user row.
Database triggers advance it on actual insert, delete or changed user/role
identity in user_roles, including cascades. A move advances both subjects
once. Exact no-ops and conflict-do-nothing inserts do not advance it; rewriting
a set via delete/insert conservatively advances it. Rollback rolls back the
epoch. Overflow and decreasing the epoch fail closed. PostgreSQL rejects
TRUNCATE of user_roles; maintenance must use row deletion. SQLite rejects
relation-ID replacement across users to prevent silent conflict-delete bypass.
Migration 000013 replaces the ordinary-DML non-reuse assumption with a
permanent UUID witness for all users present at cutover or inserted later.
A deleted user UUID cannot be recreated, and user ID updates are forbidden.
Trusted maintenance must not disable triggers; pre-cutover deleted identities
and privileged restore remain outside the witness history.

The epoch update locks the same user row as Auth's issuer mutation fence.
SQLite readers must already own its write fence. Writers should lock users
before changing relations. Migration 000006 adds a PostgreSQL BEFORE ROW
guard: lock old/new surviving user IDs in UUID order with FOR NO KEY UPDATE
NOWAIT, then old/new referenced roles for insert/update with FOR SHARE
NOWAIT. This precedes tenant validation and retains revision/FK authority
through the AFTER trigger and commit. A late competing writer fails with
SQLSTATE 55P03 instead of waiting for a parent while holding a relation row.
DELETE locks only the surviving revision parent; a cascaded deleted user
need not be resurrected. Active parent RLS is rejected because hidden rows
cannot establish complete authority. SQLite retains its single-writer
protocol. Retry the entire transaction after contention; bulk child-row
ordering and cascades before the row guard are not globally proven.

Auth stores the exact epoch separately from account recovery revision.
Acceptance checks both before consuming the credential. Migration revokes
unconsumed legacy invitations without guessing their epochs, retaining
consumption and prior revocation history. Down revokes outstanding invitations
before removing the binding. Drain old binaries before migrating and deploy
matching issuers and acceptors together; no live security-store downgrade.

## Boundaries and verification

No wire or public DTO changes. Role-grant and module-policy history require
separate epochs. Trusted database administration and joint database rollback
are outside this guarantee. Source regressions cover rollback, ABA, no-op,
owner transfer, cascade, overflow and cutover. Compilation, fresh/incremental
schema, PostgreSQL contention and Auth transport evidence remain unrun.
