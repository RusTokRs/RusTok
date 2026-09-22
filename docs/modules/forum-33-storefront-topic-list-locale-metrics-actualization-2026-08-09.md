# FORUM-33K storefront topic-list locale metrics actualization — 2026-08-09

Status: `source-ready / maintainer-execution-open / locale-baseline-complete`

## Rechecked execution cursor

The merged FORUM-33 source sequence is A through J. FORUM-33H observes locale fallback on bounded `forumUnreadTopics` rows; FORUM-33I observes bounded unread/activity states; FORUM-33J observes locale fallback on bounded `forumCategoryTree` nodes.

Fresh `main` before this slice was `f9b50401f836d97092b21b9f4e4d0b2437ab3fe1`. Changes after FORUM-33J were outside Forum.

Fresh source search still finds no persisted Forum-owned `forum_attachment*`, `ForumAttachment`, or `attachment_id` relation/source-revision authority. Attachments therefore remain blocked on FORUM-14.

Fresh posting-policy search still finds `ForumPostingPolicyEvaluator::decide`, `DuplicateContent`, and `ExternalSpamScore` only in documentation, tests, and source verification rather than mounted production enforcement. Spam-outcome telemetry therefore remains blocked on real owner execution/contracts.

Moderation recovery has public write/recovery commands, but Forum still has no public Moderation application-operation read/status contract suitable for lag/status diagnostics. Forum must not inspect Moderation-private application storage.

## Why this locale surface is useful

`forumStorefrontAudienceTopics` is the public paginated storefront topic-list surface. It is materially different from the authenticated unread page and category tree already covered by H/J: it represents the normal public topic discovery path and can therefore reveal locale fallback behavior that those surfaces do not exercise.

The mounted owner call remains:

```text
ForumTopicAudienceListService::list_public_storefront_visible_with_locale_fallback
```

The returned `TopicListItem` values already contain:

- `requested_locale`;
- `effective_locale`;
- `available_locales`.

FORUM-33K observes those already-materialized owner DTOs only after the successful owner read. It adds no database query, owner call, fallback pass, authorization decision, channel check, visibility decision, write, repair, cursor, or response-field change.

## Metric contract

The metric is:

```text
rustok_forum_graphql_storefront_topic_list_locale_resolution_total{outcome="..."}
```

`outcome` has exactly three values:

- `exact`: at least one localized row exists and effective locale equals requested locale;
- `fallback`: at least one localized row exists and effective locale differs from requested locale;
- `missing`: the owner DTO reports no available locale.

Only the normalized page returned by the existing GraphQL pagination path is observed. The metric is an observation counter, not a tenant-wide population gauge or unique-topic estimate; repeated reads can count the same topic again.

## Cardinality and privacy boundary

The only label is fixed `outcome`. No tenant-controlled locale string becomes a label. The observer also exports no tenant ID, user ID, topic/category/author ID, title, slug, route, channel, tag, status, reply/vote count, subscription state, solution ID, metadata, or arbitrary error.

Collector registration is best effort through `rustok_telemetry::register_runtime_collector`; registration failure does not alter the GraphQL result and can be retried on a later successful observation.

## Existing behavior preserved

The existing public module/channel guards, tenant scoping, pagination normalization, owner call, read-path query metric, read-path budget metric, DTO mapping, page total, offset and limit remain authoritative and unchanged.

FORUM-33K does not change the merged H, I, or J collectors. It uses a surface-specific family so each source contract remains explicit and existing guards do not need to be widened or rewritten.

## Locale baseline stopping point

With K, FORUM-33 has representative locale-fallback observations on three materially different bounded mounted GraphQL reads:

1. authenticated unread-topic page;
2. category tree;
3. public storefront topic list.

That is sufficient for the baseline requested by FORUM-33. Do not continue adding locale metrics to every single-item/topic/reply/admin/HTTP read merely for coverage. A future locale metric should require a distinct operational question or evidence of a blind spot, not just another localized DTO.

## Remaining FORUM-33 blocks

- attachment reconciliation: blocked on FORUM-14 persisted Forum-owned relation/source-revision authority;
- spam outcomes: blocked on mounted owner enforcement and real duplicate/external-spam inputs;
- Moderation application status/lag: blocked on an explicit public Moderation read/status contract;
- Search/Notifications: already have owner-side reconciliation/diagnostic coverage; avoid duplicate Forum telemetry without a new gap.

The canonical implementation plan still lags portions of the merged FORUM-33 execution ledger. This dated actualization records the live cursor without replacing the large canonical file through the GitHub contents API and risking unrelated concurrent roadmap edits.

## Next cursor

After FORUM-33K, stop locale expansion. On the next continuation, recheck whether FORUM-14, mounted posting-policy enforcement, or a public Moderation diagnostic owner contract has landed. If none has landed, reassess whether FORUM-33 has any truthful remaining source slice rather than manufacturing telemetry from missing authority.

## Maintainer validation

Per maintainer instruction, no Cargo command, test, Node verifier, formatter, GraphQL request, database fixture, migration, build, workflow, CI, lock generation or `git diff --check` was executed while preparing this slice.

Suggested source guard, intentionally not run here:

```bash
node scripts/verify/verify-forum-storefront-topic-list-locale-metrics-source.mjs
```
