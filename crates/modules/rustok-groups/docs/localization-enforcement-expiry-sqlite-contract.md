# Groups localization enforcement expiry SQLite evidence contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_localization_enforcement_expiry_sqlite.rs` retains executable evidence that localization management authorization follows the Groups owner-clock effective membership state instead of raw `group_memberships.status`.

The packet uses a real temporary SQLite file, every production Groups migration, `GroupLocalizationService`, and `GroupMembershipEnforcementCommandService`. It adds no alternate authorization path, cleanup worker, dependency, or manifest change.

## Contract

The fixture contains one lifecycle-active owner and one lifecycle-active administrator. The owner first creates the initial English translation through `GroupLocalizationCommandPort`; the administrator can then read the management translation surface normally.

The owner applies a short production membership suspension to the administrator. While that enforcement is effective, the evidence requires:

- stored membership lifecycle status remains `active`;
- stored lifecycle `member_count` remains unchanged;
- membership revision reflects the suspension mutation;
- `GroupLocalizationReadPort::list_group_translations` fails with `groups.membership_suspended`;
- `GroupLocalizationCommandPort::upsert_group_translation` fails with the same stable owner code;
- the denied write does not create the requested translation;
- the unsuspended owner still retains management access.

After the owner clock passes `effective_until`, the test performs **no cleanup mutation**. It then requires:

- the administrator can read management translations again;
- the administrator can create a new translation through the same production localization command;
- stored membership status is still `active`;
- membership revision has not changed since the original suspension;
- lifecycle `member_count` is still unchanged;
- the localization write advances the owner group version normally.

This is source evidence for localization suspension/expiry semantics only. It does not claim concurrent enforcement-vs-localization-write evidence or native/GraphQL localization parity.

## Execution status

The packet was not executed while preparing this slice. Runtime evidence remains **maintainer execution pending** and the FBA `localization_transport_parity` / `localization_concurrency` fields remain null.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_localization_enforcement_expiry_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-localization-boundary.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
