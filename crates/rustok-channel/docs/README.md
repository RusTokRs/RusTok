# Documentation `rustok-channel`

`rustok-channel` is an experimental core module that introduces platform-level
channel context for delivery surfaces and channel-aware runtime resolution.

## Purpose

- publish the canonical runtime entry type `ChannelModule`;
- keep channel resolution logic inside the module, not in `apps/server`;
- provide the platform with a unified channel-aware contract for host runtime and domain consumers.

## Responsibilities

- storage for `channels`, `channel_targets`, `channel_module_bindings`, `channel_oauth_apps`;
- storage for `channel_resolution_policy_sets` and `channel_resolution_policy_rules`;
- domain-owned resolution layer: `RequestFacts`, `ResolutionDecision`, `ResolutionTraceStep`, `ChannelResolver`;
- tenant-scoped typed resolution policies and explicit default channel semantics;
- canonical resolution order `explicit selectors -> built-in host slice -> typed policies -> explicit default -> unresolved`, where the built-in host fast-path remains a separate compatible layer before policy-only evaluation;
- module-owned Leptos admin UI package `rustok-channel-admin` with operator flow for policy authoring/edit/reorder/enable-disable and native-first `#[server]` + REST fallback transport parity;
- FBA provider boundary `ChannelReadPort` / `channel.read_projection.v1` for channel/default/host-target read projections, where `npm run verify:channel:fba` without compilation locks registry, static matrix and no-compile executable runtime fallback smoke (`embedded_native`, `rest_compatibility`, `unresolved_context`);
- source-locked proof points for `rustok-pages`, `rustok-blog`, `rustok-commerce` and `rustok-forum`, where `npm run verify:channel:proof-points` holds the usage of resolved host `ChannelContext`, `channel_module_bindings`, metadata `channelSlugs`/`forum_topic_channel_access` visibility, commerce channel snapshot without a second sales-channel domain and forum SEO/read-path channel filtering.

## Integration

- used by `apps/server` as a mandatory `Core` module and as a runtime composition root;
- publishes a shared host contract through `rustok-api` (`ChannelContext`, request-level metadata, `resolution_trace`);
- uses `rustok-auth` as the source of truth for OAuth applications and access tokens;
- already serves as a runtime proof point for `rustok-pages`, `rustok-blog`, `rustok-commerce` and `rustok-forum`, with their source/docs synchronization locked by `npm run verify:channel:proof-points`.

## Verification

- `cargo xtask module validate channel`
- `cargo xtask module test channel`
- targeted server middleware tests for resolution order and explicit default semantics
- `npm run verify:channel:fba` for no-compile FBA registry/static-matrix/runtime-fallback-smoke guardrail
- `npm run verify:channel:resolution-contract` for no-compile guardrail of resolution order and built-in host fast-path decision
- `npm run verify:channel:proof-points` for no-compile guardrail of current channel-aware proof points in `rustok-pages`, `rustok-blog`, `rustok-commerce` and `rustok-forum`

## Related documents

- [README crate](../README.md)
- [Implementation plan](./implementation-plan.md)
- [Manifest layer contract](../../../docs/modules/manifest.md)
