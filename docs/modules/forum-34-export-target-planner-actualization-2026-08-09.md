# FORUM-34H bounded export target planner actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor and recheck

FORUM-34A through FORUM-34G are merged before this slice. 34E added bounded exact stored-locale enumeration for replies, 34F added the bounded owner export reader over explicit localized targets, and 34G added matching bounded exact stored-locale enumeration for categories and topics.

Fresh `main` for this slice is `a0d8054e02296fa43a81d9dd53d4b616c231d0d7`. The commits after 34G are Commerce and Page Builder work and do not overlap Forum import/export source.

The canonical Forum implementation-plan ledger still labels `FORUM-34` as `planned` even though 34A-34G are merged. Safe synchronization of that large concurrently edited source remains open; this dated packet records the truthful 34H cursor without replacing a partial/truncated roadmap image.

## Gap selected

34F accepts an explicit `ForumExportReadBatch`, while 34E/34G expose exact stored locales for known reply/category/topic IDs. Before 34H, callers still had to duplicate the expansion logic that turns source IDs into one exact localized read target per stored locale and had no shared boundary tying discovery size to the 34F fragment limit.

34H adds a Forum-owned, non-wire planner that composes those existing locale-enumeration APIs into the exact input shape consumed by 34F.

## Public in-process contract

`ForumExportTargetPlanRequest` carries one tenant ID plus category, topic and reply ID vectors. `ForumExportTargetPlanner::plan_fragment(...)` accepts the three existing owner facades, `SecurityContext`, and the request, and returns `ForumExportReadBatch`.

The request and resulting read batch remain in-process composition types. 34H does not derive `Serialize` or `Deserialize`, add a file format, or introduce GraphQL/REST/CLI transport.

## Admission and bounds

Planning fails closed before locale discovery when the context is public-read, the tenant ID is nil, no source IDs are supplied, the combined category + topic + reply source-ID count exceeds 512, any source ID is nil, an ID repeats within its resource kind, or any requested resource kind lacks its exact `*:manage` scope.

The manage checks are preflighted for every requested kind before the first owner locale query, so a mixed-scope request does not perform partial discovery work before failing authorization.

The combined source-ID bound is intentionally the same 512 ceiling as `MAX_FORUM_EXPORT_READ_TARGETS_PER_FRAGMENT`. Since each valid source has at least one stored locale, a source batch above 512 could never produce a valid 34F fragment and is rejected before any discovery call.

## Exact locale expansion

For each non-empty resource kind, in deterministic order `category -> topic -> reply`, the planner calls exactly one existing owner locale-enumeration API: `CategoryService::available_locales_for_categories`, `TopicService::available_locales_for_topics`, or `ReplyService::available_locales_for_replies`.

The planner preserves caller source-ID order inside each kind and preserves the owner locale-fact order while normalizing every locale through `normalize_locale_code`.

It defensively rejects locale-fact contract drift when returned fact count differs from requested ID count, a returned ID differs from the corresponding requested ID, a source returns no locales, a locale is invalid, or two returned strings collapse to the same normalized locale.

Each normalized locale becomes one `ForumExportReadTarget { kind, id, locale }`.

## Localized target ceiling

Expansion is bounded independently from the source-ID ceiling. If multilingual expansion would produce more than 512 localized targets, planning fails with `TooManyTargets` as soon as target 513 would be appended.

This means a request with a small number of highly multilingual resources cannot silently create a read batch that 34F would reject later. Once the 512 ceiling is reached, later resource kinds are not queried.

## Ownership and side effects

`export_planner.rs` contains no SeaORM/database/entity access. It does not call category/topic/reply full owner `get`, does not call `ForumOwnerExportReader::read_fragment`, and does not map export records itself.

Its only owner reads are the three established exact stored-locale enumeration APIs from 34E/34G. Export eligibility remains a 34F concern: archived, hidden, deleted, merged/canonicalized, or otherwise non-readable owner content may still fail when the planned target is passed to the exact owner reader. 34H does not weaken that boundary.

34H performs no writes and adds no checkpoint, receipt, replay, audit, durable job, or migration state.

## Determinism

For the same tenant, source-ID vectors, security authority, and stored locale facts, target ordering is deterministic: all category targets in category input order, then topics, then replies; locales for each source retain the order returned by the exact owner locale API. No sorting by presentation fields, viewer state, timestamps, votes, or fallback locale is introduced.

## Non-goals

34H adds no tenant-wide category/topic/reply enumeration, durable migration/export runner, checkpoint or continuation cursor, import persistence adapter, external user/identity resolution, history/revision, votes/reputation, attachment or route-alias transfer, Search rebuild orchestration, export file transport, GraphQL/REST/CLI surface, or schema migration.

## Next FORUM-34 cursor

With 34H, callers that already possess bounded owner IDs can now compose exact locale discovery -> bounded localized target planning -> 34F exact owner reads -> 34D export mapping without duplicating storage access or fallback semantics.

The next safe slice should inspect how bounded source IDs can be enumerated with explicit lifecycle/history policy. A tenant-wide cursor/checkpoint/receipt contract should not be invented inside Forum while the neutral shared migration runner remains absent.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-export-target-planner-source.mjs
```
