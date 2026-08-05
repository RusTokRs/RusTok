# FORUM-24E historical topic-merge route backfill

FORUM-24E adds a bounded owner repair for topic merges committed before
FORUM-24B began writing localized route aliases in the merge transaction.

## Owner boundary

`ForumTopicMergeRouteBackfillService::backfill_merge_route_aliases` requires
`forum_topics:manage`. It reads only immutable `forum_topic_merge_operations`
rows for the selected tenant and writes aliases only through
`ForumTopicRouteService::record_merge_redirect_aliases_in_tx`.

The repair does not reconstruct merge policy, inspect semantic-event payloads,
or mutate merge receipts. New merges already compose aliases atomically and do
not depend on this repair for correctness.

## Bounded cursor

The input contains an optional `(merged_at, operation_id)` cursor and a required
limit from 1 through 100. Receipts are read in ascending cursor order through
`idx_forum_topic_merge_operations_route_backfill`.

One page and all aliases ensured by that page commit in one transaction. When
another page remains, the result returns the final processed receipt as
`next_cursor`; otherwise `exhausted` is true and `next_cursor` is absent.

## Alias policy

Every non-empty localized source slug is handled by the existing route owner.
Target locale precedence remains unchanged:

1. exact source locale;
2. platform fallback locale;
3. lexicographically first target locale.

A routed source whose retained target has no localized route fails closed.
Existing aliases are accepted only when source ownership, disposition, target,
locale and reason exactly match the merge receipt. Replaying the same page
therefore verifies the same routes without inserting duplicates.

`ensured_alias_count` counts routes that were inserted or already existed with
that exact payload. It is deliberately not an insertion-only metric, so exact
page replay returns the same result.

## Compatibility and exclusions

FORUM-24E amends the unreleased FORUM-24A route migration with one tenant/cursor
index. It adds no new table, event, REST route, GraphQL field, admin UI or
storefront mount.

Delete-history recovery is not inferred after localized translation rows have
already been removed. Category routes, transport/UI composition, storefront
mounting, hreflang/SEO publication policy and retained runtime evidence remain
follow-up FORUM-24 scope.

## Verification

```bash
node scripts/verify/verify-forum-topic-merge-route-backfill-owner.mjs
cargo test -p rustok-forum --test topic_merge_route_backfill_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_route_alias_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

These commands were not run by the implementation agent, per maintainer
request. Source publication does not claim runtime verification.
