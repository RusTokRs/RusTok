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
- Keeps category/topic CRUD and reply previews on their existing explicit GraphQL transport.
- Selects direct authenticated native server functions for the FORUM-21 topic-merge workflow in SSR/hydrate builds and retains GraphQL for CSR/headless builds, without fallback.
- Presents category, topic, reply-preview, merge and selected-reply split workflows as module-owned Forum pages.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles.
- Embeds owner-side SEO panels through `rustok-seo-admin-support`.

## Entry points

- `ForumAdmin` — root route dispatcher for ordinary Forum admin pages plus `/modules/forum/merge` and `/modules/forum/split`.
- `rustok-module.toml [provides.admin_ui]` — host composition contract.

## FFA structure

- `admin/src/core.rs` owns the existing framework-agnostic category/topic view policy.
- `admin/src/topic_merge_model.rs` owns merge candidate, command, receipt, validation, accepted-solution selection, and retry-identity policy without Leptos imports.
- `admin/src/topic_split_model.rs` owns split command/receipt validation and the paired operation/target retry identities without Leptos imports.
- `admin/src/transport.rs` is the only UI-facing transport facade, selects exactly one merge transport per compile profile, and exposes the split manager GraphQL adapter without fallback.
- `crates/rustok-forum/admin/src/transport/topic_merge_native_server_adapter.rs` extracts server-side auth/tenant/runtime context and calls `TopicService` plus `ForumTopicMergeService` directly.
- `admin/src/transport/topic_merge_graphql_adapter.rs` preserves CSR/headless parity through `mergeForumTopic` and `mergeForumTopicResolvingSolution`.
- `admin/src/transport/topic_split_graphql_adapter.rs` composes `splitForumTopicReplies` and bounded candidate/reply reads without owner policy.
- `admin/src/ui/root.rs` performs route-only composition.
- `admin/src/ui/topic_merge.rs` and `admin/src/ui/topic_split.rs` are thin Leptos render/effect adapters.

## Transport state

FORUM-21O replaces the historical FORUM-21N GraphQL-only Leptos state with compile-profile selection:

- `ssr` and `hydrate` use native server functions;
- `csr` and headless/default builds use GraphQL;
- a failed selected path is returned as an error and never triggers cross-path fallback;
- native request DTOs contain locale or the framework-neutral merge command only, never access tokens, tenant IDs, actor IDs, permission snapshots, database handles, or event-bus handles.

The native adapter derives tenant and actor authority from `TenantContext` and `AuthContext`, obtains the database and `TransactionalEventBus` from `HostRuntimeContext`, rechecks `forum_topics:list` or `forum_topics:manage`, and delegates to the existing Forum owners.

## Interactions

- Consumed by `apps/admin` through manifest-driven code generation.
- Mounted under `/modules/forum` with child pages for topics, categories, merge and split.
- Requires the routed tenant and `forum_topics:manage`; both transport adapters and the backend owner revalidate authority.
- Keeps one operation ID stable across an exact retry and rotates it whenever source, target, reason, or accepted-solution selection changes.
- Uses explicit source/target accepted-solution choice only when both topics are solved.

## Documentation

- See [platform docs](../../../docs/index.md).
- See [FORUM-21N admin merge UI](../docs/forum-21n-topic-merge-admin-ui.md).
- See [FORUM-21O native admin merge transport](../docs/forum-21o-topic-merge-native-admin.md).
- See [FORUM-21V selected-reply split admin composition](../docs/forum-21v-topic-split-admin-ui.md).
