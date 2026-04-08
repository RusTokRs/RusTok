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
- Ship the module-owned Leptos admin UI package for channel management.

## Scope

This crate intentionally ships a minimal v0 model:

- `channels`
- `channel_targets`
- `channel_module_bindings`
- `channel_oauth_apps`

Current v0 wiring also includes:

- server-side channel resolution middleware now delegates to the domain-owned pipeline `header -> query -> built-in host slice -> policy seam -> default`, where `default` means the tenant's explicit default channel; runtime keeps active-only resolution semantics across all selectors and typed `resolution_source` diagnostics,
- the first typed domain resolution seam for the final architecture: `RequestFacts`, `ResolutionDecision`, `ResolutionTraceStep`, and a `ChannelResolver` that keeps precedence inside `rustok-channel`,
- persisted tenant-scoped typed resolution policies via `channel_resolution_policy_sets` and `channel_resolution_policy_rules`, with versioned JSON definitions, action-channel foreign keys, and deterministic rule order by `priority`,
- the first live typed predicate set for policies: `HostEquals`, `HostSuffix`, `OAuthAppEquals`, `SurfaceIs`, and `LocaleEquals`,
- `web_domain` targets now use shared canonical normalization/validation (`scheme/path/port` trimming, lowercase, strict host validation), and host lookup reuses the same semantics as storage,
- a thin REST bootstrap/write surface in `apps/server`,
- `rustok-channel-admin` for Leptos admin composition,
- live proof points in `rustok-pages` and `rustok-blog`, where public read-path gating already uses `channel_module_bindings`, and both modules now exercise metadata-based publication-level `channelSlugs` allowlists.

Validated baseline:

- `cargo check -p rustok-channel`
- `cargo test -p rustok-channel --lib`
- `cargo check -p rustok-admin`
- `cargo check -p rustok-server`
- `cargo test -p rustok-api --lib`
- `cargo test -p rustok-server middleware::channel::tests --lib`
- `cargo test -p rustok-server registry_dependencies_match_runtime_contract --lib`
- `cargo test -p rustok-server registry_module_readmes_define_interactions_section --lib`

It does not yet provide:

- a full omnichannel orchestration model,
- channel-owned access token issuance,
- storefront UI,
- GraphQL transport adapters.

## Interactions

- `apps/server` registers the module as a core module and wires its runtime presence.
- `apps/server` resolves the active channel and exposes the thin transport surface, while the module keeps domain logic locally.
- `rustok-api` hosts the shared `ChannelContext` and request-level contracts.
- `rustok-auth` remains the source of truth for OAuth applications and tokens.
- Domain modules may gradually become channel-aware by reading channel context or channel bindings.
- The Leptos admin UI lives in `crates/rustok-channel/admin` and is mounted by `apps/admin` through manifest-driven wiring.

## Entry points

- `ChannelModule`
- `ChannelResolver`
- `ChannelService`
- `controllers::routes`

## Next Steps

- Keep the current `channel_module_bindings + metadata` model for v0 while `pages` and `blog` continue to serve as proof points.
- Revisit a dedicated relation model only if future domains need stronger DB-level querying, authoring UX, or semantics that request-time filtering can no longer cover cleanly.
- Roll out tenant-scoped typed resolution policies as the next architecture phase, without introducing a second fallback concept beyond explicit default channel.
- Decide later whether `target`, `connector`, and publishable credentials should become separate concepts.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
