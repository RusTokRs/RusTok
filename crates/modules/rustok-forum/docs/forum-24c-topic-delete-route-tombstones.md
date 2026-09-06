# FORUM-24C topic delete route tombstones

## Status

`source_ready_maintainer_execution_pending`

FORUM-24C composes localized route tombstones into the existing topic delete owner. It adds no new command or transport field. Route history is recorded before the topic is soft-deleted and commits with the existing lifecycle transaction.

Machine contract:

```text
crates/rustok-forum/contracts/forum-topic-delete-route-tombstone-owner.json
```

## Owner composition

`TopicService::delete` calls `ForumTopicRouteService::record_delete_tombstones_in_tx` after the delete claim and before localized cleanup. The route owner reads each topic translation with a non-empty slug and records one immutable `gone` row for its tenant, locale, short identity and normalized slug.

The public delete API is unchanged and uses the stable reason `Topic deleted`. Tombstones have no target topic or target locale. A topic without route-bearing translations keeps the previous behavior.

## Existing history

Append-only history is preserved:

- an existing redirect for the same topic and route remains a redirect;
- an exact existing `gone` row is idempotent;
- ownership, target-field or reason drift fails closed.

This means deleting an archived merge source does not replace its FORUM-24B redirect.

## Resolution

After deletion, current-route lookup excludes the topic through `deleted_at`. Its old route resolves from the ledger as `gone`, with no canonical descriptor and with the original topic identity retained.

Route resolution does not grant read permission. A host must still apply its visibility and deleted-content policy.

## Compatibility and remaining work

This slice does not change GraphQL, REST schemas, storefront routes, merge receipts or domain events. Topic slug mutation and historical backfill are not included.

Remaining work includes topic rename aliases, storefront mounting, category routes, hreflang and SEO policy, plus retained runtime evidence.

## Maintainer verification

```bash
node scripts/verify/verify-forum-topic-delete-route-tombstone-owner.mjs
cargo test -p rustok-forum --test topic_delete_route_tombstone_sqlite -- --nocapture
cargo test -p rustok-forum --test topic_merge_route_alias_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
