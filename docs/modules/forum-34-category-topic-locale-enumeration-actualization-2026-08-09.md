# FORUM-34G bounded category/topic locale enumeration actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / shared-runner-blocked`

## Cursor and recheck

FORUM-34A through FORUM-34F are merged before this slice. 34E established manage-only exact stored-locale enumeration for replies; 34F added the bounded operator owner-reader that consumes explicit localized targets and delegates field mapping to the 34D export mapper.

Fresh `main` for this slice is `22600b287fe01c2e30e5c5ce82137010032897c7`. The commits after 34F are Order and Page Builder only and do not overlap Forum import/export source.

A recheck of `rustok-core` RBAC confirms that an effective `Manage` permission satisfies narrower actions for the same resource. Therefore the 34F owner reader's `*:manage` admission remains compatible with the downstream owner `Read` checks; no hidden `manage + read` contract correction is required.

The canonical Forum implementation-plan ledger still labels `FORUM-34` as `planned` even though 34A-34F are merged. Safe synchronization of that large concurrently edited source remains open; this packet records the truthful FORUM-34G cursor without replacing a partial/truncated roadmap image.

## Gap selected

The next export planner needs exact locale identities for categories, topics and replies before constructing 34F localized targets. Replies already have a bounded batch API from 34E. Categories and topics exposed `available_locales` only as a side effect of full localized owner reads, which would force the planner to hydrate one full owner response per source ID and create avoidable N+1 work.

34G closes that parity gap with dedicated bounded owner locale-enumeration APIs for categories and topics.

## Public owner contracts

`CategoryService` now exposes:

- `MAX_FORUM_CATEGORY_LOCALE_ENUMERATION_IDS = 512`;
- `available_locales_for_categories(tenant_id, security, category_ids)`.

`TopicService` now exposes:

- `MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS = 512`;
- `available_locales_for_topics(tenant_id, security, topic_ids)`.

Both methods return `Vec<(Uuid, Vec<String>)>` in caller-supplied ID order. They are in-process owner APIs; this slice adds no GraphQL, REST, CLI, file-format or serde admission surface.

## Admission and validation

Both public facades:

- reject `SecurityContext::public_read()`;
- require the exact resource `Action::Manage` scope;
- delegate with the same trusted tenant/security context.

The raw owner storage methods repeat the `Manage` check and then fail closed on:

- nil tenant IDs;
- more than 512 IDs;
- nil IDs;
- duplicate IDs;
- IDs not present in the requested tenant;
- owner rows that have no stored localized rows.

An empty ID batch returns an empty result without a storage query.

## Exact stored-locale semantics

Locale enumeration does not call `resolve_by_locale_with_fallback`, full `get`, list hydration, vote/subscription reads, taxonomy, custom-field resolution, visibility composition or canonical topic resolution.

For each resource kind the storage path performs:

1. one tenant-scoped bounded existence query for the complete requested ID set;
2. one existing batched translation loader over the same tenant and IDs;
3. `available_locales_from` over only the stored category/topic translation rows.

The result therefore describes stored locale identities, not requested/effective fallback locales and not presentation eligibility.

The locale API intentionally does not claim that an archived/deleted/otherwise non-presentable owner row is export-readable. A later 34F exact owner read still applies the public owner visibility/lifecycle boundary before producing an export fragment. 34G supplies planning facts only; it does not bypass owner eligibility.

## N+1 boundary

34G adds no per-ID owner `get`, `find_topic`, `find_category`, translation query or viewer-state hydration loop. Category and topic locale identities are each resolved with a bounded existence query plus the existing batched translation query.

Together with 34E, all three current Forum export shapes now have bounded exact stored-locale discovery suitable for a later planner without introducing one full owner read per source ID merely to discover languages.

## Ownership and non-goals

34G adds no:

- generic or Forum-only durable migration runner;
- tenant-wide enumeration or checkpoint cursor;
- export eligibility decision;
- full owner response hydration;
- persistence write;
- import adapter;
- durable receipt/replay/audit state;
- external-user resolution;
- revision/history, vote/reputation or attachment transfer;
- Search rebuild orchestration;
- transport or schema migration.

## Next FORUM-34 cursor

The next safe Forum-owned slice can compose the category/topic/reply locale-enumeration APIs into a bounded non-wire target planner that emits 34F `ForumExportReadBatch` values and rejects expansion beyond the 512 localized-target fragment bound. Tenant-wide enumeration, durable chunk cursors and replay/checkpoint ownership remain blocked on an admitted shared runner contract and explicit lifecycle/history scope.

## Maintainer validation

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, migration, database scenario, workflow, CI command, lock generation or `git diff --check` was run while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-category-topic-locale-enumeration-source.mjs
```
