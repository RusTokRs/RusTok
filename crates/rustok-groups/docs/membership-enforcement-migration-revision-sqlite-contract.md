# Groups membership-enforcement migration/revision SQLite contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_membership_enforcement_migration_revision_sqlite.rs` retains focused SQLite source evidence for two GROUPS-07 foundation gates:

- migration `m20260723_000008_create_group_membership_enforcement_state` backfill/schema behavior;
- monotonic `group_memberships.revision` runtime behavior across material membership changes and enforcement mutations.

The packet uses the canonical `rustok_groups::migrations::migrations()` order. It does not recreate migration SQL or use an alternate revision implementation.

## Real pre-000008 backfill

The test applies only the first seven production Groups migrations, then inserts one group with an owner and ordinary active member. At that point it requires:

- `group_memberships.revision` is absent;
- `group_membership_enforcements` is absent.

It then applies migration index seven, which the canonical migration list reserves for `m20260723_000008_create_group_membership_enforcement_state`.

After `000008`:

- exactly one `revision` column exists on `group_memberships`;
- the bounded enforcement projection table exists;
- both pre-existing membership rows are backfilled to revision one.

This is a real migration-boundary fixture rather than a row created after the new schema already existed.

## Membership revision monotonicity

The target membership starts at revision one. A material direct role change `member -> moderator` advances revision to two. An explicit attempt to decrease revision back to one must fail and leave revision two.

After the production enforcement owner lifecycle completes, a material stored lifecycle change `active -> left` advances revision from four to five. A later explicit decrease is again rejected and revision remains five.

The lifecycle SQL at the end is trigger evidence only and is intentionally performed after production enforcement owner assertions. It is not a replacement for a lifecycle owner command or member-count mutation.

The packet intentionally does not define a cross-backend contract for arbitrary SQL no-op updates. Owner-visible behavior is the monotonic revision contract plus revision advances for material owner-domain changes.

## Enforcement-trigger revision sources

After applying all remaining production Groups migrations, the real `GroupMembershipEnforcementCommandService` consumes expected target revision two.

A direct owner suspension must produce:

- target membership revision three;
- enforcement revision one;
- group version two;
- unchanged stored lifecycle member count two.

A direct owner revoke at expected membership revision three must produce:

- target membership revision four;
- enforcement revision two;
- non-null revocation timestamp;
- group version three;
- unchanged lifecycle member count two.

This retains evidence that enforcement insert/update trigger paths are monotonic revision sources and compose with command-level expected-revision CAS.

## Migration ordering

The source verifier checks that `migrations/mod.rs` still lists `m20260723_000008_create_group_membership_enforcement_state` immediately before `m20260808_000009_extend_group_domain_events_for_membership_enforcement`. The runtime fixture requires at least nine migrations, applies `take(7)`, then index seven, then every migration from `skip(8)`.

This keeps the evidence stable if later migrations are appended while failing loudly if the historical migration boundary is reordered.

## Execution status

The packet was not executed while preparing this slice. FBA `membership_enforcement_migration` and `membership_enforcement_revision_runtime` remain null until maintainer execution.

Maintainer command:

```bash
cargo test -p rustok-server --features mod-groups \
  --test groups_membership_enforcement_migration_revision_sqlite -- --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-membership-enforcement-migration-revision-sqlite.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, browser/schema execution, or CI job was run while adding this source.
