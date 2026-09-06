# Forum moderation revision migration contract

Status: **source-ready / maintainer execution pending**

## Scope

`crates/rustok-forum/tests/moderation_revision_migration_contract.rs` retains SQLite/PostgreSQL evidence for Forum-owned moderation subject revision clocks introduced by `m20260807_000027_add_forum_moderation_subject_revisions`.

These rows are current-state fencing clocks only. They do not replace Forum lifecycle state and do not copy Moderation cases, decisions, application operations or audit history into Forum.

## Upgrade / backfill

The upgrade scenarios install the ordinary Forum prerequisites, take the production `ForumModule::migrations()` list, hold back its current final migration, apply every earlier Forum migration, and seed one existing topic plus one existing reply. The held production migration object is then applied without duplicating its SQL in the test.

Both backends must backfill exactly one revision row for each pre-existing subject, starting at revision `1`:

- `forum_topic_moderation_subject_revisions(tenant_id, topic_id, revision)`;
- `forum_reply_moderation_subject_revisions(tenant_id, reply_id, revision)`.

This retains the upgrade guarantee that subjects which existed before moderation clocks became active receive a stable initial reviewed revision instead of becoming unmoderatable or being assigned a timestamp-derived identity.

## Clean install / new subjects

Separate clean-install paths apply the full production Forum migration list before inserting subjects. The insert triggers must initialize every new topic and reply at revision `1` on both SQLite and PostgreSQL.

The upgrade scenario also creates a second topic/reply after the migration and requires the same initialization behavior, so backfill and future-row initialization are proven independently.

## Trigger parity

After upgrade, the same assertion sequence is used on both backends.

For topics, the clock must advance for:

- owner metadata changes;
- permanent lock state changes;
- topic translation insert;
- moderation-relevant topic translation update;
- topic translation delete.

A no-op write to `reply_count`, which is intentionally outside the moderation subject identity fields, must not advance the topic moderation revision.

For replies, the clock must advance for:

- lifecycle/enforcement status change (`approved -> hidden`);
- reply body insert;
- moderation-relevant reply body update;
- reply body delete.

A no-op write to `updated_at`, which is not itself reviewed subject content/state, must not advance the reply moderation revision.

This protects the central fencing property: content or lifecycle facts that can change the reviewed subject advance the dedicated revision, while unrelated bookkeeping does not create false stale-review conflicts.

## Prerequisites and isolation

The harness mirrors the existing Forum PostgreSQL regression bootstrap: a minimal platform `users` table, production Outbox and Taxonomy migrations, the shared Flex field-definition cache-generation prerequisite, then Forum migrations.

SQLite uses a fresh in-memory database with foreign keys enabled for each scenario. PostgreSQL reads `RUSTOK_FORUM_TEST_DATABASE_URL`, falling back to a PostgreSQL `DATABASE_URL`, and creates separate isolated `rustok_forum_moderation_revision_<scenario>_<uuid>` schemas which are dropped with `CASCADE`.

## Boundary with concurrency evidence

This contract proves schema/backfill/trigger parity only. It deliberately does not claim that a concurrent content edit and moderation application have been exercised. The separate concurrency slice must drive the real Forum moderation adapter against independent PostgreSQL connections and prove that the revision fence produces either one exact application or a stale-review conflict, never silent retargeting.

## Maintainer commands

Intentionally not run while preparing this slice:

```bash
cargo test -p rustok-forum --test moderation_revision_migration_contract -- --nocapture

RUSTOK_FORUM_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-forum --test moderation_revision_migration_contract -- --nocapture

node scripts/verify/verify-forum-moderation-revision-migration-contract.mjs
```

No tests, Cargo commands, Node verifiers, formatters, real PostgreSQL migrations, workflows or CI were executed while preparing this file.
