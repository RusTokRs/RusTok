# Groups governance/enforcement PostgreSQL stress contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_governance_enforcement_stress_postgres.rs` is the PostgreSQL counterpart to the SQLite bounded fan-out packet. It retains executable source evidence that the canonical GROUPS-07 owner serialization order remains deadlock-free and exact under many simultaneous writers to one group aggregate.

The packet adds no alternate owner implementation, retry shim, dependency, manifest, lockfile, registry promotion, or production behavior change.

## PostgreSQL isolation

The test creates one unique PostgreSQL schema and gives every pooled connection that schema through startup options:

```text
options=-csearch_path=<schema>,public
```

It never relies on session-local `SET search_path`. Every command uses a pre-opened single-connection pool before the shared barrier is released, so connection setup is not part of barrier participation.

## Fan-out shape

The packet runs three independent rounds. Each round creates one fresh tenant/group containing one active owner and eight ordinary active target memberships at revision one.

A shared barrier releases sixteen production owner commands simultaneously:

- eight `GroupGovernanceCommandPort::change_group_role` calls requesting `member -> moderator`;
- eight `GroupMembershipEnforcementCommandPort::suspend_membership` calls prepared against revision one.

All sixteen calls address the same group aggregate. Every target pair is bounded by a 30-second timeout. PostgreSQL deadlock errors, task panic, timeout, retryable persistence failures, both-success or both-fail are evidence failures.

## Allowed per-target outcomes

Exactly one material mutation may commit for each target.

If governance serializes first:

- role becomes `moderator`;
- membership revision becomes two;
- suspension fails non-retryably with `groups.membership_enforcement_revision_conflict`;
- no active enforcement row exists.

If suspension serializes first:

- role remains `member`;
- membership revision becomes two;
- governance fails non-retryably with `groups.membership_suspended`;
- exactly one active suspension row exists.

No storage-level or scheduler-specific error is treated as a valid business outcome.

## Aggregate invariants

After every round:

- exactly eight material owner mutations have committed;
- every target revision is exactly two;
- `groups.version == base_version + 8`;
- lifecycle `member_count` remains nine;
- every target's role/enforcement projection matches exactly one serialized winner.

The packet therefore exercises the canonical `Group -> GroupMembership -> GroupMembershipEnforcement` lock order under bounded same-aggregate fan-out rather than repeating only one two-command race.

## Execution status

The test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. FBA `governance_concurrency` remains null until maintainer execution.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_governance_enforcement_stress_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-governance-enforcement-stress-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
