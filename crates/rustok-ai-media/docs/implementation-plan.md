# План реализации `rustok-ai-media`

## Цель

Сделать `rustok-ai-media` domain-owned adapter crate для media AI verticals, начиная с `image_asset` task/tool identity и pure validation contracts.

## Этапы

1. Scaffold crate + docs.
2. Перенести `image_asset` task/tool identity из `rustok-ai` в media-owned descriptor API.
3. Перенести validation helpers для generated/runtime payload contracts.
4. Добавить targeted verification и синхронизировать central registry evidence.

## Execution checkpoint

- Создан support crate `rustok-ai-media` с local docs.
- Перенесены `IMAGE_ASSET_TASK_SLUG`, `IMAGE_ASSET_TOOL_NAME`, descriptor registry и `register_media_ai_vertical_handlers` adapter API.
- Перенесена canonical image-size normalization/validation (`WIDTHxHEIGHT`, numeric bounds `1..=4096`) в media-owned pure helper, consumed by `rustok-ai` direct media runtime.
- Runtime fallback smoke evidence для `MediaAssetReadPort` source-level профиля закрыт в `contracts/evidence/ai-media-runtime-fallback-smoke.json`; следующий шаг: расширить media-owned generated artifact contract при разрешённых проверках.
- Added compile-free static evidence coverage in the unified `scripts/verify/verify-ai-domain-verticals.mjs` gate for descriptor ownership, runtime binding seams, and validation/policy tests without compilation.
- Last updated at (UTC): 2026-06-24T00:00:00Z

## FFA/FBA status

- FFA status: `not_started`
- FBA status: `in_progress`
- Structural shape: `domain_support_adapter`
- Evidence: crate owns media AI vertical task/tool identity, handler adapter API and pure image-size validation while executable provider/runtime composition remains in `rustok-ai`; FBA support-consumer metadata is locked in `crates/rustok-ai-media/contracts/ai-media-fba-registry.json` for `MediaAssetReadPort` / `media.asset_read.v1`, mirrored by `crates/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json`, and checked by `scripts/verify/verify-ai-media-fba.mjs` without long compilation; status remains below `boundary_ready` until runtime fallback smoke lands.
