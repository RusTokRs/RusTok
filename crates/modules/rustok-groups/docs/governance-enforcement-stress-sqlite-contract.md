# Groups governance/enforcement SQLite stress contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_governance_enforcement_stress_sqlite.rs` retains bounded stress evidence for the GROUPS-07 owner serialization contract under high same-aggregate contention on SQLite.

The packet complements the single-target governance/enforcement race source. It does not add another owner implementation, retry shim, cleanup worker, dependency, manifest, lockfile, or production behavior change.

## SQLite storage contract

The fixture uses one real temporary SQLite file. Every independent command connection is configured with:

- one pooled connection;
- `PRAGMA busy_timeout = 5000`;
- the fixture database in WAL mode.

This means `database is locked` / storage-unavailable behavior is not accepted as a domain outcome. SQLite must serialize through the production no-op group writer reservation.

## Fan-out shape

The packet runs three independent rounds. Each round creates one fresh tenant/group containing:

- one active owner;
- eight active ordinary target memberships at revision one.

A shared Tokio barrier releases sixteen production owner commands at once:

- one `GroupGovernanceCommandPort::change_group_role` per target, requesting `member -> moderator`;
- one `GroupMembershipEnforcementCommandPort::suspend_membership` per target, prepared against membership revision one.

All commands address the same group aggregate. Every pair is bounded by a 30-second timeout so a deadlock, stalled writer reservation, task panic or unexpectedly retryable storage failure is evidence failure rather than an accepted result.

## Allowed per-target outcomes

Exactly one material owner mutation must commit per target.

If governance wins first:

- stored role becomes `moderator`;
- membership revision advances to two;
- the prepared suspension fails non-retryably with `groups.membership_enforcement_revision_conflict`;
- no active enforcement row exists.

If suspension wins first:

- stored role remains `member`;
- membership revision advances to two;
- governance fails non-retryably with `groups.membership_suspended`;
- exactly one active suspension row exists.

Both commands succeeding, both failing, timeout, task panic or any other error code fails the packet.

## Aggregate invariants

After each round the evidence requires:

- exactly eight successful material owner mutations in total;
- every target membership revision is exactly two;
- `groups.version == base_version + 8`;
- lifecycle `member_count` remains nine;
- every final target state matches exactly one serialized winner.

This retains evidence that queueing many same-group writers does not weaken the canonical `Group -> GroupMembership -> GroupMembershipEnforcement` owner protocol.

## Execution status

The packet was not executed while preparing this slice. FBA `governance_concurrency` remains null until maintainer execution.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_governance_enforcement_stress_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-governance-enforcement-stress-sqlite.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
