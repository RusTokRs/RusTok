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
- own deterministic localized topic route identity plus immutable redirect/tombstone history;
- own locale-aware category route identity plus immutable localized slug history without conflating identity and visibility authorization;
- expose manager-only move, merge, split, reply-branch fork and reply-range owners without duplicating policy;
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
- FORUM-21O selects direct authenticated native owner composition for Leptos SSR/hydrate while retaining GraphQL for CSR/headless with no fallback;
- FORUM-21P adds the transport-neutral selected-reply split owner with immutable receipt/event, parent-closed movement, exact access-policy cloning and counter reconciliation;
- FORUM-21Q adds the transport-neutral reply-branch fork owner with deterministic copied identities, complete bounded revision/relation provenance, source immutability and explicit non-copy policy; FORUM-21U provides its manager GraphQL transport;
- FORUM-21R exposes `splitForumTopicReplies` as a routed-tenant, `forum_topics:manage` GraphQL adapter over the unchanged split owner and immutable receipt; FORUM-21V provides its Leptos and Next-admin composition;
- FORUM-21S adds the transport-neutral bounded reply-range move owner with deterministic append positions, explicit asymmetric parent policy, unchanged reply-owned references and checked ACL/solution/counter reconciliation; FORUM-21T provides its manager GraphQL transport;
- FORUM-21T exposes `moveForumTopicReplyRange` as a routed-tenant, `forum_topics:manage` GraphQL adapter over the unchanged reply-range owner and immutable receipt; FORUM-21X provides its Leptos and Next-admin composition without inferring canonical positions from visible row order;
- FORUM-21U exposes `forkForumTopicReplyBranch` as a routed-tenant, `forum_topics:manage` GraphQL adapter over the unchanged fork owner and immutable receipt; admin composition remains follow-up scope.
- FORUM-21V composes `splitForumTopicReplies` in the module-owned Leptos and Next-admin surfaces with stable retry/target identities and no transport-local movement policy.
- FORUM-24A adds `ForumTopicRouteService`, a twelve-hex topic identity, exact-locale canonical descriptors and an append-only redirect/tombstone ledger; host mounting and owner write composition remain follow-up scope.
- FORUM-24B composes immutable localized source-route redirects into new topic merges in the same owner transaction without changing merge receipts or events.
- FORUM-24C records immutable localized `gone` routes in the topic delete transaction while preserving existing merge redirects.
- FORUM-24D adds an explicit owner command for localized topic slug changes with atomic old-route aliases and delete/merge lifecycle resolution.
- FORUM-24E provides a bounded, cursor-resumable owner repair that ensures exact route aliases for immutable merge receipts created before FORUM-24B.
- FORUM-24F exposes the localized topic slug rename owner through an additive routed-tenant GraphQL mutation while preserving owner-defined update and ownership semantics.
- FORUM-24G composes that mutation in the module-owned Leptos and Next-admin packages without adding route, alias, merge or locale-selection policy to either UI.
- FORUM-24H exposes visibility-safe storefront canonical/redirect resolution through a legacy-compatible GraphQL field and exact category/topic audience recheck.
- FORUM-24I composes the canonical localized topic route in `rustok-forum-storefront` and the shared Rust storefront router, cuts topic-card navigation over from UUID query links, and preserves native/GraphQL selected-path parity.
- FORUM-24J records an immutable anonymous visibility and route-channel snapshot before topic deletion, seals channel scope with count and SHA-256 digest, and exposes only a boolean owner decision.
- FORUM-24K adds a separate GraphQL route decision field, consumes the FORUM-24J boolean in GraphQL and native storefront paths, and maps only owner-authorized tombstones to private no-store `410 Gone` while preserving `404` for missing or unauthorized history.
- FORUM-24L adds `ForumCategoryRouteService` for `/{locale}/forum/c/{slug}`, reuses the existing tenant/locale slug uniqueness and shared locale fallback order, hides archived categories, and fails closed when first-available lookup crosses category identities; transport, aliases and SEO remain follow-up scope.
- FORUM-24M composes explicit and name-derived category slug changes into an append-only tenant/locale route ledger, permanently reserves historical keys, and resolves exact-locale aliases before fallback current routes; transport, tombstone disclosure and SEO remain follow-up scope.

## Verification

- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- `npm run verify:forum:admin-boundary`
- `npm run verify:forum:storefront-boundary`
- `node scripts/verify/verify-forum-topic-route-storefront-graphql.mjs`
- `node scripts/verify/verify-forum-topic-route-storefront-mount.mjs`
- `node scripts/verify/verify-forum-topic-route-tombstone-visibility-owner.mjs`
- `node scripts/verify/verify-forum-topic-route-authorized-gone.mjs`
- `node scripts/verify/verify-forum-category-route-identity-owner.mjs`
- `node scripts/verify/verify-forum-category-slug-alias-owner.mjs`
- task-specific owner, transport, UI and runtime commands from the canonical plan

## Related documents

- [README crate](../README.md)
- [Canonical implementation plan](./implementation-plan.md)
- [Accepted Forum slug/locale decision](../../../DECISIONS/2026-03-29-forum-slug-locale-contract.md)
- [FORUM-21B merge owner](./forum-21b-topic-merge-owner.md)
- [FORUM-21H accepted-solution policy](./forum-21h-topic-merge-solution-policy.md)
- [FORUM-21L competing solution resolution](./forum-21l-topic-merge-solution-resolution.md)
- [FORUM-21M checked cross-category merge](./forum-21m-topic-merge-cross-category.md)
- [FORUM-21N admin merge workflow](./forum-21n-topic-merge-admin-ui.md)
- [FORUM-21O native Leptos merge transport](./forum-21o-topic-merge-native-admin.md)
- [FORUM-21P selected-reply split owner](./forum-21p-topic-split-owner.md)
- [FORUM-21Q reply-branch fork owner](./forum-21q-topic-fork-owner.md)
- [FORUM-21R topic split GraphQL transport](./forum-21r-topic-split-graphql-transport.md)
- [FORUM-21S bounded reply-range move owner](./forum-21s-reply-range-move-owner.md)
- [FORUM-21T reply-range move GraphQL transport](./forum-21t-reply-range-move-graphql-transport.md)
- [FORUM-21U topic fork GraphQL transport](./forum-21u-topic-fork-graphql-transport.md)
- [FORUM-21V topic split admin composition](./forum-21v-topic-split-admin-ui.md)
- [`forum-21w-topic-fork-admin-ui.md`](./forum-21w-topic-fork-admin-ui.md) — FORUM-21W manager fork workflow composition for Leptos and Next-admin.
- [FORUM-21X reply-range move admin composition](./forum-21x-reply-range-move-admin-ui.md)
- [FORUM-21I/J canonical resolution and HTTP redirect](./forum-21i-topic-canonical-resolution.md)
- [FORUM-21K topic merge GraphQL transport](./forum-21k-topic-merge-graphql-transport.md)
- [FORUM-24A topic route identity owner](./forum-24a-topic-route-identity-owner.md)
- [FORUM-24B topic merge route aliases](./forum-24b-topic-merge-route-aliases.md)
- [FORUM-24C topic delete route tombstones](./forum-24c-topic-delete-route-tombstones.md)
- [FORUM-24D topic slug rename owner](./forum-24d-topic-slug-rename-owner.md)
- [FORUM-24E historical merge route backfill](./forum-24e-topic-merge-route-backfill.md)
- [FORUM-24F topic slug rename GraphQL transport](./forum-24f-topic-slug-rename-graphql-transport.md)
- [FORUM-24G topic slug rename admin UI](./forum-24g-topic-slug-rename-admin-ui.md)
- [FORUM-24H storefront topic route GraphQL transport](./forum-24h-topic-route-storefront-graphql.md)
- [FORUM-24I storefront topic route mount](./forum-24i-topic-route-storefront-mount.md)
- [FORUM-24J topic route tombstone visibility owner](./forum-24j-topic-route-tombstone-visibility.md)
- [FORUM-24K authorized topic route gone transport](./forum-24k-topic-route-authorized-gone.md)
- [FORUM-24L localized category route identity owner](./forum-24l-category-route-identity-owner.md)
- [FORUM-24M category slug alias owner](./forum-24m-category-slug-alias-owner.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
