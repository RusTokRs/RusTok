# Groups localization enforcement SQLite concurrency contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_localization_enforcement_concurrency_sqlite.rs` retains executable source evidence that localization management writes and membership suspension serialize through the shared Groups SQLite writer reservation instead of authorizing against stale membership state.

The packet uses one real temporary file-backed SQLite database, WAL mode, a bounded busy timeout, independent single-connection SeaORM pools, all production Groups migrations, `GroupLocalizationService`, and `GroupMembershipEnforcementCommandService`. It introduces no test-only owner mutation path.

## Race contract

Each of twelve unique fixtures contains one lifecycle-active owner and one lifecycle-active administrator. The owner first creates the baseline English translation. A barrier then releases two independent tasks at the same time:

- administrator upsert of a French translation through `GroupLocalizationCommandPort`;
- owner suspension of that administrator through `GroupMembershipEnforcementCommandPort` with expected membership revision `1`.

Both commands enter the canonical Groups owner write protocol. On SQLite, `reserve_group_write_for_update` obtains the writer reservation through the production no-op group update before mutable authorization state is read.

The suspension must always commit successfully. Exactly these localization outcomes are valid:

1. **Localization wins the writer reservation first.** The French translation commits while the administrator is still effective-active. Suspension then commits afterward. Both commands succeed, and the localization result's group version must be lower than the suspension result's group version.
2. **Suspension wins the writer reservation first.** Localization continues only after suspension commit, re-evaluates effective manager state, fails with stable `groups.membership_suspended`, and creates no French translation.

A localization success with a group version at or after the suspension result is forbidden. A denied localization write that leaves French translation state is forbidden. SQLite lock acquisition errors are also forbidden outcomes: the evidence configures `PRAGMA busy_timeout = 5000` and must observe domain serialization, not surface a storage-lock race as business behavior.

After every round a fresh production `GroupMembershipEnforcementReadPort` call must report the administrator as effective `Suspended`, inactive for access, and at membership revision `2`. The suspension result must retain lifecycle `member_count=2`.

## SQLite isolation

The packet uses:

```text
sqlite://<temporary-file>?mode=rwc
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
```

Each racing service owns an independent single-connection pool against the same file, so the evidence exercises real SQLite writer arbitration rather than reusing one connection.

## Execution status

The packet was not executed while preparing this slice. The FBA `localization_concurrency` field therefore remains null until maintainer execution.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_localization_enforcement_concurrency_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-localization-enforcement-concurrency-sqlite.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
