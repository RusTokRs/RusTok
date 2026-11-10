# Groups membership-subject owner-lock contract

Status: **source-complete / execution evidence pending**

## Purpose

`lock_membership_enforcement_target_by_id_for_update` is the Groups-owned lock primitive for trusted producer paths that receive a revisioned `group_memberships.id` instead of a `(group_id, user_id)` tuple.

It is intentionally crate-private. It does not authorize moderation, create receipts, interpret external decisions, or expose Groups persistence outside the owner crate.

## Lock protocol

A membership UUID cannot reveal its owning group without a lookup, so the primitive uses this sequence:

0. on SQLite, reserve the database writer before opening a locator snapshot, using
   the shared empty receipt UPDATE; other supported backends need no database-wide reservation;
1. perform a tenant-scoped membership **locator read** by `group_memberships.id`;
2. treat only `group_id` and `user_id` from that row as aggregate-location facts;
3. acquire the canonical Groups group writer reservation;
4. read the group under that retained reservation;
5. re-read the membership under owner lock and require the same group/user identity;
6. lock the current `group_membership_enforcements` row when present;
7. return only the locked owner rows to the caller.

The canonical mutation order therefore remains:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

The locator row is never sufficient for authorization, revision validation, provenance validation, or mutation. If the subject disappears between lookup and owner locking, the primitive returns `None`. If immutable aggregate identity disagrees after locking, it fails with an owner invariant instead of silently retargeting the subject.

PostgreSQL/MySQL use row locks. SQLite reserves its writer before the initial locator read, then retains the existing no-op `groups.version` reservation and membership/enforcement re-reads. Reserving only after the locator could fail read-snapshot promotion under concurrent writes.

## Moderation handoff

The neutral Groups moderation adapter performs its durable producer receipt admission **before** calling this primitive. After admission, it can lock the membership subject by immutable UUID, validate exact reviewed revision/effect/provenance against the returned rows, and invoke the existing shared Groups enforcement mutation.

Groups composes the existing Moderation adapter and platform Outbox idempotency owner;
the locking primitive does not add a second receipt protocol. Adapter rollback and
terminal-receipt persistence failures propagate as retryable finalization errors,
preserving the original domain error in diagnostic context.

## Execution status

No Cargo command, test, Node verifier, formatter, migration, workflow, or CI job was run while adding this primitive. Runtime contention evidence remains pending.
