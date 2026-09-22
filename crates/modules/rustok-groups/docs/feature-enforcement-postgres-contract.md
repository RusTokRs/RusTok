# Groups feature-settings enforcement PostgreSQL evidence contract

Status: **executable source added / maintainer execution pending**

`apps/server/tests/groups_feature_enforcement_postgres.rs` is the PostgreSQL counterpart to the SQLite feature-settings enforcement packet. It retains executable source evidence that feature management participates in the same Groups owner serialization boundary as membership enforcement.

## PostgreSQL isolation

The test creates one unique schema and supplies it to every pooled connection through PostgreSQL startup options:

```text
options=-csearch_path=<schema>,public
```

It intentionally does not rely on session-local `SET search_path`. Every connection uses one pool slot and production Groups migrations.

## Suspension and owner-clock expiry

An effective-active administrator first updates `forum.discussions` through `GroupCommandPort::set_group_feature`, advancing `groups.version` exactly once. The owner then applies a temporary suspension through the production enforcement command.

While stored membership remains lifecycle `active` at revision two, a fresh feature update must fail non-retryably with `groups.membership_suspended`, preserve the existing feature configuration, and leave aggregate version unchanged.

`GroupMembershipEnforcementReadPort` must report the active suspension and later report effective `Active` after `effective_until`. Without revoke or cleanup, the same administrator must then update the feature successfully. Expiry itself does not advance membership revision or change lifecycle `member_count`.

## Enforcement-versus-feature-write serialization

Eight fresh group fixtures race one feature write against one suspension through a shared barrier, with both command connections opened before the barrier is released.

Only two material outcomes are allowed:

- feature mutation serializes first and suspension follows, producing two aggregate version advances and the requested feature state;
- suspension serializes first and the feature mutation is denied with `groups.membership_suspended`, producing only the suspension version advance and no feature state.

Suspension must always commit with membership revision two. Stored membership remains `active`, member count remains two, and final group version equals the suspension result. PostgreSQL deadlock, persistence/retry fallback, task panic, partial feature mutation, or an extra aggregate advance is evidence failure.

## Owner surfaces only

Feature mutation uses `GroupsService` through `GroupCommandPort`, enforcement uses the production command port, feature observation uses `GroupAccessReadPort::enabled_group_features`, and effective state observation uses `GroupMembershipEnforcementReadPort`. Direct SQL is limited to isolated fixture creation and diagnostic aggregate/lifecycle reads.

No production behavior, dependency, manifest, lockfile, migration, Moderation adapter, or fallback is introduced by this evidence slice.

## Execution status

The test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. The canonical feature runtime/concurrency gate therefore remains maintainer-owned.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_feature_enforcement_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-feature-enforcement-postgres.mjs
```

No Cargo command, Rust test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
