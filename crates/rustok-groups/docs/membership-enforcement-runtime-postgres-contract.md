# Groups direct membership-enforcement PostgreSQL runtime contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_membership_enforcement_runtime_postgres.rs` is the PostgreSQL counterpart to the SQLite direct-command runtime packet. It retains executable source evidence for fail-closed security, exact local hierarchy/platform authority, lifecycle-count invariance, receipt replay, and atomic audit/event/receipt facts through the production `GroupMembershipEnforcementCommandPort`.

The packet adds no alternate owner implementation, Moderation adapter, dependency, manifest, lockfile, registry promotion, GraphQL path, or production behavior change. Direct SQL is limited to fixture setup and post-operation evidence reads.

## PostgreSQL isolation

One unique PostgreSQL schema is created for the ignored runtime packet. Every SeaORM connection used by the production service receives that schema through startup options:

```text
options=-csearch_path=<schema>,public
```

The source intentionally does not rely on session-local `SET search_path`. After all three fresh-tenant scenarios complete, the scoped connection is dropped and the schema is removed through the administrative connection.

## Denial and zero-side-effect contract

A fresh six-member group exercises four non-retryable denials:

- moderator self-target -> `groups.membership_enforcement_self_target`;
- non-member platform `groups:moderate` actor targeting the current owner -> `groups.membership_enforcement_owner_protected`;
- moderator targeting administrator -> `groups.manager_required`;
- ordinary member targeting ordinary member -> `groups.manager_required`.

After the denials, evidence requires group version one, lifecycle member count six, every membership revision one, no enforcement row, and no audit/event/receipt row. Platform authority therefore bypasses local-membership admission only and never bypasses hard owner protection or hierarchy/self-target invariants.

## Exact hierarchy and platform bypass

A second fresh group proves all supported direct tiers:

1. owner suspends/revokes administrator;
2. administrator suspends/revokes moderator;
3. moderator suspends/revokes ordinary member;
4. a non-member `groups:moderate` user suspends/revokes another ordinary member.

Each suspend/revoke is a production owner command. Eight material mutations advance group version from one to nine, touched target membership revisions from one to three, and never alter stored role/status or lifecycle member count six. The untouched member remains revision one. Exactly eight audit facts, eight membership semantic events and eight completed receipts exist for that fresh tenant/group.

## Atomic receipt/audit/event lifecycle

A third fresh group isolates one owner/member lifecycle.

The suspension commit at expected membership revision one must atomically produce membership revision two, group version two, unchanged member count six, one `group.membership_suspended` audit fact, one `groups.membership.suspended` event and one `groups.membership.suspend.v1` receipt for the exact actor/group/idempotency identity.

Exact same-key replay returns `replayed=true` without additional version/revision/ledger rows. Reusing the same key with a changed request fails non-retryably with `groups.conflict` and leaves state/ledger counts unchanged.

The revoke at expected membership revision two must atomically produce membership revision three, group version three, non-null revocation, one `group.membership_suspension_revoked` audit fact, one `groups.membership.suspension_revoked` event and one `groups.membership.suspension_revoke.v1` receipt. Exact revoke replay after current enforcement becomes inactive returns the stored result without duplicating ledger facts.

## Final provenance

The final bounded enforcement row must retain:

- `source_kind=direct_local`;
- `actor_kind=user`;
- original owner UUID as `actor_id`;
- the owner-result enforcement revision;
- non-null revocation marker.

The target remains stored `member/active` at membership revision three, group version three, and lifecycle member count six.

## Execution status

The test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. FBA `membership_enforcement_command_runtime` remains null until maintainer execution.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_membership_enforcement_runtime_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-membership-enforcement-runtime-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, browser/schema execution, or CI job was run while adding this source.
