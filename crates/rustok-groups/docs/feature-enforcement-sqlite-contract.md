# Groups feature-settings enforcement SQLite evidence contract

Status: **executable source added / maintainer execution pending**

`apps/server/tests/groups_feature_enforcement_sqlite.rs` retains file-backed SQLite evidence for the transaction-aware Groups feature-settings authorization boundary introduced by `set_group_feature`.

## Storage profile

The packet uses one temporary SQLite file, production Groups migrations, one-connection SeaORM pools, `PRAGMA busy_timeout = 5000`, and WAL mode. It does not use `sqlite::memory:` and does not accept `database is locked` or persistence-unavailable behavior as a domain result.

## Suspension and owner-clock expiry

An effective-active administrator first writes `forum.discussions` through `GroupCommandPort::set_group_feature`. The feature write advances `groups.version` by one.

The group owner then applies a temporary suspension through the production `GroupMembershipEnforcementCommandPort`. While stored membership remains lifecycle `active`, a fresh feature write by that administrator must fail non-retryably with `groups.membership_suspended`, leave feature configuration unchanged, and leave `groups.version` unchanged.

The packet reads effective state through `GroupMembershipEnforcementReadPort`. After `effective_until`, the same administrator must become effective-active again without revoke or cleanup and may update the feature. Expiry itself must not synthesize a membership revision or change lifecycle `member_count`.

## Enforcement-versus-feature-write serialization

Eight independent race fixtures release a feature mutation and suspension through one barrier, using separate pre-opened SQLite connections. The commands address the same group and administrator.

Only these serialized outcomes are allowed:

- feature write commits first, then suspension commits; the aggregate advances by two versions and the feature exists;
- suspension commits first, then the feature write fails with `groups.membership_suspended`; the aggregate advances only for suspension and no feature row is exposed by the owner read port.

Suspension must always commit with membership revision two. The stored membership remains `active`, lifecycle `member_count` stays two, and final group version must equal the suspension result. A retryable/storage error, task panic, partial feature write, or extra aggregate version advance is evidence failure.

## Owner surfaces only

Feature mutation uses `GroupsService` through `GroupCommandPort`; enforcement uses the production command port; feature observation uses `GroupAccessReadPort::enabled_group_features`; effective membership observation uses `GroupMembershipEnforcementReadPort`. Raw SQL is limited to fixture creation and diagnostic aggregate/lifecycle revision reads.

No production behavior, dependency, manifest, lockfile, migration, Moderation adapter, or fallback is introduced by this evidence slice.

## Execution status

The packet was not executed while preparing this slice. The canonical plan therefore remains `in_progress` and the feature-settings runtime/concurrency gate remains maintainer-owned.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_feature_enforcement_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-feature-enforcement-sqlite.mjs
```

No Cargo command, Rust test, Node verifier, formatter, migration execution, workflow, or CI job was run while adding this source.
