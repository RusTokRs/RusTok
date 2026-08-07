# rustok-channel

`rustok-channel` is an experimental core module that introduces a platform-level channel context for external delivery surfaces such as websites, applications, API clients, embedded targets, and other entry points.

## Purpose

`rustok-channel` owns the canonical channel model and resolution pipeline for RusToK delivery surfaces.

## Responsibilities

- Store the canonical `Channel` entity for platform-level delivery context.
- Track channel targets such as web domains, mobile apps, API clients, embedded surfaces, or external bindings.
- Bind platform modules to a channel in a lightweight, explicit way.
- Link channels to existing OAuth applications without introducing a second token subsystem.
- Provide a thin service layer for creating and querying experimental channel data.
- Back the shared request-level `ChannelContext` used by host transport layers.
- Own the domain resolution pipeline (`RequestFacts -> ResolutionDecision`) that host middleware applies.
- Own the durable database generation used to recover channel-resolution caches across serving replicas.
- Own a positive monotonic `channels.index_revision` storage column and retained `channel_index_tombstones`. Every Channel update advances the live revision exactly once; hard deletion retains an exact source version strictly above the final live row. These values remain storage-internal and are not exposed through Channel DTOs or the SeaORM write model.
- Own tenant-scoped `channel_index_identity_generations` for cross-owner Index relation freshness. This generation advances transactionally only when Channel identity resolution can change: insert, delete, id movement, tenant movement, or canonical slug change. `is_active`, targets, OAuth bindings, and resolution-policy changes do not advance this identity watermark.
- Publish only a neutral `ChannelRuntimeSelected` marker for selected cross-module composition. The Channel crate does not depend on `rustok-index` and does not construct generic Index mutations.
- The selected `rustok-distribution` bridge publishes the non-localized `rustok-channel::sales_channel@1` schema and one bounded source. Replay enumeration uses stable `channel_id` ordering and returns live upserts or retained hard-delete mutations from the same source identity; `index_revision` is used only as generic mutation `source_version`. The canonical Product graph separately consumes the tenant identity generation as a freshness watermark for Product-to-SalesChannel resolution.
- Ship the module-owned Leptos admin UI package for channel management.
- Expose `ChannelReadPort` / `channel.read_projection.v1` as the FBA provider boundary for channel/default/host-target read projections, with deadline-aware read semantics and no-compile executable fallback smoke evidence until executable runtime smoke is available.

## Scope

This crate intentionally ships a minimal v0 model:

- `channels`
- `channel_index_tombstones` as storage-internal replay identity state
- `channel_index_identity_generations` as tenant-scoped Product/SalesChannel relation freshness state
- `channel_targets`
- `channel_module_bindings`
- `channel_oauth_apps`

Current v0 wiring also includes:

- server-side channel resolution middleware now delegates to the domain-owned pipeline `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`, where `explicit default` means the tenant's explicit default channel and the built-in host fast-path intentionally remains a compatibility/performance layer before policy-only evaluation; runtime keeps active-only resolution semantics across all selectors plus typed `resolution_source + resolution_trace` diagnostics,
- the first typed domain resolution seam for the final architecture: `RequestFacts`, `ResolutionDecision`, `ResolutionTraceStep`, and a `ChannelResolver` that keeps precedence inside `rustok-channel`,
- persisted tenant-scoped typed resolution policies via `channel_resolution_policy_sets` and `channel_resolution_policy_rules`, with versioned JSON definitions, action-channel foreign keys, and deterministic rule order by `priority`,
- a trigger-backed `channel_resolution_invalidation_state` generation that advances in the same database transaction as mutations to channel, target, binding, OAuth-app binding, policy-set, and policy-rule tables; the server uses local/Redis publication as a fast path and periodically reconciles the persisted generation to recover missed delivery,
- the first live typed predicate set for policies: `HostEquals`, `HostSuffix`, `OAuthAppEquals`, `SurfaceIs`, and `LocaleEquals`,
- `web_domain` targets now use shared canonical normalization/validation (`scheme/path/port` trimming, lowercase, strict host validation), and host lookup reuses the same semantics as storage,
- a thin REST bootstrap/write surface in `apps/server`, now including policy-set/rule authoring, extended rule update patches (priority/is_active/action/predicates), rule reorder endpoints, and runtime trace diagnostics in channel bootstrap,
- `rustok-channel-admin` for Leptos admin composition, now including policy-set activation plus policy-rule authoring/edit/removal/reorder/enable-disable flows with build-profile-selected native `#[server]` transport and REST secondary path parity,
- live proof points in `rustok-pages`, `rustok-blog`, `rustok-commerce`, and `rustok-forum`, where public read-path gating already uses `channel_module_bindings`/resolved host `ChannelContext`; pages/blog exercise metadata-based publication-level `channelSlugs` allowlists, commerce preserves channel snapshot through storefront cart/order/pricing flows without a second sales-channel domain, and forum locks topic/reply/SEO visibility through `forum_topic_channel_access` plus request channel slug filtering.

Previously validated baseline:

- `cargo check -p rustok-channel`
- `cargo test -p rustok-channel --lib`
- `cargo check -p rustok-admin`
- `cargo check -p rustok-server`
- `cargo test -p rustok-api --lib`
- `cargo test -p rustok-server middleware::channel::tests --lib`
- `cargo test -p rustok-server registry_dependencies_match_runtime_contract --lib`
- `cargo test -p rustok-server registry_module_readmes_define_interactions_section --lib`
- `npm run verify:channel:fba` (no-compile provider registry, static matrix, and no-compile executable runtime fallback smoke gate)
- `npm run verify:channel:resolution-contract` (no-compile resolution order and built-in host fast-path decision gate)
- `npm run verify:channel:proof-points` (no-compile pages/blog/commerce/forum proof-point source/docs sync gate)

The new durable-generation path remains source-complete until the current permanent cache workflow and multi-replica failure-recovery scenarios pass on the same revision. The SalesChannel Index revision/tombstone/identity-generation paths likewise remain source-complete until owner execution admits delete/recreate, identity-movement, freshness, and replay evidence.

It does not yet provide:

- a full omnichannel orchestration model,
- channel-owned access token issuance,
- storefront UI,
- GraphQL transport adapters.

## Interactions

- `apps/server` registers the module as a core module, starts the supervised channel-cache invalidation runtime, and exposes its terminal state through critical runtime guardrails.
- `apps/server` resolves the active channel and exposes the thin transport surface, while the module keeps domain logic and durable generation ownership locally.
- `rustok-cache` supplies bounded invalidation transport; Redis PubSub is an acceleration path rather than the durable source of truth.
- `rustok-api` hosts the shared `ChannelContext` and request-level contracts.
- `rustok-auth` remains the source of truth for OAuth applications and tokens.
- `rustok-distribution` consumes the neutral selection marker and Channel-owned live/tombstone/identity-generation table contracts to publish generic SalesChannel Index capabilities and Product-to-SalesChannel freshness composition; Index core and server remain Channel-agnostic.
- Domain modules may gradually become channel-aware by reading channel context or channel bindings.
- The Leptos admin UI lives in `crates/rustok-channel/admin` and is mounted by `apps/admin` through manifest-driven wiring.

## Entry points

- `ChannelModule`
- `ChannelRuntimeSelected`
- `ChannelResolver`
- `ChannelService`
- `read_resolution_invalidation_generation`
- `controllers::routes`

## Next Steps

- Execute the permanent cache workflow and multi-replica durable-generation recovery scenarios.
- Execute SalesChannel hard-delete, identity-reuse/movement, Product relation freshness, replay, reconciliation, and restart evidence before admitting an authoritative Index consumer.
- Keep the current `channel_module_bindings + metadata` model for v0 while `pages` and `blog` continue to serve as proof points.
- Revisit a dedicated relation model only if future domains need stronger DB-level querying, authoring UX, or semantics that request-time filtering can no longer cover cleanly.
- Decide later whether `target`, `connector`, and publishable credentials should become separate concepts.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [M7 SalesChannel Index source contract](../rustok-index/docs/m7-sales-channel-source.md)
- [M7 Product-SalesChannel Freshness Witness](../rustok-product/docs/index-sales-channel-relation-freshness.md)
- [Platform docs index](../../docs/index.md)