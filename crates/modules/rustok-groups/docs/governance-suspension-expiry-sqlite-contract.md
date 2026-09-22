# Groups governance suspension/expiry SQLite evidence contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_governance_suspension_expiry_sqlite.rs` retains executable source evidence that local governance authority follows Groups owner-clock effective membership rather than raw stored role/status on SQLite.

The packet uses a real temporary SQLite file, every production Groups migration, `GroupGovernanceService`, and `GroupMembershipEnforcementCommandService`. It introduces no cleanup worker, alternate governance authorization path, dependency, manifest, or lockfile change.

## Contract

The fixture contains one owner, one administrator, and two ordinary active members. Before enforcement, the administrator proves real local governance authority by changing the first member from `member` to `moderator` through `GroupGovernanceCommandPort`.

The owner then applies a short production suspension to the administrator. While the suspension is effective, the evidence requires:

- the administrator's stored role remains `admin`;
- stored membership status remains `active`;
- membership revision advances exactly for the suspension mutation;
- stored lifecycle `member_count` remains unchanged;
- a fresh administrator role-change command for the second member fails with stable `groups.membership_suspended` and `retryable=false`;
- the denied command leaves the second member's role/status/revision unchanged.

After the Groups owner clock passes `effective_until`, the test performs **no revoke or cleanup mutation**. A fresh administrator governance command must then succeed for the same second member. The evidence requires:

- the second member changes from `member` to `moderator`;
- the restored governance command advances group version exactly once after the suspension version;
- the target membership revision advances exactly once for the role mutation;
- the administrator remains stored `admin/active`;
- the administrator membership revision remains exactly the suspension revision, proving expiry itself creates no synthetic revision change;
- lifecycle `member_count` remains unchanged.

This source complements the existing SQLite governance/enforcement replay, role-versus-suspension race, and platform ownership recovery packet. It does not duplicate those contracts and does not claim GraphQL/native governance parity.

## Execution status

The packet was not executed while preparing this slice. Governance runtime evidence remains **maintainer execution pending**.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_governance_suspension_expiry_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-governance-suspension-expiry-sqlite.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
