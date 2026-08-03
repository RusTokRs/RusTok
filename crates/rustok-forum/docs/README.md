# Documentation `rustok-forum`

`rustok-forum` is the domain module for forum/Q&A scenarios. The module operates on
forum-owned persistence and remains an independent bounded-context boundary.

## Canonical roadmap

[The implementation plan](./implementation-plan.md) is the single source of
truth for Forum task status, sequencing, definitions of done, the planned shared
notifications module, and cross-module release gates.

## Purpose

- publish the canonical Forum runtime contract for categories, topics, replies and moderation;
- keep Forum-owned transport surfaces, Q&A capabilities and UI packages inside the module;
- keep REST handlers on a narrow `ForumHttpRuntime` with explicit DB/event bus handles;
- resolve selected merged-source topic IDs through the immutable merge receipt ledger;
- expose manager-only merge commands and module-owned admin composition without duplicating owner policy;
- evolve the Forum as a taxonomy-aware and channel-aware domain with explicit observability.

## Scope

- `CategoryService`, `TopicService`, `ReplyService`, `ModerationService`;
- Forum-owned storage for categories, topics, replies, votes, solutions, subscriptions and user stats;
- transport surfaces: GraphQL, REST, Leptos admin/storefront packages and the module-owned Next-admin package;
- Forum widget contract-freeze and Page Builder consumer evidence;
- tag attachments via `forum_topic_tags` with shared vocabulary in `rustok-taxonomy`;
- visibility, moderation and user-facing derived fields in Forum read/write contracts.

## Integration

- uses `rustok-content` only as a shared helper/orchestration dependency;
- uses `rustok-taxonomy` as a shared dictionary for tag identity;
- uses `rustok-profiles` for the author presentation contract;
- uses `rustok-channel` for visibility and SEO gating;
- selected merge reads resolve through the immutable receipt, while mutation commands keep exact identity semantics;
- `mergeForumTopic` and `mergeForumTopicResolvingSolution` remain the only admin merge command contracts;
- FORUM-21N composes those commands in Leptos and Next-admin without changing the owner, receipt or event schema;
- FORUM-21O selects direct authenticated native owner composition for Leptos SSR/hydrate while retaining GraphQL for CSR/headless with no fallback.

## Verification

- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- `npm run verify:forum:admin-boundary`
- `npm run verify:forum:storefront-boundary`
- task-specific owner, transport, UI and runtime commands from the canonical plan

## Related documents

- [README crate](../README.md)
- [Canonical implementation plan](./implementation-plan.md)
- [FORUM-21B merge owner](./forum-21b-topic-merge-owner.md)
- [FORUM-21H accepted-solution policy](./forum-21h-topic-merge-solution-policy.md)
- [FORUM-21L competing solution resolution](./forum-21l-topic-merge-solution-resolution.md)
- [FORUM-21M checked cross-category merge](./forum-21m-topic-merge-cross-category.md)
- [FORUM-21N admin merge workflow](./forum-21n-topic-merge-admin-ui.md)
- [FORUM-21O native Leptos merge transport](./forum-21o-topic-merge-native-admin.md)
- [FORUM-21I/J canonical resolution and HTTP redirect](./forum-21i-topic-canonical-resolution.md)
- [FORUM-21K topic merge GraphQL transport](./forum-21k-topic-merge-graphql-transport.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
