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
- Owns one explicit GraphQL transport for category/topic CRUD, reply previews, and the FORUM-21N topic-merge workflow; no REST fallback is retained.
- Presents category, topic, reply-preview, and merge workflows as module-owned Forum pages.
- Ships package-owned `admin/locales/en.json` and `admin/locales/ru.json` bundles.
- Embeds owner-side SEO panels through `rustok-seo-admin-support`.

## Entry points

- `ForumAdmin` — root route dispatcher for ordinary Forum admin pages and `/modules/forum/merge`.
- `rustok-module.toml [provides.admin_ui]` — host composition contract.

## FFA structure

- `admin/src/core.rs` owns the existing framework-agnostic category/topic view policy.
- `admin/src/topic_merge_model.rs` owns merge candidate, command, receipt, validation, accepted-solution selection, and retry-identity policy without Leptos imports.
- `admin/src/transport.rs` is the only UI-facing transport facade.
- `admin/src/transport/topic_merge_graphql_adapter.rs` composes `mergeForumTopic` and `mergeForumTopicResolvingSolution` through `rustok-graphql`.
- `admin/src/ui/root.rs` performs route-only composition.
- `admin/src/ui/topic_merge.rs` is the thin Leptos render/effect adapter.

## Transport state

The package remains a documented single-adapter GraphQL state. FORUM-21N does not pretend that a GraphQL call wrapped by a server function is a native owner path and never sends an access token inside a server-function DTO. A future native parity slice must add direct authenticated owner composition and preserve GraphQL for CSR/headless use.

## Interactions

- Consumed by `apps/admin` through manifest-driven code generation.
- Mounted under `/modules/forum` with child pages for topics, categories, and merge.
- Requires the routed tenant and `forum_topics:manage`; the backend owner revalidates both.
- Keeps one operation ID stable across an exact retry and rotates it whenever source, target, reason, or accepted-solution selection changes.
- Uses explicit source/target accepted-solution choice only when both topics are solved.

## Documentation

- See [platform docs](../../../docs/index.md).
- See [FORUM-21N admin merge UI](../docs/forum-21n-topic-merge-admin-ui.md).
