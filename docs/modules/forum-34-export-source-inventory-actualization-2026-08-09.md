# FORUM-34I bounded export source inventory actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor and recheck

FORUM-34A through FORUM-34H are merged before this slice. 34H accepts bounded owner source IDs and composes exact locale discovery into the localized read targets consumed by 34F. Fresh `main` for this slice is `0fbfaaf717291527de5feba4d4019fde09ff8bad`; intervening commits after 34H are outside Forum import/export source.

The canonical Forum implementation-plan ledger still labels `FORUM-34` as `planned`. This packet records the truthful 34I cursor without replacing a partial/truncated whole-file roadmap image.

## Why not the existing read model

`ForumReadModelService` cursor reads are presentation projections: categories hydrate translations/subscriptions, topics hydrate translations/votes/subscriptions/solutions, and replies hydrate bodies/votes/solution state and are scoped by one topic ID. 34I therefore adds an export-specific owner inventory that selects only IDs plus lifecycle/canonical predicates.

## Public in-process contract

`ForumExportSourceInventoryService::list_page(...)` accepts `SecurityContext`, one non-nil tenant ID, one `ForumExportReadTargetKind`, an optional `after_id`, and an explicit limit in `1..=512`. It returns ordered IDs, a cursor, and `has_more` from one `limit + 1` query.

`ForumExportSourceInventoryPage::target_plan_request()` converts a non-empty page into the one-kind `ForumExportTargetPlanRequest` consumed by 34H and returns `None` for an empty terminal page instead of manufacturing an `EmptySources` request. Request/page types stay non-wire and derive no serde contract.

## Authorization

Public-read contexts fail before storage access. Tenant-wide discovery cannot safely interpret `PermissionScope::Own`, because there is no already-admitted owner ID to constrain the scan. 34I therefore requires `PermissionScope::All` for the requested kind's exact `Action::Manage`: `forum_categories:manage`, `forum_topics:manage`, or `forum_replies:manage`. `Own` and `None` fail closed.

## Bounded live keyset semantics

Each call performs exactly one bounded ID query for one resource kind, ordered by UUID with strict `id > after_id` continuation. At most 513 rows are fetched to return at most 512 IDs plus `has_more`.

The cursor is an in-process source position, not a durable migration checkpoint. A sequence of calls does **not** claim a transactionally frozen tenant snapshot: concurrent inserts, deletes, merges, archives, or restores can change later pages. The absent neutral shared runner must own quiescence/snapshot/checkpoint/receipt semantics before FORUM-34 can claim complete resumable tenant export.

## Lifecycle and canonical policy

Archived categories are excluded through `forum_category_lifecycle`, because `ForumExportCategoryRecord` has no lifecycle/status field and exporting an archived category as ordinary data would silently reactivate it.

Topics require an active category, `deleted_at IS NULL`, and must not be a `source_topic_id` in `forum_topic_merge_operations`. A normal `TopicStatus::Archived` topic remains eligible when it is not soft-deleted or a merge source because `ForumExportTopicRecord.status` preserves that current state. Soft-deleted topics are excluded because the current export schema cannot distinguish them from ordinary archived state. Merge-source topics are excluded because the owner facade canonicalizes them and 34F rejects identity substitution.

Replies require an export-eligible topic plus active category. Reply rows are **not** filtered by their own `deleted_at`/`deleted` status: `ForumExportReplyRecord.status` can preserve a current deleted tombstone, and omitting a deleted parent could leave a live child with a permanently missing `parent_reply_id`. Revision history remains out of scope.

## Referential and storage boundaries

The topic/reply joins prevent a topic with an intentionally absent category or a reply with an intentionally absent topic from entering the pipeline. Locale/body presence is **not** silently filtered: missing localized owner content remains visible to 34E/34G or 34F and fails closed there instead of disappearing from migration coverage.

`export_inventory.rs` reads only IDs and relational/lifecycle predicates. It does not hydrate translations, bodies, custom fields, tags, votes, subscriptions, solutions, audience state, or export records. PostgreSQL and SQLite use equivalent tenant-scoped keyset SQL; unsupported backends fail closed.

## Non-goals and next cursor

34I adds no durable runner, frozen multi-page snapshot, checkpoint/receipt/replay/audit persistence, write/import adapter, revision/history export, vote/reputation or attachment transfer, route-alias/merge-history export, Search rebuild orchestration, GraphQL/REST/CLI transport, or schema migration.

The source-ready current-state export chain is now `34I bounded candidate IDs -> 34H bounded exact-locale target planning -> 34F exact owner reads -> 34D export mapping`.

The next safe slice can compose that chain for one bounded page, while cross-page snapshot/checkpoint ownership remains blocked on the absent neutral shared migration runner. Import persistence and identity resolution remain separate open work.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run.

Suggested source guard, intentionally not run:

```bash
node scripts/verify/verify-forum-export-source-inventory-source.mjs
```
