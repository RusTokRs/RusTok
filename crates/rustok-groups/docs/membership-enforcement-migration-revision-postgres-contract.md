# Groups membership-enforcement migration/revision PostgreSQL contract

Status: **executable source added / maintainer execution pending**

## Purpose

`apps/server/tests/groups_membership_enforcement_migration_revision_postgres.rs` is the PostgreSQL counterpart to the SQLite migration/revision packet. It retains focused source evidence for the real pre-`000008` backfill boundary and monotonic membership revision sources across material membership changes and production enforcement mutations.

The source uses the canonical `rustok_groups::migrations::migrations()` order and does not recreate migration SQL or introduce an alternate revision owner.

## PostgreSQL isolation

The ignored packet creates a unique PostgreSQL schema and supplies it to the scoped SeaORM connection through startup options:

```text
options=-csearch_path=<schema>,public
```

No session-local `SET search_path` is used. The administrative connection creates and drops the isolated schema; every migration and owner command executes through the scoped connection.

## Real pre-000008 backfill

Only the first seven production Groups migrations are applied before the fixture inserts one group with an owner and ordinary active member.

Before enforcement migration `000008`, `information_schema` must report:

- no `group_memberships.revision` column in `current_schema()`;
- no `group_membership_enforcements` table in `current_schema()`.

Migration index seven is then applied as the canonical `m20260723_000008_create_group_membership_enforcement_state` boundary. Both existing membership rows must appear with revision one and the bounded enforcement projection table must exist.

## Membership revision monotonicity

A material target role change `member -> moderator` advances revision from one to two. An explicit decrease back to one must fail and leave revision two.

After the production enforcement lifecycle, a material stored lifecycle change `active -> left` advances revision from four to five; a later explicit decrease is again rejected.

The final lifecycle SQL exists only to retain trigger evidence after owner-command assertions. It is not treated as a lifecycle owner command or member-count mutation.

As in the SQLite packet, arbitrary SQL no-op update behavior is intentionally outside this cross-backend contract. The evidence contract is monotonicity plus revision advancement for material owner-domain changes.

## Enforcement-trigger revision sources

After all remaining production Groups migrations are applied, `GroupMembershipEnforcementCommandService` consumes expected target revision two.

The production owner suspension must produce membership revision three, enforcement revision one, group version two and unchanged lifecycle member count two.

The production owner revoke at expected revision three must produce membership revision four, enforcement revision two, non-null revocation, group version three and unchanged lifecycle member count two.

This retains PostgreSQL evidence that the enforcement projection insert/update triggers advance the membership subject revision and compose with expected-revision CAS.

## Migration ordering

The source verifier retains the same historical ordering requirement as SQLite: `m20260723_000008_create_group_membership_enforcement_state` must precede `m20260808_000009_extend_group_domain_events_for_membership_enforcement` in `migrations/mod.rs`.

The runtime source requires at least nine migrations, applies `take(7)`, then migration index seven, then every migration from `skip(8)` so later appended migrations remain compatible with the fixture.

## Execution status

The test is ignored unless `RUSTOK_GROUPS_TEST_POSTGRES_URL` is configured and was not executed while preparing this slice. FBA `membership_enforcement_migration` and `membership_enforcement_revision_runtime` remain null until maintainer execution.

Maintainer command:

```bash
RUSTOK_GROUPS_TEST_POSTGRES_URL='postgres://...' \
  cargo test -p rustok-server --features mod-groups \
  --test groups_membership_enforcement_migration_revision_postgres -- --ignored --nocapture
```

Source guard:

```bash
node scripts/verify/verify-groups-membership-enforcement-migration-revision-postgres.mjs
```

No Cargo command, test, Node verifier, formatter, migration execution, workflow, browser/schema execution, or CI job was run while adding this source.
