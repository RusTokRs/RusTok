# Documentation `rustok-forum`

`rustok-forum` is the domain module for forum/Q&A scenarios. The module already operates on
forum-owned persistence and must remain an independent bounded context
boundary, not reverting back to the shared storage model.

## Canonical roadmap

[The implementation plan](./implementation-plan.md) is the single source of
truth for forum task status, sequencing, definitions of done, the planned
shared notifications module, and cross-module release gates. Other forum
documents describe stable contracts only and must not duplicate its backlog.

## Purpose

- publish the canonical forum runtime contract for categories, topics, replies and moderation;
- keep forum-owned transport surfaces, Q&A capabilities and UI packages inside the module;
- keep REST handlers on a narrow `ForumHttpRuntime` with explicit DB/event bus handles; `controllers::axum_router` builds it from `HostRuntimeContext` and generated host composition mounts it without a framework adapter;
- resolve selected merged-source topic IDs through the immutable merge receipt ledger to one retained canonical target;
- return an authorization-safe permanent redirect from the existing ID-based REST GET route when its path ID is a merged source, while direct targets retain the existing JSON response;
- expose the idempotent same-category or checked cross-category topic merge owner through routed-tenant, `forum_topics:manage` GraphQL commands that return the immutable owner receipt;
- transfer only the exact published-reply category aggregate during a cross-category merge while retaining source and target topic category identities and the exact schema-version-1 semantic event;
- resolve two valid competing accepted solutions only through an explicit manager-selected reply identity, preserving the winner and auditing the rejected candidate in the same merge transaction;
- evolve the forum as a taxonomy-aware and channel-aware domain with an explicit observability surface.

## Scope

- `CategoryService`, `TopicService`, `ReplyService`, `ModerationService`;
- forum-owned storage for categories, topics, replies, votes, solutions, subscriptions and user stats;
- transport surfaces: GraphQL, REST, Leptos admin/storefront packages;
- forum widget contract freeze surfaces: `ForumWidgetContractService`, REST endpoints `/api/forum/widgets/catalog` + `/api/forum/widgets/validate`, GraphQL query `forumWidgetCatalog`;
- forum page-builder consumer evidence: FW-2 static fallback matrix plus live Wave 1 rollout packet with control-plane audit trail, fallback/no-5xx guarantees, complete smoke outcomes, numeric SLO checks, forum-owned observability traces, keep decision, owner approvals, a monthly refresh policy, non-empty required refresh sections, and machine-readable latest-refresh provenance;
- tag attachments via `forum_topic_tags` with shared vocabulary in `rustok-taxonomy`;
- visibility, moderation and user-facing derived fields in forum read/write contracts.

## Integration

- uses `rustok-content` only as a shared helper/orchestration dependency;
- uses `rustok-taxonomy` as a shared dictionary for tag identity;
- uses `rustok-profiles` for the author presentation contract;
- the server GraphQL host binds `ProfileSummaryLoader` to the current anonymous,
  authenticated-human, or trusted-service audience before Forum topic/reply
  resolvers return `authorProfile`; the Profiles owner batch removes restricted,
  hidden, blocked, missing, and cross-tenant summaries before localized profile
  and tag reads, without per-author privacy calls;
- standalone/custom GraphQL hosts must attach the same audience-bound loader;
  `ProfileSummaryLoader::new` is anonymous and fail-closed by default;
- uses `rustok-channel` for visibility/pilot gating on the public read-path: channel-restricted topics are stored in `forum_topic_channel_access`, public GraphQL checks `channel_module_bindings`, and SEO/read-path filters consume the host-provided request channel slug.
- `rustok-forum/admin` already embeds owner-side SEO panels through `rustok-seo-admin-support`,
  and `rustok-seo` now holds target kinds `forum_category` and `forum_topic` for the shared runtime/resolver contract.

## Verification

- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- `npm run verify:page-builder:consumer:forum` for fast FBA consumer guardrail without compilation, including Wave 1 smoke/SLO/trace anti-drift checks;
- targeted tests for topic/reply lifecycle, moderation, votes, subscriptions,
  visibility contracts, merged-topic canonical resolution, HTTP redirects,
  checked cross-category merge ownership, manager-only merge GraphQL composition
  and explicit competing-solution resolution;
- `npm run verify:channel:proof-points` for no-compile capture of forum channel-aware read-path/SEO markers

## Related documents

- [README crate](../README.md)
- [Canonical implementation plan](./implementation-plan.md)
- [FORUM-21B merge owner](./forum-21b-topic-merge-owner.md)
- [FORUM-21M checked cross-category merge](./forum-21m-topic-merge-cross-category.md)
- [FORUM-21H accepted-solution policy](./forum-21h-topic-merge-solution-policy.md)
- [FORUM-21L competing solution resolution](./forum-21l-topic-merge-solution-resolution.md)
- [FORUM-21I/J canonical resolution and HTTP redirect](./forum-21i-topic-canonical-resolution.md)
- [FORUM-21K topic merge GraphQL transport](./forum-21k-topic-merge-graphql-transport.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
