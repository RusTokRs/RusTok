# `rustok-ai-media` — Implementation Plan

## Goal

Make `rustok-ai-media` the domain-owned adapter crate for media AI verticals, starting with `image_asset` task/tool identity and pure validation contracts.

## Stages

1. Scaffold crate + docs.
2. Move `image_asset` task/tool identity from `rustok-ai` to media-owned descriptor API.
3. Move validation helpers for generated/runtime payload contracts.
4. Add targeted verification and synchronize central registry evidence.

## Execution checkpoint

- Support crate `rustok-ai-media` created with local docs.
- `IMAGE_ASSET_TASK_SLUG`, `IMAGE_ASSET_TOOL_NAME`, descriptor registry and `register_media_ai_vertical_handlers` adapter API moved.
- Canonical image-size normalization/validation (`WIDTHxHEIGHT`, numeric bounds `1..=4096`) moved to media-owned pure helper, consumed by `rustok-ai` direct media runtime.
- Runtime fallback smoke evidence for `MediaAssetReadPort` source-level profile closed in `contracts/evidence/ai-media-runtime-fallback-smoke.json`; next step: expand media-owned generated artifact contract when compilations are allowed.
- Added compile-free static evidence coverage in the unified `scripts/verify/verify-ai-domain-verticals.mjs` gate for descriptor ownership, runtime binding seams, and validation/policy tests without compilation.
- Last updated at (UTC): 2026-06-24T00:00:00Z

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `boundary_ready`
- Structural shape: `no_ui_boundary`
- Evidence: crate owns media AI vertical task/tool identity, handler adapter API and pure image-size validation while executable provider/runtime composition remains in `rustok-ai`; FBA support-consumer metadata is locked in `crates/rustok-ai-media/contracts/ai-media-fba-registry.json` for `MediaAssetReadPort` / `media.asset_read.v1`, including `skip_asset_enrichment`, `proxy_storage_relative_url`, and `summarize_internal_binary` degraded modes, mirrored by `crates/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json` and runtime-verified smoke `crates/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json`, and checked by `scripts/verify/verify-ai-media-fba.mjs` plus `cargo test -p rustok-ai-media --lib`.
