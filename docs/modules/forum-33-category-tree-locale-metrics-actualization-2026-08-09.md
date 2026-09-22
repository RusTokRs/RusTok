# FORUM-33J category-tree locale metrics actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / bounded-locale-coverage`

## Rechecked FORUM-33 cursor

The executed source sequence is now FORUM-33A through FORUM-33I. The latest two telemetry slices are:

- FORUM-33H: fixed-cardinality locale-resolution observations on successful `forumUnreadTopics` reads;
- FORUM-33I: fixed-cardinality unread/activity observations on the same bounded owner-backed page.

Fresh `main` still has no persisted Forum-owned attachment relation/source-revision authority. Attachments therefore remain blocked on FORUM-14; this slice does not infer relation truth from rich text or Media-private storage.

## Why category tree is the next truthful locale surface

The canonical FORUM-33 telemetry scope includes locale fallback. `forumCategoryTree` is already a mounted bounded GraphQL read and already resolves localized owner DTOs through the category audience read service.

The authoritative call remains:

```text
ForumCategoryAudienceReadService::tree_authenticated_owner_visible_with_audience_context
```

Its `CategoryTreeResponse` contains bounded recursive `CategoryTreeNode` values with the exact locale facts needed for observation:

- `requested_locale`;
- `effective_locale`;
- `available_locales`.

FORUM-33J observes only those already-materialized owner nodes after the owner read succeeds. It performs no additional database query, owner call, fallback resolution, permission check, visibility decision, mutation or repair.

## Metric contract

The new metric is:

```text
rustok_forum_graphql_category_tree_locale_resolution_total{outcome="..."}
```

`outcome` has exactly three values:

- `exact`: at least one localized row exists and effective locale equals requested locale;
- `fallback`: at least one localized row exists and effective locale differs from requested locale;
- `missing`: the owner node reports no available locale.

The tree is already bounded by `MAX_FORUM_CATEGORY_TREE_NODES`; recursive observation walks only returned nodes and adds no separate population scan.

## Why this is a separate surface-scoped family

The merged FORUM-33H collector remains unchanged. FORUM-33J does not refactor or widen that established `forumUnreadTopics` source contract merely to share a private collector implementation.

A category-tree-specific family keeps the new surface explicit and preserves the existing H/I source guards without changing their behavior. It is not duplicate sampling of the same owner result: H observes unread-topic rows, while J observes category-tree nodes.

## Cardinality and privacy boundary

The only label is bounded `outcome`. The metric does not export locale values themselves and contains no:

- tenant ID;
- user ID;
- category ID or parent ID;
- category name or slug;
- route, icon, color or description;
- topic/reply counts;
- arbitrary error or permission values.

Tenant-controlled locale strings therefore cannot create Prometheus series.

## Runtime and failure semantics

Collector registration uses `rustok_telemetry::register_runtime_collector` and is best effort. If the runtime registry is not ready, the GraphQL owner result and response remain unchanged and a later successful observation can retry registration.

The existing category-tree read-path query and budget metrics remain unchanged. FORUM-33J adds no migration, table, queue, worker, job, receipt, checkpoint or write path.

## Remaining ownership blocks

Spam-outcome telemetry remains blocked on real posting-policy owner enforcement/contracts; FORUM-26D is still a pure evaluator and `DuplicateContent` / `ExternalSpamScore` are reserved reasons rather than mounted operational outcomes.

Moderation recovery now has public owner commands and host GraphQL orchestration, but that is a write/recovery boundary, not a Forum-owned application-operation diagnostic read. Forum still must not inspect Moderation private operation storage. A later Moderation diagnostic requires an explicit owner-supported read/status contract.

Search and Notifications already own substantial convergence/reconciliation telemetry and should not receive duplicate Forum metrics without a new gap.

## Canonical-plan drift

The large canonical Forum implementation plan still lags parts of the merged FORUM-33 execution ledger. This dated packet records the actual cursor without whole-file replacement through the GitHub contents API, which could overwrite unrelated concurrent roadmap edits.

## Next FORUM-33 cursor

Attachments remain blocked on FORUM-14. Spam outcomes remain blocked on real owner enforcement. Moderation application status remains blocked on a public read/status owner contract.

After this category-tree locale extension, recheck another mounted localized Forum surface only if it adds useful coverage without turning telemetry into blanket instrumentation. Topic/reply storefront reads are possible candidates. Otherwise stop the locale expansion and wait for one of the blocked owner contracts to land.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source check, intentionally not run here:

```bash
node scripts/verify/verify-forum-category-tree-locale-metrics-source.mjs
```
