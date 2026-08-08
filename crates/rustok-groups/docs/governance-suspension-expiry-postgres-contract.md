# Groups governance suspension/expiry PostgreSQL evidence contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_governance_suspension_expiry_postgres.rs` is the PostgreSQL counterpart to the SQLite governance suspension/expiry packet. It retains executable source evidence that local governance authority follows Groups owner-clock effective membership rather than raw stored role/status.

The packet uses a unique PostgreSQL schema, every production Groups migration, `GroupGovernanceService`, and `GroupMembershipEnforcementCommandService`. It introduces no cleanup worker, alternate governance authorization path, dependency, manifest, or lockfile change.

## PostgreSQL isolation

Every pooled connection receives the isolated schema through PostgreSQL startup options:

```text
options=-csearch_path=<schema>,public
```

The packet intentionally does not rely on session-local `SET search_path`.

## Contract

The fixture contains one owner, one administrator, and two ordinary active members. Before enforcement, the administrator proves real local governance authority by changing the first member from `member` to `moderator` through `GroupGovernanceCommandPort`.

The owner then applies a short production suspension to the administrator. While effective, the evidence requires:

- stored administrator role remains `admin`;
- stored membership status remains `active`;
- administrator membership revision advances exactly for suspension;
- stored lifecycle `member_count` remains unchanged;
- a fresh administrator role-change command for the second member fails with `groups.membership_suspended` and `retryable=false`;
- the denied command leaves the target role/status/revision unchanged.

After `effective_until`, the test performs **no revoke or cleanup mutation**. The same administrator must regain governance authority with a fresh role command. The restored command must advance group version exactly once after the suspension version; the target membership revision advances exactly once; the administrator remains stored `admin/active` at the exact suspension revision; lifecycle `member_count` remains unchanged.

This source complements the existing PostgreSQL governance/enforcement replay, role-versus-suspension race, and platform ownership recovery packet. It does not duplicate those contracts and does not claim native/GraphQL governance parity.

## Execution status

The test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. Governance runtime evidence remains **maintainer execution pending**.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_governance_suspension_expiry_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-governance-suspension-expiry-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
