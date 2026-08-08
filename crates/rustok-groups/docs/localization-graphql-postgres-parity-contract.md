# Groups localization native/GraphQL PostgreSQL parity contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_localization_graphql_postgres_parity.rs` is the PostgreSQL counterpart to the SQLite localization transport packet. It retains executable source evidence that native localization ports and the stable final Groups GraphQL roots expose the same Groups-owned semantics.

The packet uses a unique PostgreSQL schema, all production Groups migrations, `GroupLocalizationService`, and `graphql_application_cas::{GroupsQueryRoot, GroupsMutationRoot}`. It introduces no alternate localization owner, private GraphQL schema, fallback, dependency, or manifest change.

## PostgreSQL isolation

Every pooled connection receives the isolated schema through PostgreSQL startup options:

```text
options=-csearch_path=<schema>,public
```

The packet intentionally does not rely on session-local `SET search_path`.

## Equivalent owner fixtures

Two equivalent Groups start at the same aggregate version and are owned by the same authenticated user with no platform `groups:manage` permission. One group is exercised through native localization ports; the other through the final GraphQL roots.

## Parity contract

The packet requires parity for:

- English translation creation, including locale, title, summary, body, `created`, and group version;
- French translation creation with nullable summary and matching group version;
- ordered exact-locale reads through native `list_group_translations` and GraphQL `groupTranslations`;
- French translation deletion with matching locale and group version;
- last-translation deletion denial: native `groups.conflict`, non-retryable owner error, GraphQL `BAD_USER_INPUT`, and the same owner-safe message;
- final state containing only the English translation on both equivalent groups.

Local owner authorization remains a Groups owner-domain decision. GraphQL receives an empty effective permission list and does not use a platform-manage shortcut.

## Final-root composition

The schema uses the module's stable `graphql_application_cas::GroupsQueryRoot` and `GroupsMutationRoot`. It does not instantiate `GroupsLocalizationMutation` directly.

## Execution status

The test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. FBA `localization_transport_parity` remains null until maintainer execution.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_localization_graphql_postgres_parity -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-localization-graphql-postgres-parity.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, browser/schema execution, workflow, or CI job was run while adding this source.
