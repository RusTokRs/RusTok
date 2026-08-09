# FORUM-34B NodeBB dependency inspection actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor

FORUM-34A introduced the bounded runner-neutral NodeBB category/topic/post mapper and external source-reference contract.

Fresh `main` before this slice was `84911afda2c54321a6e30da616b4bb3eea6edb74`. A fresh repository search still finds no neutral shared import runner/framework, `rustok-import` owner crate, generic `ImportRunner`, `ImportAdapter`, `ImportJob`, checkpoint or receipt API suitable for Forum composition.

The historical FORUM-34 scope requires imports to be dry-runnable, resumable, cursor-based, idempotent and bounded. FORUM-34B advances only the dry-run mapping boundary that Forum can truthfully own before shared orchestration exists.

## Public inspection contract

`rustok-forum::import_inspection` adds a side-effect-free `NodebbForumImportInspector` over the existing `NodebbForumImportMapper`.

`inspect_batch` first runs the FORUM-34A structural mapper. Invalid or oversized source batches therefore retain the same mapping errors and never receive dependency diagnostics that could mask a structural failure.

For a structurally valid batch it returns:

- the exact mapped `ForumImportCandidateBatch`;
- a deterministic `unresolved_dependencies` list in category, topic and post source order.

The inspector does not change candidate data and does not resolve anything by querying the database or another owner.

## Dependency relations

The fixed relation vocabulary is:

- `category_parent` — a positive category parent reference;
- `topic_category` — the category required by a topic;
- `topic_main_post` — the positive NodeBB `mainPid` selected as the topic body;
- `post_topic` — the topic required by a post;
- `author_user` — an external NodeBB author identity requiring shared identity/Profile-owner composition.

The fixed dispositions are:

- `missing_batch_record` — the referenced category/topic/post is not present in this bounded source batch;
- `mismatched_batch_record` — currently used when a referenced `mainPid` exists in the batch but belongs to another topic;
- `external_owner_resolution` — a positive NodeBB user reference that Forum must not convert into a RusTok user/profile identity itself.

No arbitrary error string, DB result or provider state is encoded into the relation/disposition vocabulary.

## Fail-closed semantics

FORUM-34B deliberately does not treat a missing reference as proof that the source object does not exist globally. The shared runner may later satisfy a `missing_batch_record` from an earlier/later cursor page or durable source-reference map.

Likewise, positive NodeBB `uid` values remain unresolved external references. Forum neither invents UUIDs nor reads Profiles/auth persistence to match them.

A `topic_main_post` mismatch is explicit rather than silently accepting a post from another topic as the body source.

`NodebbForumImportInspection::is_dependency_complete()` is true only when this inspection found no batch dependency or external-owner dependency. It is not a persistence-admission decision: final Forum owner validation, identity resolution, idempotency, RBAC and runner receipts remain separate gates.

## Bounds

The FORUM-34A source batch remains capped at `MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH = 512` records.

Each source record can emit at most three dependency issues, so FORUM-34B exposes the deterministic upper bound:

```text
MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH = 1536
```

The inspector uses only bounded `BTreeSet` / `BTreeMap` indexes over the current batch. It never loads another page or complete export into memory.

## Ownership boundary

The inspection module imports no SeaORM/database type, `Uuid`, Media lifecycle API, Profiles persistence, Notifications, Search or Moderation storage. It has no async call, transaction, create/update/delete operation, runtime extension, scheduler or transport.

FORUM-34B does not add:

- a Forum-only import runner;
- checkpoint, receipt, replay or audit persistence;
- a migration;
- CLI/admin transport;
- cross-batch lookup;
- identity/profile matching;
- candidate persistence;
- search rebuild execution;
- attachment/media import mapping.

Those remain blocked on their proper shared runner or owner contracts.

## Remaining FORUM-34 scope

Still open after FORUM-34B:

- neutral shared runner contract and host composition;
- durable cursor/checkpoint/receipt/replay/audit semantics;
- explicit external-user resolution through the proper shared owner boundary;
- cross-batch dependency resolution using runner-owned state;
- candidate-to-existing Forum owner command adapter;
- attachment/media mapping after FORUM-14 provides Forum-owned relations;
- Forum export adapter over stable bounded owner reads;
- dry-run report composition at runner/job level;
- reconciliation/search rebuild orchestration;
- CLI/admin transport only after RBAC/idempotency/audit admission exists;
- retained SQLite/PostgreSQL/restart/lost-response evidence.

The large canonical implementation plan is not replaced wholesale through the GitHub contents API; this dated packet records the execution cursor without overwriting unrelated concurrent roadmap edits.

## Maintainer validation

Per maintainer instruction, no test, Cargo command, Node verifier, formatter, migration, DB scenario, CLI, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-nodebb-import-dependency-inspection-source.mjs
```
