# Documentation `rustok-forum`

`rustok-forum` is the domain module for forum/Q&A scenarios. The module already operates on
forum-owned persistence and must remain an independent bounded context
boundary, not reverting back to the shared storage model.

## Purpose

- publish the canonical forum runtime contract for categories, topics, replies and moderation;
- keep forum-owned transport surfaces, Q&A capabilities and UI packages inside the module;
- keep REST handlers on a narrow `ForumHttpRuntime` with explicit DB/event bus handles; `controllers::axum_router` builds it from `HostRuntimeContext` and generated host composition mounts it without a framework adapter;
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
- uses `rustok-channel` for visibility/pilot gating on the public read-path: channel-restricted topics are stored in `forum_topic_channel_access`, public GraphQL checks `channel_module_bindings`, and SEO/read-path filters consume the host-provided request channel slug.
- `rustok-forum/admin` already embeds owner-side SEO panels through `rustok-seo-admin-support`,
  and `rustok-seo` now holds target kinds `forum_category` and `forum_topic` for the shared runtime/resolver contract.

## Verification

- `cargo xtask module validate forum`
- `cargo xtask module test forum`
- `npm run verify:page-builder:consumer:forum` for fast FBA consumer guardrail without compilation, including Wave 1 smoke/SLO/trace anti-drift checks;
- targeted tests for topic/reply lifecycle, moderation, votes, subscriptions and visibility contracts;
- `npm run verify:channel:proof-points` for no-compile capture of forum channel-aware read-path/SEO markers

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Admin UI package](../admin/README.md)
- [Storefront UI package](../storefront/README.md)
- [Event flow contract](../../../docs/architecture/event-flow-contract.md)
