# План реализации `rustok-ai-product`

## Цель

Сделать `rustok-ai-product` канонической domain-owned точкой для product AI verticals.

## Этапы

1. Создать crate + docs contracts.
2. Перенести `product_attributes` direct wiring из `rustok-ai` в registration API этого crate.
3. Перенести `product_copy` direct wiring.
4. Добавить targeted tests и валидацию contracts.

## Execution checkpoint

- Создан начальный scaffold crate и документация.
- Перенесены generated payload contracts и базовая валидация `product_copy` / `product_attributes` в `rustok-ai-product`; `rustok-ai` consume-ит эти validators в direct generation path.
- Добавлен domain-owned registration metadata API (`product_ai_verticals`) для `product_copy` / `product_attributes`; runtime handler registration в `rustok-ai` использует эти task/tool constants.
- Added compile-free static verification gate `scripts/verify/verify-ai-domain-verticals.mjs` for product descriptors, runtime binding seam, and generated payload validators.
- Last updated at (UTC): 2026-06-23T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - `admin/src/core.rs`, `admin/src/transport.rs`, and `admin/src/ui/leptos.rs` provide the module-owned admin FFA split.
  - Transport exposes a native-server plus GraphQL fallback placeholder profile; concrete host rendering remains a follow-up.
  - FBA support-consumer metadata is locked in `crates/rustok-ai-product/contracts/ai-product-fba-registry.json` for `ProductCatalogReadPort` / `product.catalog_read.v1`, including `generate_from_prompt_only`, `skip_catalog_enrichment`, and `require_operator_review` degraded modes, mirrored by `crates/rustok-ai-product/contracts/evidence/ai-product-consumer-static-matrix.json` and source-smoke `crates/rustok-ai-product/contracts/evidence/ai-product-runtime-fallback-smoke.json`, and checked by `scripts/verify/verify-ai-product-fba.mjs` without long compilation.
  - Boundary readiness is backed by executable `cargo test -p rustok-ai-product --lib` coverage for product-owned descriptors and generated payload validation.
  - The global readiness board uses the canonical hyphenated module slug `ai-product`.
