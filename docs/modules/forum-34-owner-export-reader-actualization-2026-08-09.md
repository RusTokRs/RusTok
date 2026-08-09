# FORUM-34F bounded owner export reader actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor and recheck

FORUM-34A through FORUM-34E are merged on `main` before this slice:

- 34A maps bounded NodeBB category/topic/post source records into runner-neutral external references;
- 34B inspects missing, mismatched and external-owner dependencies without treating inspection as persistence admission;
- 34C rejects source-local category cycles while leaving cross-batch parents unresolved;
- 34D maps already-authorized full Forum owner responses into tenant-scoped `rustok.forum.export.v1` fragments;
- 34E exposes manage-only bounded exact stored-locale enumeration for reply export planning.

Fresh `main` for this rebased slice is `c1243da5ecf0b00589daf67d6304b8d210f8dc73`. The commits after 34E are Commerce, Page Builder, Translation and event-contract maintenance; repository search still finds no generic `ImportRunner`, `ExportRunner`, `ImportAdapter`, `ExportAdapter` or `rustok-import` owner suitable for Forum migration orchestration.

The newer `TranslationExchangeService` remains Translation-owned lifecycle/orchestration for translation exchange artifacts. It does not become the shared Forum category/topic/reply migration runner and does not move Forum import/export ownership into `rustok-translation`.

The canonical Forum ledger still labels `FORUM-34` as `planned`, although 34A-34E are merged and this slice adds another source-ready boundary. That status is stale. The GitHub connector available for this maintenance path only supports whole-file contents replacement for existing files; rewriting the large concurrently edited canonical plan without a complete safe source image would risk deleting unrelated roadmap content. This dated packet records the truthful execution cursor and keeps canonical-plan synchronization explicitly open rather than performing a destructive partial replacement.

## Gap selected

34D deliberately accepts only already-authorized full owner responses. 34E closes reply locale discovery, but a caller still had no bounded Forum-owned composition boundary that could request exact localized owner views and feed them into the 34D mapper.

34F adds that composition boundary without adding a second storage read path.

## Public contract

`rustok-forum::export_mapping` now also publishes:

- `MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT`;
- `ForumExportReadTargetKind` (`Category`, `Topic`, `Reply`);
- `ForumExportReadTarget`;
- `ForumExportReadBatch`;
- `ForumExportReadError`;
- `ForumOwnerExportReader`.

`ForumExportReadBatch` and `ForumExportReadTarget` are in-process composition types only. They deliberately do not derive `Serialize` or `Deserialize`; no new GraphQL, REST, CLI or file-format admission path is created by this slice.

A read batch carries one explicit tenant and at most 512 localized targets. Targets are normalized through the shared locale normalizer and duplicate `(kind, id, normalized locale)` requests are rejected before any owner call. Nil tenant IDs, nil target IDs, invalid locales and empty batches fail closed.

## Operator admission

`ForumOwnerExportReader::read_fragment` rejects `SecurityContext::public_read()` before owner reads.

For every resource kind present in the request it requires the exact corresponding manage authority:

- category targets require `forum_categories:manage`;
- topic targets require `forum_topics:manage`;
- reply targets require `forum_replies:manage`.

Ordinary list/read authority is intentionally insufficient for this export-oriented composition API.

## Owner-only read composition

The reader has no SeaORM/database/entity dependency. It receives existing `CategoryService`, `TopicService` and `ReplyService` owner facades from the caller and uses only their localized owner read methods.

For each bounded target it calls the corresponding owner with the same trusted tenant/security context and with no caller-supplied fallback locale. The shared owner locale resolver can still select platform/first-available fallback content when an exact translation is absent, so 34F does not trust the returned response blindly. After every owner call it verifies:

1. returned owner identity equals the requested UUID;
2. normalized `effective_locale` equals the normalized requested locale.

A mismatch fails closed before mapping. This prevents a fallback read from silently fabricating multilingual export completeness and prevents Topic canonical resolution from silently exporting a different topic identity. URL aliases/merged-topic transfer remain separate future FORUM-34 scope.

After all owner views pass those checks, 34F constructs the existing non-wire `ForumExportOwnerViewBatch` and delegates serialization semantics to `ForumOwnerExportMapper`. The reader therefore does not duplicate field selection, canonical rich-text handling, viewer-state exclusion or export schema ownership from 34D.

## Bounded execution semantics

This is the first executable owner-reader composition boundary, not the final high-throughput tenant exporter.

The reader executes at most 512 localized owner calls per fragment and preserves target order within each exported resource-kind vector. It intentionally does not add direct batched SQL to bypass owner services. A future shared runner may add chunking/checkpoints and Forum may add owner-native bulk response methods if required by retained performance evidence, but those optimizations must preserve the same exact-locale and owner-authorization semantics.

## Ownership and non-goals

34F adds no:

- generic or Forum-only durable migration runner;
- checkpoint/receipt/replay/audit persistence;
- source-to-target identity receipt storage;
- external-user resolution;
- candidate-to-owner import writes;
- revisions/history transfer;
- vote/reputation transfer;
- attachment/media transfer;
- URL alias transfer;
- Search rebuild orchestration;
- GraphQL/REST/CLI/admin transport;
- migration or schema change;
- direct read of another module's persistence.

## Remaining FORUM-34 scope

After 34F, still open:

- synchronize the canonical implementation-plan ledger from stale `planned` to truthful `in_progress` without overwriting concurrent roadmap edits;
- neutral shared import/export runner contract and host composition;
- durable cursor/checkpoint/receipt/replay/audit semantics;
- bounded tenant/category/topic/reply enumeration and runner chunk orchestration around the owner reader;
- retained performance evidence and any owner-native bulk response optimization justified by it;
- cross-batch import dependency resolution;
- external-user resolution through auth/Profile owner contracts;
- candidate-to-existing Forum owner command adapter;
- revisions/history export/import policy;
- votes/reputation export/import through their owners;
- attachment/media mapping after FORUM-14 establishes Forum-owned relations;
- URL alias transfer where route-owner contracts permit it;
- runner-level dry-run/reconciliation/Search rebuild orchestration;
- CLI/admin transport only after RBAC/idempotency/audit admission;
- retained SQLite/PostgreSQL/restart/lost-response evidence.

## Maintainer validation

Per maintainer instruction, no test, Cargo command, Node verifier, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-owner-export-reader-source.mjs
```
