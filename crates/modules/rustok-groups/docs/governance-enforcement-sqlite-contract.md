# Groups governance/enforcement SQLite evidence contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_governance_enforcement_sqlite.rs` is the SQLite counterpart to the PostgreSQL GROUPS-07 governance/enforcement evidence packet.

It runs against a real temporary SQLite file, not `sqlite::memory:`. Multiple production Groups services therefore use independent SeaORM pools against the same database file and exercise the actual SQLite writer-reservation path.

## Production surface

The fixture:

- requires the server `mod-groups` feature;
- uses the existing `tempfile` server dev-dependency;
- applies every production migration returned by `rustok_groups::migrations::migrations()`;
- uses `GroupGovernanceCommandPort` and `GroupMembershipEnforcementCommandPort` only;
- adds no crate dependency, manifest change, lockfile change, or alternative owner implementation.

## SQLite writer serialization

PostgreSQL/MySQL use row locks. SQLite cannot provide the same row-lock primitive, so the production owner protocol reserves the writer before mutable aggregate reads with:

```sql
UPDATE groups SET version = version WHERE tenant_id = ? AND id = ?
```

The file-backed test races governance and direct enforcement from separate pools. It accepts only the same two serialized outcomes as PostgreSQL:

- governance commits first, bumps membership revision through the role trigger, and the prepared suspension fails with `groups.membership_enforcement_revision_conflict`;
- suspension commits first, bumps membership revision through the enforcement trigger, and governance later fails with `groups.membership_suspended`.

Both commands succeeding is forbidden. The target revision must advance by exactly one material change.

## Replay parity

The SQLite packet also retains receipt-order evidence:

1. an active admin successfully changes a member role;
2. the owner suspends that admin;
3. the same admin replays the exact completed governance request and receives the stored result despite current suspension;
4. another actor using the same idempotency key receives `groups.conflict`.

This protects the same group-lock -> actor-bound receipt -> current effective authorization ordering as PostgreSQL.

## Platform recovery parity

As in the PostgreSQL packet, the fixture installs the already-defined moderation-owned enforcement projection for the current owner directly because the neutral Groups Moderation adapter remains a separate blocked slice.

The actual ownership transfer still goes through production `GroupGovernanceCommandPort` with `groups:manage`. The replacement must be effective-active; the suspended current owner is demoted to admin and `groups.owner_user_id` moves atomically to the replacement.

The fixture is only owner-state setup. It is not Moderation adapter evidence.

## Execution status

This source was not executed while preparing the PR. Until maintainers run it, SQLite concurrency/replay/recovery remain **execution pending**, not completed runtime evidence.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_governance_enforcement_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-governance-enforcement-sqlite.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
