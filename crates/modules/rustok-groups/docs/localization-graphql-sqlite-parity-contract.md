# Groups localization native/GraphQL SQLite parity contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_localization_graphql_sqlite_parity.rs` retains executable source evidence that the stable final Groups GraphQL root and the native localization ports expose the same Groups-owned localization semantics on SQLite.

The packet uses one real temporary file-backed SQLite database, all production Groups migrations, `GroupLocalizationService`, and `graphql_application_cas::{GroupsQueryRoot, GroupsMutationRoot}`. It adds no alternate GraphQL mutation path, direct localization-table mutation, fallback, dependency, or manifest change.

## Equivalent owner fixtures

The test creates two equivalent public Groups owned by the same authenticated user with no platform `groups:manage` permission:

- one group is exercised through `GroupLocalizationReadPort` / `GroupLocalizationCommandPort`;
- the other is exercised through the stable final GraphQL root.

Because each group starts with the same aggregate version and equivalent owner membership, matching mutations must return matching owner versions and localization payload semantics without pretending that two transports mutate one aggregate concurrently.

## Parity contract

The packet requires parity for:

- English translation creation, including normalized locale, title, summary, body, `created`, and group version;
- French translation creation with nullable summary and matching group version;
- exact-locale ordered management reads through native `list_group_translations` and GraphQL `groupTranslations`;
- French translation deletion with matching locale and group version;
- last-translation deletion denial: native stable `groups.conflict`, non-retryable owner failure, GraphQL `BAD_USER_INPUT`, and the exact same owner-safe message;
- final state containing only the English translation on both equivalent groups.

The GraphQL request carries the exact tenant-bound owner principal and an empty permission list. Local owner authority therefore remains a Groups owner-domain decision rather than a transport-side platform permission shortcut.

## Final-root composition

The test builds the same stable `graphql_application_cas::GroupsQueryRoot` / `GroupsMutationRoot` entrypoints published by the module manifest. Localization reaches that root through the existing merged chain; the evidence does not instantiate `GroupsLocalizationMutation` as a private alternate schema.

## Execution status

The packet was not executed while preparing this slice. FBA `localization_transport_parity` therefore remains null until maintainer execution.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_localization_graphql_sqlite_parity -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-localization-graphql-sqlite-parity.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, browser/schema execution, workflow, or CI job was run while adding this source.
