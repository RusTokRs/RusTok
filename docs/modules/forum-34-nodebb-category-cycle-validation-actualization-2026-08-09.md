# FORUM-34C NodeBB category cycle validation actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor

FORUM-34A introduced the bounded runner-neutral NodeBB source mapper. FORUM-34B added side-effect-free dependency inspection for missing batch records, mismatched `mainPid` ownership and external author identity resolution.

Fresh `main` before this slice was `5070b9002287311d9bc7150b3b78123c55fa9548`. The only change after FORUM-34B was Blog/Taxonomy work; no Forum source overlapped. Fresh repository search still finds no neutral shared import runner/framework, `rustok-import` owner crate or generic import job/checkpoint/receipt contract suitable for Forum composition.

## Gap closed

FORUM-34B treated an in-batch category parent reference as resolved whenever the referenced category ID existed in the same bounded batch.

That left one unsafe graph shape: self-parent and multi-node category cycles could make `NodebbForumImportInspection::is_dependency_complete()` return true even though a future importer could not construct a valid category tree from that batch.

FORUM-34C closes only that source-local graph gap.

## Public behavior

`ForumImportDependencyDisposition` now includes the fixed additive value:

```text
cyclic_batch_relation
```

`NodebbForumImportInspector::inspect_batch` computes category-cycle membership using only positive parent references whose target category is present in the current bounded batch.

For every category that is itself a member of such a cycle, the existing `category_parent` dependency is emitted with `cyclic_batch_relation` in the original category source order.

This includes:

- direct self-parent (`cid == parentCid`);
- two-node cycles;
- longer in-batch parent cycles.

A category that merely points into a cyclic component is not itself labeled cyclic. The actual cycle members already make the inspection incomplete.

## Cross-page fail-closed rule

A positive `parentCid` whose category record is absent from the current batch remains:

```text
missing_batch_record
```

FORUM-34C does not infer that such a reference is cyclic or invalid globally. A future shared runner may resolve it from an earlier/later cursor page or from runner-owned durable source-reference state.

Therefore cycle evidence is intentionally limited to relationships fully proven by the current bounded source batch.

## Bounds and determinism

The source batch remains capped at `MAX_FORUM_IMPORT_SOURCE_RECORDS_PER_BATCH = 512`.

Cycle detection builds bounded `BTreeMap` / `BTreeSet` indexes from the current category records only. It performs no recursive database lookup and never loads another export page.

The existing dependency issue bound remains valid:

```text
MAX_FORUM_IMPORT_DEPENDENCY_ISSUES_PER_BATCH = 1536
```

Each source record still emits at most three dependency issues, and each category emits at most one parent dependency issue whether it is missing or cyclic.

Diagnostics remain deterministic in original category/topic/post source order.

## Ownership boundary

This slice adds no:

- database read or write;
- migration;
- UUID synthesis;
- Profiles/auth lookup;
- Media access;
- Forum-only runner;
- checkpoint, receipt, replay or audit persistence;
- cross-batch lookup;
- scheduler;
- CLI/admin transport;
- candidate persistence;
- search rebuild execution.

The category-cycle check is a pure source-batch validation concern owned by the Forum NodeBB adapter. Runner orchestration and cross-page state remain outside Forum until a neutral shared contract exists.

## Evidence added in source

Source tests cover:

- direct self-parent;
- a two-node category cycle;
- a missing external parent remaining `missing_batch_record` rather than cyclic;
- an acyclic in-batch category chain remaining dependency-complete.

These tests are source additions only in this slice and were not executed.

## Remaining FORUM-34 scope

Still open after FORUM-34C:

- neutral shared runner contract and host composition;
- durable cursor/checkpoint/receipt/replay/audit semantics;
- external-user resolution through the proper owner boundary;
- cross-batch dependency resolution;
- candidate-to-existing Forum owner command adapter;
- attachment/media mapping after FORUM-14 provides Forum-owned relations;
- Forum export adapter over stable bounded owner reads;
- runner-level dry-run report composition;
- reconciliation/search rebuild orchestration;
- CLI/admin transport only after RBAC/idempotency/audit admission exists;
- retained SQLite/PostgreSQL/restart/lost-response evidence.

The large canonical implementation plan is not replaced wholesale through the GitHub contents API; this dated packet records the FORUM-34C cursor without overwriting unrelated concurrent roadmap edits.

## Maintainer validation

Per maintainer instruction, no test, Cargo command, Node verifier, formatter, migration, DB scenario, CLI, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-nodebb-category-cycle-validation-source.mjs
```
