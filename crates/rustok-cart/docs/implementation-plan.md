# `rustok-cart` — Implementation Plan

Status: cart boundary extracted; module remains owner of cart state and storefront
context snapshot, while orchestration over checkout lives in umbrella `rustok-commerce`.

## Execution checkpoint

- Current phase: phase_b_ready
- Last checkpoint: Cart storefront read ownership now includes full shipping option summaries and storefront repricing in `cart/storefront-data`; `rustok-commerce-storefront` consumes cart workspace data through `rustok-cart-storefront::transport::fetch_cart` instead of keeping its own checkout cart GraphQL/native read mapping.
- Next step: Continue only with owner-module checkout handoff slices that remove real umbrella presentation/read leakage, or return to parity/evidence hardening for SSR native path, GraphQL selected path, headless cart mutation contracts and DOM evidence.
- Open blockers: None.
- Hand-off notes for next agent: Update this block and the central readiness board after each increment.
- Last updated at (UTC): 2026-06-30T08:39:56Z


## FFA/FBA status

- FFA status: `phase_b_ready`
- FBA status: `in_progress`
- FBA verification: `scripts/verify/verify-ecommerce-fba-registries.mjs`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile FBA gate `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` and fixture-regression suite check `crates/rustok-cart/contracts/evidence/cart-runtime-contract-smoke.json`: shared read policy executed before owner `CartService`, typed error mapping and fallback/degraded registry parity protected from drift; status remains `in_progress` until live provider execution;
  - FBA provider registry `crates/rustok-cart/contracts/cart-fba-registry.json`, static contract evidence `crates/rustok-cart/contracts/evidence/cart-contract-test-static-matrix.json` and neutral `CartSnapshotReadPort`/`cart.checkout_snapshot.v1` are locked for commerce checkout snapshot consumers; runtime contract execution/fallback smoke remain pending before `boundary_ready`;
  - umbrella facade `rustok_commerce::{services::cart, CartService}` is removed; commerce REST/GraphQL/storefront/test consumers import `CartService` from `rustok-cart` directly, so cart owner service is no longer masked by the ecommerce umbrella;
  - cart delivery-group keys and shipping-selection matching no longer read `seller_scope` as a fallback identity; canonical selection is `shipping_profile_slug + seller_id`, while no-seller carts use only the shipping profile, and the storefront boundary guardrail now blocks `sellerScope`/`seller_scope` from returning to cart read queries or storefront model DTOs;
  - `rustok-cart-storefront` now exposes cart-owned storefront DTOs/transport publicly for aggregate consumers, `StorefrontCartDeliveryGroup` includes full `StorefrontCartShippingOption` summaries (`id/name/currency/amount/provider/active`), GraphQL/native cart read mapping fills the same DTO shape, and the native `cart/storefront-data` read path preserves checkout repricing before mapping the cart DTO;
  - module plan synchronized with the central FFA/FBA readiness board; UI surface already published and maintained in migration/backlog rhythm;
  - storefront slice extracts `core/` helpers for route/input normalization, UUID validation, adjustment metadata mapping, channel-slug normalization, decrement policy, typed fetch/decrement/remove request objects, GraphQL decrement command dispatch, stable serializable transport fallback error evidence, DOM evidence adapter, display/view-model mapping and checkout handoff summary view-model consumed by commerce orchestration;
  - `ui/leptos::CartView` now calls the thin `transport` facade through core-owned request objects, receives prepared view-model values from `core/` and renders error evidence attributes `data-cart-transport-failed-path`, `data-cart-transport-fallback-attempted`, `data-cart-transport-native-error`, `data-cart-transport-graphql-error`; transport facade preserves validation errors without GraphQL retry and returns `CartTransportError` with stable `failed_path` (`native_server`/`graphql`), `fallback_attempted`, `native_error` and `graphql_error`, native `#[server]` + GraphQL adapter calls live inside `storefront/src/transport/`, legacy `storefront/src/api.rs` removed and banned by guardrail, while the API layer no longer recalculates GraphQL decrement policy;
  - Cart-owned checkout handoff decision: cart status/handoff presentation belongs to `rustok-cart/storefront`; umbrella `rustok-commerce` may pass checkout context but must consume the cart-owned component rather than owning cart presentation;
  - further promotion to `parity_verified` is performed only together with full parity evidence and updating local+central docs.
- Last verified at (UTC): 2026-06-30T08:39:56Z
- Owner: `rustok-cart` module team

## Scope of work

- maintain `rustok-cart` as owner of cart lifecycle and line-item state;
- synchronize cart snapshot contract, runtime dependencies, storefront UI ownership and local docs;
- prevent cart domain logic from returning to the umbrella or host layer.

## Current state

- `carts` and `cart_line_items` are already module-owned;
- `cart_adjustments` are already module-owned and capture language-neutral promotion/discount snapshot without display labels;
- tax runtime is no longer directly wired into cart service: `rustok-cart` calls `rustok-tax`,
  and `cart_tax_lines` now carry typed `provider_id`;
- cart lifecycle and persisted storefront context snapshot are already built into the base contract;
- cart write-side now supports batch repricing of line items when context/quantity changes,
  so that unit_price remains consistent with the pricing resolver;
- transport adapters are still published through the `rustok-commerce` facade, without dependency cycles;
- storefront cart inspection, safe decrement/remove write-side and seller-aware delivery-group snapshot already moved to `rustok-cart/storefront`;
- storefront package continued FFA decomposition: pure cart UI policy, typed request construction, GraphQL command dispatch, stable transport error evidence, Leptos DOM evidence adapter and display/view-model mapping organized under `storefront/src/core/{identifiers,policy,request,view_model,error}.rs`, Leptos layer lives in `storefront/src/ui/leptos.rs` and uses the facade in `storefront/src/transport/mod.rs`, build-profile-selected native/GraphQL orchestration lives in `storefront/src/transport/`, legacy `storefront/src/api.rs` removed, and fast guardrail `scripts/verify/verify-cart-storefront-boundary.mjs` locks the boundary and docs sync;
- channel/context/deliverability orchestration over cart is still performed at the umbrella-module level.
- targeted tests now explicitly verify that cart mutation paths `set_adjustments` and typed promotion apply-path are rejected when `checking_out`, so no concurrent pricing snapshot mutation occurs during checkout.

## Stages

### 1. Contract stability

- [x] lock cart lifecycle and storefront context snapshot;
- [x] maintain line-item CRUD and totals inside `rustok-cart`;
- [x] add typed cart adjustment snapshot with `subtotal_amount`, `adjustment_total` and net `total_amount`;
- [x] maintain sync between cart runtime contract, commerce orchestration, storefront route ownership and module metadata.

### 2. Storefront ownership

- [x] move storefront cart inspection to `rustok-cart/storefront`;
- [x] use native Leptos `#[server]` functions as default internal data layer;
- [x] keep GraphQL storefront contract as fallback;
- [x] move safe cart-owned line-item decrement/remove mutations out of the aggregate storefront surface;
- [x] start FFA separation of storefront package into `core/` policy/request/view-model helpers, `transport` facade and `ui/leptos` render layer;
- [ ] do not mix cart-owned UI with quantity increase, add-to-cart and checkout orchestration while those write-paths require cross-domain validation.

### 3. Checkout hardening

- [x] keep `checking_out`/recovery semantics compatible with payment/order orchestration;
- [x] cover stale snapshot, shipping selection and multi-group edge-cases with targeted tests;
- [x] evolve cart state only through explicit snapshot/versioning semantics.

### 4. Operability

- [ ] document new cart guarantees simultaneously with checkout flow changes;
- [x] keep local docs and `README.md` synchronized with the storefront contract;
- [ ] expand diagnostics only under real runtime pressure.

## Verification

- `cargo xtask module validate cart`
- `cargo xtask module test cart`
- targeted tests for cart lifecycle, line items, typed adjustments, snapshot context and checkout-preflight semantics

## Update rules

1. When changing cart runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md`, `docs/README.md` and `storefront/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing checkout orchestration expectations, update umbrella docs in `rustok-commerce`.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and relevance of `README.md` and local docs.
- [ ] Lock/update verification gates for the current module state.
