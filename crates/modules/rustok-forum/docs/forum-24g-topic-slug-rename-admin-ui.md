# FORUM-24G topic slug rename admin UI

FORUM-24G composes the existing FORUM-24F GraphQL command in both required
module-owned admin surfaces. It does not change the FORUM-24D owner, the route
ledger, GraphQL schema or public storefront routing.

## Shared workflow

Both Leptos and Next-admin:

- load at most 100 localized topic candidates through the existing bounded
  `forumTopics` read;
- keep the candidate's exact topic ID and locale instead of asking an operator
  to reconstruct route identity;
- accept one bounded non-empty slug input;
- call `renameForumTopicSlug` with the routed tenant assertion and
  `forum_topics:update` authorization;
- display the previous path, current canonical path, locale, immutable alias ID
  and owner-provided `changed` flag;
- preserve exact replay so submitting the already-canonical normalized slug can
  return `changed = false` without a transport-side rejection.

The UI performs only input hygiene. `TopicService::rename_slug` remains the sole
authority for ownership, normalization, localized-route locking, alias
persistence, projection invalidation, merge canonicalization and delete-to-gone
behavior.

## Leptos admin

`/modules/forum/rename-slug` is registered as a module child page and dispatched
by `admin/src/ui/root.rs`.

The framework-neutral model lives in
`admin/src/topic_slug_rename_model.rs`. The thin UI adapter is
`admin/src/ui/topic_slug_rename.rs`, and `admin/src/transport.rs` delegates to
`admin/src/transport/topic_slug_rename_graphql_adapter.rs` with no REST/native
fallback.

The package consumes host-effective locale and package-owned English/Russian
copy. Authentication and tenant routing continue through the existing Leptos
auth/runtime contracts.

## Next-admin

`/dashboard/forum/rename-slug` is a thin host composition page. It resolves the
session, loads bounded topic candidates through the package API and mounts
`ForumTopicSlugRename` from `apps/next-admin/packages/forum`.

The package owns its command validation, GraphQL call, UI component, locales,
exports and registry-driven navigation item. Browser requests use the existing
same-origin GraphQL proxy; the client receives no access token. Navigation is
gated by `forum_topics:update` rather than the stronger topic-manager permission.

## Compatibility and exclusions

This slice adds no migration and changes no owner, alias table, semantic event,
REST route, storefront route, canonical-resolution rule, hreflang or SEO policy.
It does not create translations, remove routes, expose private routes or choose
fallback locales.

Public localized topic mounting, redirect/gone host responses, category routes,
hreflang/SEO publication policy and maintainer runtime evidence remain later
FORUM-24 work. The canonical task remains `planned`.

## Verification

```bash
node scripts/verify/verify-forum-topic-slug-rename-owner.mjs
node scripts/verify/verify-forum-topic-slug-rename-graphql-transport.mjs
node scripts/verify/verify-forum-topic-slug-rename-admin-ui.mjs
npm run verify:forum:admin-boundary
npm run verify:blog:forum-ui-ownership
cargo test -p rustok-forum-admin topic_slug_rename_model -- --nocapture
cargo check -p rustok-forum-admin --all-targets
npm --prefix apps/next-admin run typecheck
```

No command above was run by the implementation agent, per maintainer request.
The source status is `source_ready_maintainer_execution_pending`.
