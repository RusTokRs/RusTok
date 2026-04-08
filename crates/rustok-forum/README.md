# rustok-forum

## Purpose

`rustok-forum` owns the forum domain with forum-owned persistence.

## Responsibilities

- Provide `ForumModule` metadata for the runtime registry.
- Own forum categories, topics, replies, and moderation workflows.
- Own forum subscriptions through forum-owned `forum_category_subscriptions` and
  `forum_topic_subscriptions`.
- Own per-user forum statistics through forum-owned `forum_user_stats`.
- Own forum voting state through forum-owned `forum_topic_votes` and `forum_reply_votes`.
- Own accepted-solution workflow for Q&A-style topics through forum-owned `forum_solutions`.
- Own forum topic tag attachments through forum-owned `forum_topic_tags` while reusing
  `rustok-taxonomy` as the shared term dictionary.
- Own forum topic donor payload in `forum_topics.metadata`, including the live attached-mode
  Flex integration for locale-aware custom fields through parallel localized records.
- Apply module-owned reply lifecycle rules, including pending replies for moderated categories and approved-only public storefront reads.
- Own forum storage tables for categories, topics, translations, replies, and channel access.
- Expose shared multilingual contract fields on forum read surfaces:
  `requested_locale`, `effective_locale`, and `available_locales`.
- Own forum GraphQL and REST transport adapters alongside the domain services.
- Publish a module-owned Leptos admin UI package in `admin/` for host composition.
- Publish a module-owned Leptos storefront UI package in `storefront/` for host composition.
- Publish the typed RBAC surface for `forum_categories:*`, `forum_topics:*`,
  and `forum_replies:*`.

## Interactions

- Depends on `rustok-content` for shared rich-text, locale, and future orchestration helpers.
- Depends on `rustok-taxonomy` for the shared scope-aware term dictionary behind
  forum topic tags.
- Category slugs are translation-local, while topic slugs remain stable thread
  labels; current public forum lookup stays ID-based.
- Depends on `rustok-core` for module contracts, permissions, and `SecurityContext`.
- Depends on `rustok-api` for shared auth/tenant/request GraphQL+HTTP adapter contracts.
- Used by `apps/server` through thin GraphQL/REST shims and route composition.
- `apps/admin` consumes `rustok-forum-admin` through manifest-driven `build.rs` code generation, with a NodeBB-inspired moderation workspace mounted under `/modules/forum`.
- `apps/storefront` consumes `rustok-forum-storefront` through manifest-driven `build.rs` code generation, with a public NodeBB-inspired discussion feed mounted under `/modules/forum`.
- Declares permissions via `rustok-core::Permission`.
- Transport adapters validate forum permissions against `AuthContext.permissions`, then pass
  a permission-aware `SecurityContext` into forum services.
- Forum services now re-validate category/topic/reply/moderation permissions locally, so
  transport bugs can no longer bypass forum mutation or moderation policy.
- Topic solution marking now lives in forum-owned services and transport adapters; only
  approved replies can become solutions, and the read-path exposes `solution_reply_id`
  on topics plus `is_solution` on replies.
- Topic and reply voting now lives in forum-owned services and transport adapters; the
  read-path exposes `vote_score` plus viewer-specific `current_user_vote`, while
  GraphQL/REST can set or clear votes without expanding the module permission surface.
- Category and topic subscriptions now live in forum-owned services and transport
  adapters; the read-path exposes viewer-specific `is_subscribed`, and GraphQL/REST
  can subscribe or unsubscribe without introducing a new permission family.
- Per-user forum stats now live in forum-owned services and transport adapters; the
  module tracks `topic_count`, `reply_count`, and `solution_count` through topic/reply
  lifecycle and accepted-solution transitions, and exposes a dedicated read-path for
  user-level stats.
- Topic tag write-paths now resolve existing global taxonomy tags before creating
  new forum-local terms, while forum responses still expose the same `Vec<String>`
  tag contract.
- Topic metadata now participates in the same multilingual attached-value contract as
  other live Flex donors: shared keys stay in `forum_topics.metadata`, locale-aware
  keys persist in `flex_attached_localized_values`, and read surfaces resolve them
  against the effective locale/fallback chain instead of treating topic custom fields
  as a schema-only concern.

## Entry points

- `ForumModule`
- `TopicService`
- `ReplyService`
- `CategoryService`
- `ModerationService`
- `SubscriptionService`
- `UserStatsService`
- `VoteService`
- `graphql::ForumQuery`
- `graphql::ForumMutation`
- `controllers::routes`
- `admin::ForumAdmin` (publishable Leptos package)
- `storefront::ForumView` (publishable Leptos package)

See also [docs/README.md](docs/README.md).
