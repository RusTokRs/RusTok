# rustok-forum-admin

> **For contributors and AI agents — choose the relevant guide before modifying this package:**
> [Architecture](../../../docs/UI/module-package-architecture.md) |
> [Implementation](../../../docs/UI/module-package-implementation.md) |
> [Verification](../../../docs/UI/module-package-verification.md)

Leptos admin UI package for the `rustok-forum` module.

## Responsibilities

- Exposes the forum admin root view used by `apps/admin`.
- Keeps forum-specific admin UX inside the module package.
- Participates in manifest-driven UI composition through `rustok-module.toml`.
- Keeps category/topic CRUD and reply reads on their explicit GraphQL transport.
- Selects direct authenticated native server functions for topic merge and reply creation in SSR/hydrate builds and retains GraphQL for CSR/headless builds, without fallback.
- Presents category, topic, reply authoring, localized route rename, merge, selected-reply split, reply-branch fork and reply-range move workflows as module-owned Forum pages.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles.
- Embeds owner-side SEO panels through `rustok-seo-admin-support`.

## Entry points

- `ForumAdmin` — root route dispatcher for ordinary Forum admin pages plus `/modules/forum/rename-slug`, `/modules/forum/merge`, `/modules/forum/split`, `/modules/forum/fork` and `/modules/forum/reply-range`.
- `rustok-module.toml [provides.admin_ui]` — host composition contract.

## FFA structure

- `admin/src/core.rs` owns the framework-agnostic category/topic/reply form and view policy.
- `admin/src/topic_merge_model.rs` owns merge candidate, command, receipt, validation, accepted-solution selection, and retry-identity policy without Leptos imports.
- `admin/src/topic_slug_rename_model.rs` owns localized route candidate/command/receipt mapping and bounded UI input validation without duplicating route normalization or alias policy.
- `admin/src/topic_split_model.rs` owns split command/receipt validation and the paired operation/target retry identities without Leptos imports.
- `admin/src/topic_reply_range_model.rs` owns exact endpoint/reason validation and the operation retry identity without Leptos imports.
- `admin/src/transport.rs` is the only UI-facing transport facade, selects exactly one topic-merge or reply-create transport per compile profile, and exposes slug rename, split, fork and reply-range GraphQL adapters without fallback.
- `admin/src/transport/native_server_support.rs` owns the shared trusted auth, tenant, module, permission and runtime extraction used by native Forum admin operations.
- `admin/src/transport/topic_merge_native_server_adapter.rs` calls `TopicService` plus `ForumTopicMergeService` directly after the shared native checks.
- `admin/src/transport/reply_create_native_server_adapter.rs` calls `ReplyService` directly with request-derived tenant and actor scope.
- `admin/src/transport/topic_merge_graphql_adapter.rs` preserves CSR/headless parity through `mergeForumTopic` and `mergeForumTopicResolvingSolution`.
- `admin/src/transport/graphql_adapter.rs` preserves reply-create CSR/headless parity through `createForumReply`; tenant scope is resolved from the trusted GraphQL request context unless an exact matching tenant UUID is supplied by a headless client.
- `admin/src/transport/topic_slug_rename_graphql_adapter.rs` composes `renameForumTopicSlug` and bounded localized topic candidates without reading route alias state.
- `admin/src/transport/topic_split_graphql_adapter.rs` composes `splitForumTopicReplies` and bounded candidate/reply reads without owner policy.
- `admin/src/transport/topic_reply_range_graphql_adapter.rs` composes `moveForumTopicReplyRange` without inferring owner positions or reading audit state.
- `admin/src/ui/root.rs` performs route-only composition.
- `admin/src/ui/topic_merge.rs`, `admin/src/ui/topic_slug_rename.rs`, `admin/src/ui/topic_split.rs`, `admin/src/ui/topic_fork.rs` and `admin/src/ui/topic_reply_range.rs` are thin Leptos render/effect adapters.

## Transport state

FORUM-21O replaces the historical FORUM-21N GraphQL-only Leptos state with compile-profile selection for topic merge:

- `ssr` and `hydrate` use native server functions;
- `csr` and headless/default builds use GraphQL;
- a failed selected path is returned as an error and never triggers cross-path fallback;
- native request DTOs contain locale or the framework-neutral merge command only, never access tokens, tenant IDs, actor IDs, permission snapshots, database handles, or event-bus handles.

Reply creation follows the same compile-profile selection. The shared
`discussion` editor produces one canonical `RichTextDocument`; native and
GraphQL adapters pass it to the same Forum reply owner and never replay a failed
write through the other transport.

Slug rename uses the additive FORUM-24F GraphQL mutation in all Leptos build profiles. The UI forwards only topic identity, the existing translation locale and the requested slug. `TopicService::rename_slug` remains authoritative for ownership, normalization, route locking, immutable alias creation, merge/delete lifecycle resolution and exact replay.

## Interactions

- Consumed by `apps/admin` through manifest-driven code generation.
- Mounted under `/modules/forum` with child pages for topics, categories, localized route rename, merge, split, fork and reply-range movement.
- Slug rename requires `forum_topics:update`; the GraphQL adapter and owner revalidate routed tenant and actor authority.
- Merge/split/fork/reply-range workflows require `forum_topics:manage` and retain their existing operation identity policies.
- The route rename result displays the owner-provided previous path, canonical path, locale, immutable alias ID and `changed` replay flag.

## Documentation

- See [platform docs](../../../docs/index.md).
- See [FORUM-21N admin merge UI](../docs/forum-21n-topic-merge-admin-ui.md).
- See [FORUM-21O native admin merge transport](../docs/forum-21o-topic-merge-native-admin.md).
- See [FORUM-21V selected-reply split admin composition](../docs/forum-21v-topic-split-admin-ui.md).
- See [FORUM-21W reply-branch fork admin composition](../docs/forum-21w-topic-fork-admin-ui.md).
- See [FORUM-21X reply-range move admin composition](../docs/forum-21x-reply-range-move-admin-ui.md).
- See [FORUM-24G topic slug rename admin UI](../docs/forum-24g-topic-slug-rename-admin-ui.md).

## Topic fork admin workflow

`/modules/forum/fork` composes the manager-only
`forkForumTopicReplyBranch` GraphQL command. The form retains operation and
target-topic UUIDs across an exact retry, rotates both when the command shape
changes, loads a bounded reply page for root selection, and displays the
immutable owner receipt. Descendant discovery, copy identity, policy cloning and
counter reconciliation remain in `ForumTopicForkService`.
