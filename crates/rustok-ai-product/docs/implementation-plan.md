# `rustok-ai-product` — Implementation Plan

## Goal

Make `rustok-ai-product` the canonical domain-owned point for product AI verticals.

## Stages

1. Create crate + docs contracts.
2. Move `product_attributes` direct wiring from `rustok-ai` to this crate's registration API.
3. Move `product_copy` direct wiring.
4. Add targeted tests and validation contracts.

## Execution checkpoint

- Initial scaffold crate and documentation created.
- Generated payload contracts and basic validation for `product_copy` / `product_attributes` moved to `rustok-ai-product`; `rustok-ai` consumes these validators in the direct generation path.
- Domain-owned registration metadata API (`product_ai_verticals`) added for `product_copy` / `product_attributes`; runtime handler registration in `rustok-ai` uses these task/tool constants.
- Added compile-free static verification gate `scripts/verify/verify-ai-domain-verticals.mjs` for product descriptors, runtime binding seam, and generated payload validators.
- Last updated at (UTC): 2026-06-23T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- Evidence:
  - `admin/src/core.rs`, `admin/src/transport.rs`, and `admin/src/ui/leptos.rs` provide the module-owned admin FFA split.
  - Transport exposes a build-profile-selected native-server plus GraphQL selected-path profile; concrete host rendering remains a follow-up.
  - FBA support-consumer metadata is locked in `crates/rustok-ai-product/contracts/ai-product-fba-registry.json` for `ProductCatalogReadPort` / `product.catalog_read.v1`, including `generate_from_prompt_only`, `skip_catalog_enrichment`, and `require_operator_review` degraded modes, mirrored by `crates/rustok-ai-product/contracts/evidence/ai-product-consumer-static-matrix.json` and source-smoke `crates/rustok-ai-product/contracts/evidence/ai-product-runtime-fallback-smoke.json`, and checked by `scripts/verify/verify-ai-product-fba.mjs` without long compilation.
  - Boundary readiness is backed by executable `cargo test -p rustok-ai-product --lib` coverage for product-owned descriptors and generated payload validation.
  - The global readiness board uses the canonical hyphenated module slug `ai-product`.
