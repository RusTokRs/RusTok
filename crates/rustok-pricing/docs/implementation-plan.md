# Implementation plan for `rustok-pricing`

Status: pricing boundary is defined as a separate module; the module holds the pricing runtime
baseline, module-owned admin UI already includes base-row, active `price_list` override,
rule and scope write paths, while the full promotions engine and remaining `pricing 2.0`
stay in the active backlog umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: storefront native Loco-free transport ownership
- Last checkpoint: Pricing storefront native pricing atlas endpoint `pricing/storefront-data` now builds `PricingService` from `HostRuntimeContext` DB/event-bus handles and builds `ChannelService` from the same neutral DB handle. The storefront package no longer depends on `loco-rs` or `rustok-outbox/loco-adapter`, while the build-profile-selected native + GraphQL selected path remains unchanged.
- Dependency evidence: pricing storefront locale matching uses `rustok_api::locale_tags_match`; no-feature/hydrate profiles no longer contain `rustok-core`; SSR profile no longer contains a package-local Loco dependency.
- Next step: Continue small FFA slices only where they reduce Leptos-owned presentation/state policy; do not change the build-profile-selected native/GraphQL transport contract.
- Open blockers: None.
- Hand-off notes for next agent: Continue small FFA/Loco-exit slices without touching the parallel UI/i18n library work unless the slice explicitly targets it.
- Last updated at (UTC): 2026-07-08T00:00:00Z

## FFA/FBA status

- FFA status: `in_progress`
- FBA status: `in_progress`
- FBA contract version: `pricing.read_projection.v1`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` and fixture-regression suite check `crates/rustok-pricing/contracts/evidence/pricing-runtime-contract-smoke.json`: shared read policy precedes owner `PricingService` invocation and typed error mapping, and fallback profiles/degraded modes do not diverge from registry. Status remains `in_progress` until live provider execution;
  - in-process implementation `PricingReadPort for PricingService` added in `src/ports.rs`: read paths require `PortContext::require_deadline_semantics`, price resolution calls owner `resolve_variant_price`, projection reads active price-list snapshot, and `CommerceError` maps to `PortError`;
  - umbrella facade `rustok_commerce::{services::pricing, PricingService}` and pricing DTO aliases under `rustok_commerce::services::*` are removed; consumers import `PricingService`, `PriceResolutionContext`, `ResolvedPrice` and pricing entities from `rustok-pricing` directly;
  - `src/ports.rs` now exports `PricingReadPort` and DTOs for product price resolution/price-list projection operations; machine-readable registry and verifier check port trait operations match FBA metadata;
  - FBA-provider metadata is open for `pricing read projection` via `crates/rustok-pricing/contracts/pricing-fba-registry.json`; status remains `in_progress` until contract tests/remote transport evidence appear that would allow promotion above embedded checkout compatibility;
  - registry now locks `contract_tests.status = planned_cases_locked`: for each port operation, an in-process/remote-adapter-placeholder case matrix is defined with baseline assertions (`typed_port_error_mapping`, `context_deadline_preserved`) with explicit deadline enforcement for read path and `write_idempotency_required` only on write operations; fallback smoke profile set; static evidence packet `crates/rustok-pricing/contracts/evidence/pricing-contract-test-static-matrix.json` is locked by `npm run verify:ecommerce:fba` (registry + evidence gates) and `npm run verify:ecommerce:fba-contract-evidence`; this closes metadata/evidence anti-drift for future contract tests, but does not promote status without runtime evidence;
  - storefront pricing route now uses framework-agnostic `storefront/src/core.rs` for summary/label/effective context formatting, query href building and shared `StorefrontPricingQuery`; Leptos `lib.rs` no longer owns this presentation/request policy;
  - storefront transport split into thin facade + explicit `native_server_adapter` and `graphql_adapter`, with fallback order (`native #[server]` first, GraphQL second) preserved; legacy `storefront/src/api.rs` removed, raw operations live in `storefront/src/transport/`, and `scripts/verify/verify-pricing-storefront-boundary.mjs` blocks legacy API return;
  - storefront native transport is Loco-free: `storefront/src/transport/native_server_adapter.rs` reads `HostRuntimeContext`, obtains `TransactionalEventBus` from the neutral typed host-handle snapshot, passes `runtime_ctx.db_clone()` into `PricingService`/`ChannelService`, and the storefront package has no `loco-rs` or `rustok-outbox/loco-adapter` dependency;
  - Leptos render/bind adapter extracted into `storefront/src/ui/leptos.rs`, and `storefront/src/lib.rs` became crate-level composition/re-export boundary;
  - targeted facade tests confirm both selected paths: native success does not call GraphQL, and GraphQL selected-path execution receives the original `StorefrontPricingQuery`;
  - request normalization/validation moved to `storefront/src/core.rs`, including typed `StorefrontPricingQueryError`; API layer converts core validation errors into the existing transport envelope without changing public behavior;
  - parity evidence: `cargo test -p rustok-pricing-storefront --lib` confirms existing transport validation tests, pure-core route/channel formatting tests, core request validation tests and selected-path transport facade tests without changing the native/GraphQL contract;
  - admin FFA slice added module-owned `admin/src/transport.rs` facade and explicit Leptos render adapter `admin/src/ui/leptos.rs`; `admin/src/lib.rs` now only wires modules and re-exports `PricingAdmin`, and Leptos adapter no longer calls raw `api::*` directly for covered flows; legacy `admin/src/api.rs` removed, raw native/GraphQL operations live in `admin/src/transport/native_server_adapter.rs`, and `scripts/verify/verify-pricing-admin-boundary.mjs` blocks legacy API return;
  - admin pricing presentation/request policy continues FFA decomposition into `admin/src/core/`: `presentation.rs` owns summary/labels/formatters, `routing.rs` — channel scope/query helpers, `requests.rs` — resolution context normalization and write draft builders; targeted pure-core tests cover pricing summary, resolution context normalization, channel-key policy and DTO builders;
  - admin write request construction for variant price, percentage discount and price-list rule/scope remains in core-owned draft builders; Leptos adapter uses explicit core imports instead of wildcard and does not construct covered write DTO inline;
  - admin GraphQL/native input sanitization for active price-list/product context (`currency_code`, UUID strings, channel slug, resolution quantity/context) moved from legacy `admin/src/api.rs` to `core/requests.rs`; transport adapter preserves the existing `ApiError`/`ServerFnError` envelope through adapter mapping;
  - admin detail header presentation is now assembled by `PricingProductDetailHeaderViewModel` in `admin/src/core/presentation.rs`: translation fallback, status badge/label, meta/seller/shipping/timestamp strings are no longer formatted inline in Leptos render path, and pure-core unit test locks fallback policy; latest admin variant-card slice added `PricingVariantCardViewModel` which assembles health label/badge, identity/profile lines, effective price line and price table outside Leptos adapter; latest admin product-list slice added `PricingProductListItemViewModel` which assembles row id/title, status label/badge, shipping-profile fallback, meta line and selected-row class policy outside Leptos adapter; latest admin editor routing slice added `legacy_channel_option_label` in `admin/src/core/routing.rs` so legacy channel option label/not-set fallback is no longer duplicated in Leptos variant price, discount and price-list rule editors; latest variant editor presentation slice added `format_variant_price_editor_title`, `format_variant_count_label` and `default_variant_price_editor_currency` to `admin/src/core/presentation.rs`, so editor title/count/default-currency policy no longer belongs to Leptos adapter.
- Last verified at (UTC): 2026-07-08T00:00:00Z
- Owner: `rustok-pricing` module team

## Scope of work

- maintain `rustok-pricing` as owner of pricing service boundary;
- synchronize pricing runtime contract, module-owned admin UI and local docs;
- do not mix pricing storage with product catalog, promotions or tax orchestration.

## Current state

- `PricingModule`, `PricingService` and pricing migrations are already defined;
- the module depends on `product` without creating a cycle with umbrella `rustok-commerce`;
- transport adapters are still published through the `rustok-commerce` facade;
- `rustok-pricing/admin` already publishes the pricing-owned admin route for price visibility,
  sale markers, currency coverage inspection, operator-side effective price context,
  active price list selector and write actions for base rows or active price-list
  overlays for variant prices, including quantity tiers and typed percentage-discount
  preview/apply by canonical base row or selected active `price_list` override; the
  selected active `price_list` rule editor is also now extracted there;
- `rustok-pricing/storefront` already publishes the pricing-owned storefront route for public
  pricing atlas, currency coverage, sale-marker visibility and active price list selector
  over existing effective context; storefront presentation policy
  for summary, health/option labels, effective context and query href is now extracted
  to framework-agnostic `storefront/src/core.rs`, shared fetch request also lives in
  `core`, transport orchestration extracted to `storefront/src/transport/`, and
  Leptos render/bind layer lives in `storefront/src/ui/leptos.rs`;
- storefront package remains a read-side surface, but admin package already
  uses `admin/src/transport.rs` facade over build-profile-selected native `#[server]` transport not only for read-side, but also for
  base-row writes, active `price_list` overrides, typed percentage adjustments and
  `price_list` rule/scope editing, keeping product GraphQL contract as fallback
  for reads; admin presentation/request policy for summary, status/price/channel
  labels, legacy channel option fallback, route href, detail-header view-model, resolution context normalization and write draft builders is extracted to Leptos-free
  `admin/src/core/` (`presentation`, `routing`, `requests`), so `admin/src/ui/leptos.rs` remains a render/bind adapter.

## Stages

### 1. Contract stability

- [x] lock pricing boundary as a separate module;
- [x] maintain `pricing -> product` dependency without umbrella cycle;
- [x] extract pricing admin UI into module-owned package `rustok-pricing/admin`;
- [x] extract pricing storefront UI into module-owned package `rustok-pricing/storefront`;
- [x] maintain sync between pricing runtime contract, admin UI, commerce transport
  and module metadata.

### 1.1. FFA storefront decomposition

- [x] extract pricing storefront presentation policy from Leptos component into
  framework-agnostic `storefront/src/core.rs`: summary, variant health, seller/channel
  labels, effective price/context formatting and route href builders;
- [x] add pure-core tests for query href/channel-scope formatting alongside existing
  transport validation suite;
- [x] introduce storefront `transport/` facade with explicit `native_server_adapter` and
  `graphql_adapter`, preserving the build-profile-selected native/GraphQL contract;
- [x] extract Leptos render/bind adapter into `storefront/src/ui/leptos.rs`, leaving
  crate root as composition/re-export boundary;
- [x] add targeted tests for `transport` facade: native-success path and GraphQL
  fallback path with preservation of original `StorefrontPricingQuery`;
- [x] move request normalization/validation from `api.rs` to `core`: UUID,
  currency, quantity, channel slug and resolution context sanitization with typed error;
- [x] remove legacy `storefront/src/api.rs`: native/GraphQL raw operations live in
  `storefront/src/transport/{native_server_adapter.rs,graphql_adapter.rs}`, and guardrail
  `scripts/verify/verify-pricing-storefront-boundary.mjs` prohibits legacy API return.

### 2. Pricing transport split

- [~] extract dedicated pricing read/write transport from umbrella `rustok-commerce`;
- [x] transition pricing admin UI from read-only product-backed transport to targeted
  base-price mutations and operator workflows;
- [~] cover transport parity, money semantics and compare-at invariants with targeted tests.

### 3. Pricing 2.0 rollout

- [~] transition from base prices to rule-driven price resolution;
- [x] introduce typed resolver foundation by `currency_code + optional region_id + optional quantity`
  with deterministic precedence for base prices;
- [x] activate explicit `price_list_id` overlay in resolver for active tenant-scoped
  price lists with base-price fallback;
- [x] add channel-aware foundation in resolver/read-side contract through
  host-provided `channel_id` / `channel_slug`, channel-scoped base rows and
  channel-filtered active price lists without ownership drift into `rustok-channel`;
- [x] extend the same channel-aware contract to module-owned admin authoring for
  variant price rows, typed discount preview/apply and active price-list scope without
  a separate seller/channel portal;
- [x] replace raw `channel_id/channel_slug` authoring inputs in pricing admin with
  a selector over `rustok-channel` read model with global fallback and legacy-scope
  compatibility option;
- [x] extend effective price context into module-owned storefront/admin read-side surfaces
  through build-profile-selected native `#[server]` transport with a GraphQL selected path;
- [x] align validation contract for `PriceResolutionContext` between runtime,
  dedicated GraphQL facade roots and native `#[server]` transport: `currency_code`
  must be a three-letter ASCII business code, `quantity < 1` is rejected,
  and `region_id`, `price_list_id` or `quantity` without `currency_code` are not
  silently ignored; malformed explicit `channel_id` is also rejected rather than
  falling back to host channel context;
- [x] extract the same validation step into pricing UI fetch wrappers before attempting
  native `#[server]` transport, so invalid input does not enter a meaningless
  GraphQL selected path and does not blur the transport contract;
- [x] add explicit channel selector in storefront/admin effective-context controls
  so channel-aware resolution can be switched without raw query editing and without
  reverting to package-local fallback chain;
- [x] switch admin active `price_list` selector to context-aware read path so the
  overlay list and rule editor recalculate based on explicitly selected `channel` rather than
  only bootstrap host context;
- [x] extend the same selector metadata contract to the GraphQL selected path for
  `rustok-pricing/admin` and `rustok-pricing/storefront` so the degraded path does not
  lose `available_channels` and channel-aware active `price_lists`;
- [x] switch the GraphQL selected-path detail contract to dedicated pricing-facing facade
  roots `adminPricingProduct` / `storefrontPricingProduct` so the degraded path
  preserves variant-level `effective_price` parity for explicit resolution context;
- [x] deliver active tenant-scoped price lists as pricing-owned read contract
  so admin/storefront routes can select overlays without raw UUID-only UX;
- [~] add tiers, adjustments and promotion-ready semantics;
- [~] cover deterministic price resolution and rounding with targeted tests.

What is additionally closed:

- module-owned `rustok-pricing/admin` now has targeted SSR tests for native
  `update-variant-price` transport path, including quantity-tier happy path, active
  `price_list_id` override happy path and permission gate;
- the same admin transport now also covers typed `preview_percentage_discount` /
  `apply_percentage_discount` path by canonical base-price row, including targeted SSR
  tests for happy path and permission gate; active `price_list` override adjustment path
  is now covered by the same transport parity layer;
- runtime tests already cover `set_price_tier` for quantity windows, invalid tier ranges
  and normalized `discount_percent` in `ResolvedPrice`, and admin/storefront surfaces
  already show sale math over typed read-side contract.
- targeted runtime/transport tests already cover strict resolution validation:
  service-level resolver, GraphQL roots `adminPricingProduct` / `storefrontPricingProduct`
  and native `#[server]` helpers in `rustok-pricing/admin` / `rustok-pricing/storefront`
  reject invalid `currency_code`, `quantity < 1`, malformed explicit `channel_id`
  and modifiers without currency.
- the same runtime now also covers channel-aware deterministic resolution:
  channel-scoped base row wins over global only when host channel matches,
  and active price list selector does not return channel-scoped list outside its scope.
- targeted runtime tests now also lock tie-break by `max_quantity`
  when `min_quantity` is the same, slug-only channel matching without `channel_id`
  and fractional `discount_percent` rounding for regular sale rows.
- service-level pricing tests now also lock channel-scope semantics for
  active `price_list`: inheritance in `set_price_list_tier_with_channel`,
  rejection of mismatched explicit scope and propagation of new scope to existing
  override rows through `set_price_list_scope`.
- the same service-level tests now also lock active time-window invariants:
  future `starts_at` and expired `ends_at` are rejected both in read-side resolution
  and write-side authoring / scope update paths, not just hidden from
  `list_active_price_lists`.
- service-level tests now also lock lifecycle typed percentage rule:
  removing rule metadata via `set_price_list_percentage_rule(..., None)` clears
  `rule_kind` / `adjustment_percent`, and the resolver with explicit `price_list_id`
  after that deterministically falls back to base row without stale discount state.
- promotion-ready preview path is now also locked by targeted tests: preview
  price-list percentage adjustment rejects future/expired `price_list` and
  channel-scope mismatch, not just `draft` status.
- apply path for price-list percentage adjustment is now also locked by targeted
  tests: future/expired `price_list` and channel-scope mismatch are rejected without
  side-effect writing a new override row and without mutating existing scoped override.
- admin SSR transport parity now also closes `price_list` rule/scope mutation
  lifecycle: clear rule metadata does not leave stale `rule_kind` / `adjustment_percent`,
  update rule path rejects inactive/draft, future/expired lists with the same contract, and
  scope clear returns active option and existing override rows to global boundary.
- pricing-focused GraphQL parity now also locks rule-driven effective
  price resolution without explicit override, explicit override precedence over rule,
  selector lifecycle after clear/scope update and channel-mismatch validation for
  `adminPricingProduct` / `storefrontPricingProduct`.
- GraphQL facade now already closes not only pricing-authoritative reads:
  admin write-side mutations `updateAdminPricingVariantPrice`,
  `previewAdminPricingVariantDiscount`, `applyAdminPricingVariantDiscount`,
  `updateAdminPricingPriceListRule` and `updateAdminPricingPriceListScope`
  also work over `PricingService`, preserving parallel transport contract
  alongside native `#[server]` path and active-option parity for rule/scope editing.
- compare-at invariants are now also locked wider than just service-level
  runtime: admin SSR transport rejects invalid `compare_at < amount` without mutating
  base row or existing `price_list` override, and GraphQL parity separately
  confirms that `compare_at == amount` does not leak into false sale state
  (`discount_percent = null`, `on_sale = false`).
- storage-level money sync is now also covered by targeted tests: `set_price` keeps
  decimal fields and legacy cents fields in sync even on fractional values,
  and clearing `compare_at` zeroes both decimal and legacy compare-at representation
  in service write path and admin native transport; admin `update-variant-price`
  separately locks the same decimal-to-cents sync for fractional operator-side inputs.
- bulk write path `set_prices` is now also locked separately: channel-scoped
  rows normalize `channel_slug`, synchronously write decimal + legacy cents
  representation, and a single invalid input rolls back the entire batch without partial update
  of already existing rows.
- event payload parity for pricing write paths is now also covered by service-level
  tests: `PriceUpdated.old_amount/new_amount` are published in rounded cents contract
  for both `set_price` and bulk `set_prices`, including cases of `update existing row`
  vs `insert new row`.
- the current broad verification baseline now also passes the full
  `graphql_runtime_parity_test`, not just the pricing-focused subset.
- legacy `apply_discount` no longer lives as a separate ad-hoc mutation: pricing runtime
  now holds typed `preview_percentage_discount` / `apply_percentage_discount` over
  canonical base-price row, and the old helper remains a compatibility wrapper.
- promotion-ready semantics have also advanced: active `price_list` can now
  hold a typed percentage rule, the resolver can fall back to base row through this rule,
  and module-owned admin transport already provides a first-class write path for rule authoring.

### 4. Operability

- [x] document new pricing guarantees concurrently with runtime surface changes;
- [x] keep local docs and `README.md` synchronized;
- [x] update umbrella commerce docs when pricing/promotion scope changes.

## Verification

- `cargo xtask module validate pricing`
- `cargo xtask module test pricing`
- `npm run verify:pricing:admin-boundary`
- `npm run test:verify:pricing:admin-boundary`
- `npm run verify:pricing:storefront-boundary`
- `npm run test:verify:pricing:storefront-boundary`
- targeted tests for price resolution, pricing transport and money semantics
- current broad verification baseline for this slice:
  `cargo test -p rustok-commerce --test pricing_service_test`,
  `cargo test -p rustok-commerce --test graphql_runtime_parity_test`,
  and SSR/lib sweeps for `rustok-pricing-admin` / `rustok-pricing-storefront`

## Update rules

1. When changing pricing runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md`, `admin/README.md`
   and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing pricing/promotion boundary, update umbrella commerce docs.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
