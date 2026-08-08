# Groups join/rejoin effective enforcement contract

Status: **source complete / maintainer runtime execution pending**

`GroupCommandPort::join_group` changes Groups-owned membership lifecycle state and may change the aggregate member count/version. Re-entry authorization therefore belongs under the same owner serialization boundary as membership enforcement.

## Owner transaction

The effective `GroupsService` now performs join/rejoin in this order:

1. validate the write context, tenant and user actor;
2. begin one Groups transaction;
3. reserve the group writer with `reserve_group_write_for_update`;
4. require the group to exist and remain active;
5. resolve the actor membership with `resolve_group_membership_enforcement_for_update`;
6. reject effective suspension or legacy ban before lifecycle mutation;
7. evaluate the existing invitation/join-policy lifecycle rules;
8. insert or update membership state in the same transaction;
9. when the result is lifecycle-active, advance `groups.member_count` and `groups.version` atomically;
10. commit once.

The write boundary therefore follows the canonical lock order:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

PostgreSQL/MySQL retain row locks. SQLite uses the shared no-op group update writer reservation before membership/enforcement reads.

## Stable re-entry failures

Stored lifecycle state is not sufficient to authorize re-entry. An active enforcement row is evaluated on the Groups owner clock inside the transaction.

- effective suspension returns `groups.membership_suspended`;
- legacy banned state returns `groups.membership_banned`;
- corrupt/unsupported enforcement remains fail-closed through the resolver.

Visibility, join-policy, invitation acceptance behavior and the existing active-member idempotent result remain unchanged.

## Concurrent enforcement semantics

The group writer reservation prevents a suspension from committing between the join authorization read and membership mutation.

If join/rejoin serializes first, its lifecycle mutation/revision effect becomes visible before a later enforcement command continues, so a suspension prepared against the old membership revision must conflict. If suspension serializes first, the join transaction observes effective suspension and writes no lifecycle/member-count state.

This slice does not add a new receipt, alternate owner, direct enforcement mutation, dependency, manifest, lockfile, migration, or Moderation coupling.

## Evidence status

Compilation and SQLite/PostgreSQL suspension-versus-join contention remain maintainer execution gates. Source presence does not promote GROUPS-07 to done.

Source guard:

```bash
node scripts/verify/verify-groups-join-enforcement-authorization.mjs
```

No Cargo command, Rust test, Node verifier, formatter, migration execution, workflow, or CI job was run while preparing this source.
