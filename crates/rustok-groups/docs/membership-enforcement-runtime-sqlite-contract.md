# Groups direct membership-enforcement SQLite runtime contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_membership_enforcement_runtime_sqlite.rs` retains executable source evidence for the direct `GroupMembershipEnforcementCommandPort` runtime boundary on SQLite: fail-closed security, exact local hierarchy/platform bypass, lifecycle-count invariance, receipt replay, and atomic audit/event/receipt facts.

The packet uses a real temporary SQLite database and every production Groups migration. All material enforcement mutations go through `GroupMembershipEnforcementCommandService`; direct SQL is limited to fixture setup and post-operation evidence reads. It introduces no Moderation adapter, alternate owner mutation, dependency, manifest, lockfile, registry promotion, or production behavior change.

## Denial and zero-side-effect contract

A dedicated fresh group starts with six lifecycle-active memberships at revision one: owner, administrator, moderator, and three ordinary members. The following direct commands must fail non-retryably:

- moderator self-target -> `groups.membership_enforcement_self_target`;
- platform `groups:moderate` actor without local membership targeting the owner -> `groups.membership_enforcement_owner_protected`;
- moderator targeting administrator -> `groups.manager_required`;
- ordinary member targeting ordinary member -> `groups.manager_required`.

After all four denials, evidence requires:

- group version remains one;
- lifecycle `member_count` remains six;
- every membership revision remains one;
- no enforcement row exists;
- no audit entry, semantic event, or command receipt exists.

This retains the hard rule that platform authority bypasses local-membership admission only; it never bypasses owner protection or self-target/hierarchy invariants.

## Exact hierarchy and platform bypass

A second fresh group proves every supported successful local tier through production suspend and revoke operations:

1. owner suspends/revokes administrator;
2. administrator suspends/revokes moderator;
3. moderator suspends/revokes ordinary member;
4. a non-member platform user carrying `groups:moderate` suspends/revokes another ordinary member.

Each material mutation advances the target membership revision exactly once and advances `groups.version` exactly once. Starting from group version one, eight material operations finish at version nine. Temporary enforcement never changes stored role/status or lifecycle `member_count`, which remains six.

The untouched ordinary member remains revision one. Every touched target ends revision three after one suspend plus one revoke. The group ledger contains exactly eight audit rows, eight membership semantic events and eight completed command receipts.

## Atomic receipt/audit/event lifecycle

A third fresh group isolates one owner/member lifecycle.

### Suspend

The owner suspends the ordinary member through `GroupMembershipEnforcementCommandPort` using idempotency key `atomic-suspend`. The commit must atomically produce:

- membership revision two;
- group version two;
- unchanged lifecycle member count six;
- one `group.membership_suspended` audit fact;
- one `groups.membership.suspended` membership event for the exact membership aggregate;
- one `groups.membership.suspend.v1` command receipt bound to the exact actor/group/key.

Exact replay with the same request/key returns `replayed=true` and adds no ledger row or version/revision change.

Reusing that key with a changed reason must fail non-retryably with `groups.conflict`. The failed changed request leaves group version, membership revision and all ledger counts unchanged.

### Revoke

The owner revokes at expected membership revision two with key `atomic-revoke`. The commit must atomically produce:

- membership revision three;
- group version three;
- unchanged member count six;
- non-null revocation timestamp;
- one `group.membership_suspension_revoked` audit fact;
- one `groups.membership.suspension_revoked` membership event;
- one `groups.membership.suspension_revoke.v1` receipt.

Exact revoke replay after the suspension is no longer active returns the stored result with `replayed=true` and does not duplicate any ledger fact.

## Final provenance

The final bounded enforcement row must preserve the original direct suspension provenance:

- `source_kind=direct_local`;
- `actor_kind=user`;
- `actor_id` equal to the owner UUID;
- enforcement revision equal to the owner result;
- revocation marker set.

The target remains stored `member/active` at revision three and the group remains lifecycle-count six at version three.

## Execution status

The packet was not executed while preparing this slice. FBA `membership_enforcement_command_runtime` remains null until maintainer execution.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_membership_enforcement_runtime_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-membership-enforcement-runtime-sqlite.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, browser/schema execution, or CI job was run while adding this source.
