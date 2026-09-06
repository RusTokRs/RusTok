# Groups localization enforcement expiry PostgreSQL evidence contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_localization_enforcement_expiry_postgres.rs` is the PostgreSQL counterpart to the SQLite localization suspension/expiry packet. It retains executable source evidence that localization management authorization follows Groups owner-clock effective membership rather than stored lifecycle status.

The packet uses a unique PostgreSQL schema, all production Groups migrations, `GroupLocalizationService`, and `GroupMembershipEnforcementCommandService`. No alternate authorization path or cleanup process is introduced.

## PostgreSQL isolation

Every execution creates a unique schema and supplies it to every pooled connection through PostgreSQL startup options:

```text
options=-csearch_path=<schema>,public
```

The packet intentionally does not use session-local `SET search_path`.

## Contract

The fixture contains one lifecycle-active owner and one lifecycle-active administrator. The owner creates the initial translation through the production localization command. The administrator can read management translations before enforcement.

The owner then applies a short production suspension to the administrator. While effective, the evidence requires:

- stored membership status remains `active`;
- membership revision reflects exactly the suspension mutation;
- stored lifecycle `member_count` remains unchanged;
- management read fails with `groups.membership_suspended`;
- management translation upsert fails with the same stable owner code;
- the denied upsert creates no translation;
- the unsuspended owner retains management access.

After the owner clock passes `effective_until`, the test performs **no cleanup mutation**. The administrator must regain management read/write access, the restored translation write must advance group version exactly once, membership revision must remain unchanged since suspension, and lifecycle `member_count` must remain unchanged.

This is suspension/expiry backend-parity source only. It does not claim concurrent enforcement-vs-localization-write or native/GraphQL localization parity evidence.

## Execution status

The PostgreSQL packet is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. Runtime evidence remains **maintainer execution pending**; FBA localization runtime fields remain null.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_localization_enforcement_expiry_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-localization-enforcement-expiry-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
