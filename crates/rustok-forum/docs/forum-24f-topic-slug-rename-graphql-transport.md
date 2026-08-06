# FORUM-24F topic slug rename GraphQL transport

FORUM-24F exposes the existing FORUM-24D localized topic slug rename owner
through one additive GraphQL mutation. The adapter contains no route ownership,
alias, locale-selection or merge/delete lifecycle policy.

## GraphQL command

`renameForumTopicSlug` accepts the routed topic ID plus
`RenameForumTopicSlugGraphqlInput`:

- `locale` selects one existing localized topic translation;
- `slug` is the new non-empty localized route segment.

The result is `GqlForumTopicSlugRename`, containing the previous slug and path,
the current canonical route descriptor, the immutable alias ID when a change
was written, and the owner-provided `changed` replay flag.

## Authorization and tenant boundary

The Forum module must be enabled. The adapter requires
`forum_topics:update`, derives the tenant from `TenantContext`, and rejects an
optional mismatched tenant assertion with `PERMISSION_DENIED`.

The adapter then forwards the authenticated permission snapshot to
`TopicService::rename_slug`. Author ownership and manager override semantics
remain exclusively owner-defined and match ordinary topic update behavior.

## Owner composition

The resolver passes only `topic_id`, `locale` and `slug` to the owner. It does
not read `forum_topic_route_aliases`, open a transaction, choose canonical
locales, resolve merge history or duplicate alias validation.

FORUM-24D therefore remains authoritative for localized-route locking, atomic
old-route redirect creation, topic timestamp/projection updates, exact replay,
payload-drift failure, merge canonicalization and delete-to-gone behavior.

## Compatibility and exclusions

This slice adds one GraphQL field and no migration. It changes no owner method,
route alias schema, semantic event, REST endpoint, admin/storefront UI or public
localized route mount.

Category routes, storefront mounting, host redirect/gone composition,
hreflang/SEO publication policy and retained runtime evidence remain follow-up
FORUM-24 scope. The canonical task remains `planned` until those release gates
and maintainer execution are complete.

## Verification

```bash
node scripts/verify/verify-forum-topic-slug-rename-owner.mjs
node scripts/verify/verify-forum-topic-slug-rename-graphql-transport.mjs
cargo test -p rustok-forum graphql::topic_slug_rename_mutation::tests -- --nocapture
cargo test -p rustok-forum --test topic_slug_rename_graphql_contract -- --nocapture
cargo test -p rustok-forum --test topic_slug_rename_sqlite -- --nocapture
cargo check -p rustok-forum --all-targets
```

No command above was run by the implementation agent, per maintainer request.
The source status is `source_ready_maintainer_execution_pending`.
