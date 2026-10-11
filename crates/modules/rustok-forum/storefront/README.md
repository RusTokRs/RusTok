# rustok-forum-storefront

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../../docs/UI/module-package-verification.md)

Leptos storefront UI package for the `rustok-forum` module.

## Responsibilities

- Exposes the forum storefront root view used by `apps/storefront`.
- Keeps forum-specific storefront UX inside the module package.
- Participates in the manifest-driven UI composition path through `rustok-module.toml`.
- Owns dual-path storefront data access for category feeds, selected threads, reply rendering, authenticated visible-topic unread state, canonical topic-route admission, and composer topic/reply writes with inline quotes.
- Adds native Leptos `#[server]` calls while keeping GraphQL as a required parallel fallback.
- Builds topic-card links with the localized canonical route `/{locale}/forum/t/{short_id}/{slug}` instead of UUID query links.
- Presents the module as a NodeBB-inspired public discussion surface.
- Ships package-owned `storefront/locales/en.ftl`, `storefront/locales/ru.ftl`, and `storefront/locales/ar.ftl` bundles declared through `[provides.storefront_ui.i18n]`; every composer and thread label resolves through `crate::i18n::t` with the locale published by `UiRouteContext`.

## Entry Points

- `ForumView` - root storefront view rendered from the host storefront slot registry.
- `resolve_storefront_topic_route` - selected-path canonical/redirect resolver used by the storefront host.

## Interactions

- Consumed by `apps/storefront` via manifest-driven `build.rs` code generation.
- Uses build-profile-selected native `#[server]` calls with GraphQL selected path and shared host libraries such as `UiRouteContext`.
- Keeps GraphQL `forumStorefrontTopicRoute` and native `forum/storefront-topic-route` aligned on `ForumTopicRouteService` plus `ForumTopicAudienceReadService`.
- Keeps GraphQL `createForumTopicWithQuotes`/`createForumReplyWithQuotes` and native `forum/storefront-topic-create`/`forum/storefront-reply-create` aligned on `TopicService::create_command_with_audience_context` and `ReplyService::create_command_with_audience_context`, each pinning its own `Forum{Topic,Reply}CreateTransport` label in the audience context. A failed selected transport never falls back to the other path.
- Should remain compatible with the host storefront slot and generic module page contract, including locale-prefixed routes via `UiRouteContext::module_route_base()`.
- Reads the effective locale from `UiRouteContext.locale` for chrome copy and renders server-sanitized richtext projections.
- Keeps public category/topic/reply reads as the compatibility baseline. Authenticated requests enrich only the already storefront-visible topic IDs with the canonical Forum unread owner projection.
- Rechecks the canonical route target through the exact category/topic audience owner before disclosure or SSR composition.
- Rechecks storefront visibility before marking the selected topic read. Anonymous requests never create read rows or receive synthetic unread values.
- Degrades to the public feed only when authentication or the required Forum topic permission is absent; network, HTTP, persistence and domain failures remain explicit.
- Hides deleted-route tombstones until Forum owns a visibility-authorized disclosure snapshot; the Rust host therefore returns the same public `404` for missing, hidden, deleted and `GONE` routes.
- Does not expose category-subtree or tenant-wide mark-read commands because those owner scopes are not yet narrowed to the storefront channel-visible topic set.
- A composer quote is sent as an owner relation only when it carries an exact positive source revision (`forum_storefront_quote_revision`). The Leptos Reply card quotes the reply text and leaves the relation unpinned: the storefront reply read model carries no revision, and `verify-forum-reply-reactions-storefront-composition.mjs` forbids the module UI from fanning out `fetch_storefront_reply_current_revision` per visible reply (that lookup stays host-owned and selected-reply only). No revision number is invented, so an edited source can never be mis-pinned.

## Known Limitations / Pending Implementation

- The composer posts plain text only. Tags, explicit slug, attachments, edit-in-place, subscribe, vote, and report actions have no storefront transport here, so they stay unreachable in this surface while the owner already exposes some of them through GraphQL. Quoting a reply from the Leptos thread posts quoted text without a pinned owner relation (see Interactions).
- Topic cards show the owner status token through a localized badge
  (`forum.topic.statusOpen|statusClosed|statusArchived|statusPending`), and an unrecognized token
  is rendered unchanged rather than relabeled.
- Submit failures are shown with the redacted transport message (`Forum GraphQL request failed` / `Forum server request failed`). The owner reason stays server-side by contract, so an author cannot tell a locked topic from a cooldown in the UI.
- The topic feed renders one server page per direction: fixed page sizes (12 topics, 20 replies),
  no cursor paging, and client-side filtering over the fetched page while the header count stays
  server-authored. The owner returns `nextCursor` for the feed query and reads
  `topics_per_page` / `replies_per_page` for its own defaults; neither the cursor nor those
  settings are consumed here, so a feed cannot page past the fetched page.
- Vote, subscribe, and report mutations stay unreachable in this surface, so `is_subscribed` and
  `current_user_vote` are not part of the read model this package renders.

## Documentation

- [FORUM-24I canonical topic route mount](../docs/forum-24i-topic-route-storefront-mount.md)
- See [platform docs](../../../../docs/index.md).

## FFA boundary

The package keeps runtime-independent storefront policy in `src/core.rs`: canonical topic hrefs, rich-content summaries, count/slug labels, category/topic card view-models, unread badge/card mapping, accent fallback, and stable status badge class mapping. `src/transport/` remains the build-profile-selected native/GraphQL selected-path facade, while `src/ui/leptos.rs` is the explicit Leptos adapter. The host receives only visibility-admitted route DTOs and owns HTTP composition; it does not read Forum storage or alias history. The fast non-compiling guardrails are `npm run verify:forum:storefront-boundary` and `node scripts/verify/verify-forum-topic-route-storefront-mount.mjs`.
