# Implementation plan for `rustok-fulfillment`

Status: fulfillment boundary is separated; shipping options, fulfillments and typed
`fulfillment_items` already serve as the foundation for the deliverability domain, while the provider
SPI and post-order delivery changes still remain in the active backlog of umbrella
`rustok-commerce`.

## Execution checkpoint

- Current phase: storefront selection transport ownership and provider SPI live-adapter evidence
- Last checkpoint: Fulfillment storefront now owns both shipping-selection transports. `storefront/src/transport/native_server_adapter/raw_adapter.rs` exposes `fulfillment/select-shipping-option`, validates/materializes owner selection updates and calls `rustok_commerce::storefront_checkout_runtime`, while `storefront/src/transport/graphql_adapter.rs` keeps the parallel public GraphQL mutation fallback. `storefront/src/transport.rs` exposes the MissingServer-gated `select_shipping_option` facade without a commerce callback, and commerce no longer contains fulfillment GraphQL or native owner-operation wrappers.
- Next step: Continue production carrier adapter wiring separately; keep seller-aware shipping-selection parity locked by the owner storefront guardrail and commerce handoff guardrail.
- Open blockers: None.
- Hand-off notes for next agent: Without compilation: maintain fast source guardrails; at the next transport cutover, synchronize commerce plan and the central FFA/FBA readiness board.
- Last updated at (UTC): 2026-06-30T08:04:31Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- FBA contract version: `fulfillment.shipping_selection.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - FBA maintenance slice moved the read-only `list_seller_shipping_options` path to shared `PortCallPolicy::read()`, and the select/write path to shared `PortCallPolicy::write()`, preserving existing FBA metadata without changing runtime surface.
  - umbrella facade `rustok_commerce::{services::fulfillment, FulfillmentService}` is removed; commerce REST/GraphQL/admin/storefront/test consumers import `FulfillmentService` from `rustok-fulfillment` directly, so fulfillment owner service is no longer masked by the ecommerce umbrella.
  - in-process implementation of `ShippingSelectionPort for FulfillmentService` added in `src/ports.rs`: read path filters shipping options by profile slug, select path requires shared `PortCallPolicy::write()` and maps `FulfillmentError` to `PortError`;
  - `src/ports.rs` now exports `ShippingSelectionPort` and DTO for seller-aware shipping options/selection operations; machine-readable registry and verifier check port trait operations match FBA metadata;
  - FBA-provider metadata is open for `seller-aware shipping selection` through `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`; provider SPI boundary raised to `boundary_ready` on executed live-adapter evidence, while base shipping-selection port contract/fallback evidence remains a follow-up before `transport_verified`;
  - storefront shipping-selection plan matches delivery groups only by `shipping_profile_slug + seller_id`; `seller_scope` is no longer accepted as a fallback target identity and is no longer part of the fulfillment-owned selection transport command/update DTOs or delivery-group model DTO;
  - registry now locks `contract_tests.status = planned_cases_locked`: for each port operation, an in-process/remote-adapter-placeholder case matrix is defined, baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) with explicit deadline enforcement for read path and `write_idempotency_required` only on write operations; fallback smoke profile set; static evidence packet `crates/rustok-fulfillment/contracts/evidence/fulfillment-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; this closes metadata/evidence anti-drift for future base port contract tests;
  - provider SPI evidence is now locked in `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-static-matrix.json`: manual/remote-placeholder cases for `quote_rates`/`create_label`/`cancel` check typed provider error mapping, idempotency-key preservation and prohibition of persistence in adapter layer, tracking webhook replay contract locks idempotent duplicate delivery and lifecycle transition delegation to `FulfillmentService`, and `src/providers.rs` contains external carrier registration contract (`ExternalFulfillmentProviderRegistration`, health/degraded-mode DTOs, descriptor-id validation, `FulfillmentProviderRegistry`) with source markers checked by `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs`; packet does not raise FBA status without runtime execution;
  - provider registry runtime-mode guardrails are now side-effect-free and check capability support, missing-provider errors and health/degraded-mode mapping before calling the carrier adapter; the registry also publishes guarded async `execute_quote_rates`/`execute_create_label`/`execute_cancel`/`execute_tracking_webhook` seams, which block unavailable carriers before adapter side effects and leave lifecycle persistence in `FulfillmentService`; targeted provider SPI tests lock fallback profile propagation and operation capability rejection without full compilation in this iteration;
  - provider SPI runtime-smoke evidence is now locked in `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-runtime-smoke.json`, and dedicated live-adapter contract in `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-contract.json`: no-compile packets lock missing-provider lookup, unsupported/unknown operation rejection, degraded fallback propagation, unavailable-provider non-executable mode, registration failure cases, webhook replay guardrails and mandatory live carrier execution cases; `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` checks this packet together with the static matrix;
  - live external carrier execution plan is now locked inside the runtime-smoke packet: verifier requires concrete-adapter evidence for guarded single invocation, typed provider-error mapping without lifecycle persistence, degraded fallback propagation, unavailable-mode adapter blocking and tracking webhook replay delegation;
  - live external carrier execution evidence is now locked in `crates/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json`: packet locks concrete-adapter contract execution for guarded single invocation, typed provider-error mapping without lifecycle persistence, degraded fallback profile `manual_shipping`, unavailable-mode adapter blocking and idempotent tracking webhook replay delegation; `scripts/verify/verify-ecommerce-provider-spi-evidence.mjs` now validates this executed evidence alongside static/runtime-smoke/contract packets without Cargo compilation and gates the `boundary_ready` status.
  - any UI/transport boundary changes must be locked with parity/boundary evidence in the same increment;
  - admin FFA slice added framework-agnostic `admin/src/core.rs` request policy for list and filters, module-owned `admin/src/transport.rs` facade, GraphQL adapter `admin/src/transport/graphql_adapter.rs` and explicit Leptos render adapter `admin/src/ui/leptos.rs`; `admin/src/lib.rs` now only wires modules and re-exports `FulfillmentAdmin`, legacy `admin/src/api.rs` removed, and Leptos adapter no longer calls raw adapter directly for covered shipping-option flows; fast guardrail `scripts/verify/verify-fulfillment-admin-boundary.mjs` locks boundary and docs sync without full-workspace compile;
  - storefront handoff + shipping-selection slice lives in `storefront/src/model.rs`, `storefront/src/core/mod.rs`, `storefront/src/transport.rs`, `storefront/src/transport/graphql_adapter.rs`, `storefront/src/transport/native_server_adapter/raw_adapter.rs` and `storefront/src/ui/leptos.rs`; fulfillment owns seller-aware presentation/normalization, update materialization, typed errors, GraphQL mutation payload/mapping, the `fulfillment/select-shipping-option` server-function shell and MissingServer fallback over the explicit commerce checkout runtime API.
  - `storefront/src/transport.rs` owns shipping-selection update materialization via `build_shipping_selection_updates`; the owner GraphQL and native adapters consume those updates directly, and commerce no longer maps fulfillment selection updates inside its storefront raw adapter.
  - manifest-driven storefront composition now registers `rustok-fulfillment-storefront` in `checkout_shipping_handoff`; `FulfillmentView` is the zero-prop host entry adapter, reads the effective locale from `UiRouteContext.locale`, and resolves copy through the module-owned `en`/`ru` catalog declared by `[provides.storefront_ui.i18n]`.
- Last verified at (UTC): 2026-06-30T08:04:31Z
- Owner: `rustok-fulfillment` module team

## Scope of work

- keep `rustok-fulfillment` as the owner shipping-option/fulfillment boundary;
- synchronize shipping contracts, allowed profile bindings and local docs;
- do not mix the base shipping domain model with provider-specific delivery logic.

## Current state

- `shipping_options`, `fulfillments`, `FulfillmentModule` and `FulfillmentService` are already separated;
- typed `fulfillment_items` already lock the fulfillment composition over `order_line_item_id + quantity`;
- typed `fulfillment_items` already lock progress fields `shipped_quantity` / `delivered_quantity` for partial delivery path;
- first-class `allowed_shipping_profile_slugs` are already part of the live contract;
- deliverability orchestration with `delivery_groups[]`, `shipping_selections[]` and multi-fulfillment checkout is built by umbrella `rustok-commerce` over this boundary;
- admin/post-order create fulfillment path in `rustok-commerce` already uses typed `items[]` and validates order-line ownership + remaining quantity before calling `FulfillmentService`;
- item-level `ship` / `deliver` adjustments already work over typed fulfillment items and write language-agnostic audit trail in metadata of fulfillment/items; `delivered_note` is not duplicated in audit JSON;
- explicit `reopen` / `reship` recovery path already works over the same typed fulfillment boundary: delivered fulfillment can be returned to `shipped`, cancelled fulfillment can be returned to actionable state, and repeat shipment attempt is locked audit-safe without language-dependent metadata;
- admin/operator surface already uses typed lifecycle for shipping options; storefront handoff presentation, request normalization, selection-update materialization, GraphQL mutation execution, native server-function execution and transport fallback policy live in `rustok-fulfillment/storefront`, while commerce exposes only the shared checkout runtime API for cart update/reprice orchestration.

## Stages

### 1. Contract stability

- [x] lock shipping-option/fulfillment boundary;
- [x] embed first-class `allowed_shipping_profile_slugs`;
- [x] keep compatibility shim for single-group carts only as a transitional transport layer;
- [x] move shipping-option admin UI to module-owned package `rustok-fulfillment/admin`;
- [x] maintain sync between fulfillment runtime contract, commerce orchestration and module metadata for the current storefront selection slice;

### 2. Deliverability expansion

- [x] bring richer fulfillment-item model without blurring the boundary;
- [x] expand fulfillment-item model from the already live manual post-order create path to item-level delivery changes and adjustments over seller-aware grouping;
- [x] add explicit post-order recovery semantics `reopen` / `reship` over typed fulfillment-item progress and language-agnostic audit trail;
- [ ] cover mixed-cart and multi-fulfillment edge-cases with targeted tests;
- [x] maintain compatibility with payment/order orchestration and shipping-profile registry for seller-aware storefront selection UI;

### 2.5. Provider expansion

- [x] form provider SPI baseline before connecting external carrier integrations;
- [x] add static provider SPI contract matrix and tracking webhook ingress/replay contract;
- [x] lock external carrier registration contract without provider-specific carrier logic in the base fulfillment lifecycle contract.
- [x] add fulfillment-owned provider registry seam for host/carrier composition without lifecycle persistence in adapter layer.
- [x] add side-effect-free runtime-mode guardrails for capability checks and degraded-mode fallback mapping before external carrier adapter invocation.
- [x] lock no-compile live carrier adapter execution contract packet.
- [x] replace static/no-compile provider SPI evidence with live runtime contract execution against concrete external adapters.
- [x] add owner registry guarded async invocation seam for carrier adapter calls before production carrier wiring.

### 3. Operability

- [x] document new fulfillment guarantees simultaneously with changing runtime surface;
- [x] keep local docs and `README.md` synchronized for the storefront selection boundary;
- [x] update umbrella commerce docs when deliverability/provider scope changes.

## Verification

- `cargo xtask module validate fulfillment`
- `cargo xtask module test fulfillment`
- `node scripts/verify/verify-fulfillment-admin-boundary.mjs`
- `node scripts/verify/verify-fulfillment-storefront-boundary.mjs`
- targeted tests for shipping options, fulfillments, delivery groups and multi-fulfillment invariants

## Update rules

1. When changing fulfillment runtime contract, update this file first.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing deliverability/provider architecture, update umbrella docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
