# Groups localization enforcement PostgreSQL concurrency contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_localization_enforcement_concurrency_postgres.rs` retains executable source evidence that localization management writes and membership suspension serialize through the shared Groups aggregate lock instead of authorizing against stale membership state.

The packet uses a unique PostgreSQL schema, all production Groups migrations, independent SeaORM pools, `GroupLocalizationService`, and `GroupMembershipEnforcementCommandService`. It adds no test-only owner mutation path.

## Race contract

Each of twelve unique fixtures contains one lifecycle-active owner and one lifecycle-active administrator. The owner first creates the baseline English translation. A barrier then releases two independent tasks at the same time:

- administrator upsert of a French translation through `GroupLocalizationCommandPort`;
- owner suspension of that administrator through `GroupMembershipEnforcementCommandPort` with expected membership revision `1`.

Both commands serialize on the same Groups aggregate writer lock. The suspension must always commit successfully. Exactly these localization outcomes are valid:

1. **Localization wins the Group lock first.** The French translation commits while the administrator is still effective-active. Suspension then commits afterward. Both commands succeed, and the localization result's group version must be lower than the suspension result's group version.
2. **Suspension wins the Group lock first.** Localization resumes only after suspension commit, re-evaluates effective manager state, fails with stable `groups.membership_suspended`, and creates no French translation.

A localization success with a group version at or after the suspension result is forbidden. A denied localization write that leaves French translation state is forbidden.

After every round a fresh production `GroupMembershipEnforcementReadPort` call must report the administrator as effective `Suspended`, inactive for access, and at membership revision `2`. Stored lifecycle member count remains `2` through the suspension result.

## PostgreSQL isolation

Every pool is connected with schema startup options:

```text
options=-csearch_path=<schema>,public
```

No session-local `SET search_path` or direct enforcement-table write is used.

## Execution status

The packet is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. The FBA `localization_concurrency` field therefore remains null until maintainer execution.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_localization_enforcement_concurrency_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-localization-enforcement-concurrency-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
