# FORUM-24B topic merge route aliases

## Status

`source_ready_maintainer_execution_pending`

FORUM-24B composes the FORUM-24A immutable topic route ledger into the existing topic merge owner. A successful new merge now records every localized source route with a non-empty slug as a redirect in the same database transaction as the reply movement, source archival, immutable merge receipt and domain event.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-merge-route-alias-owner.json
```

## Owner composition

`ForumTopicMergeService::merge_topic_internal` delegates route history to `ForumTopicRouteService::record_merge_redirect_aliases_in_tx`. The merge service does not reproduce locale normalization, short-identity derivation, route-key idempotency or append-only conflict checks.

Alias persistence happens before transaction commit. Any route conflict or persistence failure rolls back the merge together with its receipt and event. The existing `ForumTopicMergeResult` and `forum.topic.merged` schema remain unchanged.

## Source route selection

The composer reads all source topic translations that have a non-empty slug. Each route retains:

- the source topic short identity;
- the source locale and normalized source slug;
- the original source topic identity;
- the merge reason as bounded alias provenance.

A source topic with no localized route slug does not gain an alias row and does not make the merge fail.

## Target locale selection

Every selected source route redirects to one current target locale. The target locale is chosen deterministically in this order:

1. an exact target translation for the source locale;
2. the platform fallback locale;
3. the lexicographically first available target locale.

The alias stores target topic identity and target locale, not a target slug. Resolution therefore recomputes the target's current slug and follows later target renames or additional canonical merge edges without mutating the historical alias.

When the source owns one or more localized routes, the target must own at least one non-empty route slug. This fails closed before commit rather than accepting a merge whose old public routes cannot resolve anywhere.

## Replay and compatibility

Exact merge-command replay continues to return the immutable merge receipt. The route-key uniqueness constraint and owner idempotency prevent duplicate alias rows. This slice does not backfill aliases for merge receipts created before FORUM-24B.

No GraphQL, REST, admin UI or storefront route is added. Topic rename aliases, deletion tombstones, category routes, host mounting, hreflang and SEO publication policy remain separate work.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-merge-route-alias-owner.mjs
cargo test -p rustok-forum --test topic_merge_route_alias_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
