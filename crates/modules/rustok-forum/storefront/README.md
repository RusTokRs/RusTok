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
- Provides native Leptos `#[server]` calls and GraphQL as required parallel transports selected by build profile, never as runtime fallbacks.
- Builds topic-card links with the localized canonical route `/{locale}/forum/t/{short_id}/{slug}` instead of UUID query links.
- Presents the module as a NodeBB-inspired public discussion surface.
- Ships package-owned `storefront/locales/en.ftl`, `storefront/locales/ru.ftl`, and `storefront/locales/ar.ftl` bundles declared through `[provides.storefront_ui.i18n]`; every composer and thread label resolves through `crate::i18n::t` with the locale published by `UiRouteContext`, and every label that carries a number or a slug resolves through `crate::i18n::count_message` / `crate::i18n::slug_message` so the catalog binds `$count`/`$slug` itself. A caller-side `"{count} threads"` template is not an acceptable substitute: Fluent reports a format error for an unbound argument, the shared resolver turns any format error into `Failed`, and the caller literal then wins in every locale, which silently deletes all plural variants.

## Topic selection and continuation

`ForumTopicSelection` is a typed transport projection (`search`, `order`, `solvedOnly`,
`unreadOnly`) of the Forum list/read-state owners. Latest, Top, Solved, submitted search,
and authenticated Unread controls change the resource key; no predicate or ranking is
applied to loaded rows. Search is submitted explicitly instead of scanning the owner
on every keystroke. Controls remain mounted while the result reloads, including an
empty result, so the reader can change/reset the selection without losing input focus.
The search editor observes only changes to the applied search string: switching feed
modes does not erase unsubmitted text. Solved-topic badges use package-owned catalog
copy in English, Russian and Arabic rather than a hardcoded English label.

Both native and GraphQL initial/page requests forward the selection;
`StorefrontForumData.topic_selection` retains it so Load more cannot switch back to an
unselected feed. `read_state_available` selects the original read-state/audience owner;
an unread-only request cannot degrade to a public/full feed. A failed continuation
never switches transport or query. Category overrides are tracked inside the resource
source and clear a thread selected by the previous route. Reloads recreate the result
page state; topic and reply continuations check disposal and the complete captured
resource key (category, thread, locale, refresh nonce and selection) both before
dispatch and after completion. Initial results carry the originating request key
alongside the data; a retained Suspense result never adopts the current key when it
mounts or updates read-state availability for a newer selection. A retained result
cannot accept an old page or error during reload. Composer drafts belong to the
mounted Forum view, not the reloading results.

The GraphQL category connection reads `pageInfo.totalCount`; `total` is only the
package's internal projection, never a GraphQL connection field. Actual embedded
headless documents are validated by the Forum schema contract test, and response
fixtures exercise the category mapping. See the
[owner feed decision](../../../../DECISIONS/2026-10-17-forum-topic-feed-selection.md).

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
- A composer quote is sent as an owner relation only when it carries an exact positive source revision (`forum_storefront_quote_revision`). The Leptos Reply card pins that relation by fetching `forumStorefrontReplyQuoteRevision` for the one reply the author chose to quote. `verify-forum-reply-reactions-storefront-composition.mjs` still forbids fanning out `fetch_storefront_reply_current_revision` per visible reply (that lookup stays host-owned and selected-reply only). No revision number is invented: a missing quote revision stays quoted text in the body.

## Composer draft and submission contract

- Framework-independent composer state, validation and command construction live in `src/core.rs`.
  The Leptos adapter binds inputs and dispatches the selected transport only.
- In-memory drafts are scoped to topic plus parent reply, or to the posting category for a new topic.
  Switching topic, reply parent or mode saves the outgoing text/title/quotes and restores only the
  destination draft. Explicitly selecting an empty posting category moves the current new-topic
  draft; selecting a category with a saved draft preserves and restores both editors.
- A submission captures one transport command and freezes target selection and editor inputs until
  it finishes. Quote entry/removal cannot mutate that snapshot. Closing may hide the pending drawer,
  but cannot retarget it. Failure retains the draft; success clears only the submitted target.
- Drafts are memory-only for the mounted Forum view, not browser-persistent storage.

## Known Limitations / Pending Implementation

- The composer posts plain text only. Tags, explicit slug, attachments, edit-in-place, subscribe, and vote actions have no storefront transport here, so they stay unreachable in this surface while the owner already exposes some of them through GraphQL. Topic and reply report actions file `reportForumTopic` / `reportForumReply`. Quoting a reply from the Leptos thread pins the owner relation when the selected-reply quote revision is available (see Interactions).
- Topic cards show the owner status token through a localized badge
  (`forum.topic.statusOpen|statusClosed|statusArchived|statusPending`), and an unrecognized token
  is rendered unchanged rather than relabeled.
- Submit failures are shown with the redacted transport message (`Forum GraphQL request failed` / `Forum server request failed`). The owner reason stays server-side by contract, so an author cannot tell a locked topic from a cooldown in the UI.
- Both lists page forward only through the owner `nextCursor`: page sizes come from the owner
  `topics_per_page` / `replies_per_page` settings (the browser transport omits `perPage`, the server
  transport resolves them through the shared module-settings readers), appended pages are
  identity-deduplicated in `Forum{Topic,Reply}Page::append_page`, a failed page is shown inline with a
  retry instead of looking like the end of the thread, and the header count stays server-authored.
  The category overview remains limited to its initial page of 12 (the owner has no
  category page-size setting); it has no category continuation control. Selecting
  any loaded category requests an owner-filtered, cursor-paged topic feed.
- Vote and subscribe mutations stay unreachable in this surface, so `is_subscribed` and
  `current_user_vote` are not part of the read model this package renders.

## Documentation

- See [CRATE_API.md](CRATE_API.md) for full contract specification.
- See [Canonical implementation plan](../docs/implementation-plan.md).
- See [platform docs](../../../../docs/index.md).

## FFA boundary

The package keeps runtime-independent storefront policy in `src/core.rs`: canonical topic hrefs, rich-content summaries, count/slug labels, category/topic card view-models, unread badge/card mapping, accent fallback, and stable status badge class mapping. `src/transport/` remains the build-profile-selected native/GraphQL selected-path facade, while `src/ui/leptos.rs` is the explicit Leptos adapter. The host receives only visibility-admitted route DTOs and owns HTTP composition; it does not read Forum storage or alias history. `src/ui/topic_feed.rs` and `src/ui/timeline.rs` are the second half of that Leptos adapter: the feed component and the anchor-aware timeline scroller. The fast non-compiling guardrails are `npm run verify:forum:storefront-boundary` (it reads `src/ui/leptos.rs` and `src/ui/topic_feed.rs` together, so a rule cannot be satisfied in one file while the other one drifts), `node scripts/verify/verify-forum-storefront-cursor-paging.mjs`, and `node scripts/verify/verify-forum-topic-route-storefront-mount.mjs`.
