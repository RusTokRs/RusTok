# Groups feature-settings effective authorization contract

Status: **source complete / maintainer runtime execution pending**

`GroupCommandPort::set_group_feature` mutates Groups-owned feature-binding state. Manager authorization therefore belongs inside the same Groups owner transaction as the feature write.

## Owner transaction

The effective `GroupsService` now performs feature mutation in this order:

1. validate the write `PortContext`, tenant and user actor;
2. begin one Groups transaction;
3. reserve the group writer with `reserve_group_write_for_update`;
4. confirm the tenant/group owner aggregate exists;
5. evaluate `GroupManagerCapability::ManageSettings` with `require_effective_manager_owned`;
6. read and insert/update the feature binding through that transaction;
7. advance `groups.version` in the same transaction;
8. commit once.

The authorization lock boundary remains the canonical:

```text
Group -> GroupMembership -> GroupMembershipEnforcement
```

PostgreSQL/MySQL retain row locks. SQLite uses the shared no-op group update writer reservation. A concurrent enforcement mutation cannot commit a suspension between manager authorization and the feature mutation.

## Effective authority

Local authority requires an effective-active owner or administrator. Stored lifecycle `active` is compatibility state only and cannot restore authority while an effective suspension is present.

Stable owner failures are preserved:

- active suspension: `groups.membership_suspended`;
- legacy ban: `groups.membership_banned`;
- inactive membership or insufficient role: `groups.manager_required`.

Platform `groups:manage`, `groups:*`, and `*:*` authority remains an explicit bypass of local membership requirements, but still enters the owner transaction and group serialization boundary before feature mutation.

## Aggregate semantics

Feature insert/update and the corresponding `groups.version` increment are atomic. Feature settings do not change stored membership lifecycle status or `groups.member_count`.

No receipt, alternate owner, direct enforcement write, Moderation dependency, fallback, manifest change, or lockfile change is introduced by this slice.

## Evidence status

This source change was not executed while preparing the slice. SQLite/PostgreSQL suspension/expiry and enforcement-vs-feature-write contention evidence remains maintainer-owned and the canonical plan stays `in_progress`.

Source guard:

```bash
node scripts/verify/verify-groups-feature-enforcement-authorization.mjs
```

No Cargo command, Rust test, Node verifier, formatter, migration execution, workflow, or CI job was run while preparing this source.