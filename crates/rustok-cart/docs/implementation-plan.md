# Implementation plan for `rustok-cart`

## Current state

`rustok-cart` owns cart state, line items, pricing and tax snapshots, cart
lifecycle, and the persisted storefront context. Its storefront package owns
cart inspection and safe decrement/remove mutations. `rustok-commerce` keeps
cross-domain checkout, channel, and deliverability orchestration; it must not
recover cart presentation or a duplicate cart service.

Native storefront server functions use `HostRuntimeContext` and the shared
transactional event bus. The owner package keeps GraphQL as the selected
fallback path, returns full shipping-option summaries, and reprices before
mapping the storefront read model. Delivery grouping uses canonical
`shipping_profile_slug + seller_id`; legacy `seller_scope` is not a fallback
identity.

The Axum guest-cart capability adapter is owner-owned in
`rustok_cart::guest_access_http`; hosts compose it around REST/GraphQL requests
without reimplementing token parsing, cookie emission, or task-local scope.

## FFA/FBA boundary

- FFA status: `phase_b_ready`
- FBA status: `boundary_ready` — the in-process cart checkout provider is
  exercised by compiled commerce checkout consumer tests for both successful
  completion and inventory-preflight release paths. Remote/fallback,
  context-update, and recovery execution remain open.
- Structural shape: `core_transport_ui`
- FBA provider contract: `CartCheckoutPort` / `cart.checkout.v2`, covering the
  checkout snapshot plus context update and checkout lifecycle writes.
  in `crates/rustok-cart/contracts/cart-fba-registry.json`.
- Static and no-compile runtime evidence:
  `crates/rustok-cart/contracts/evidence/cart-contract-test-static-matrix.json`
  and `crates/rustok-cart/contracts/evidence/cart-runtime-contract-smoke.json`.
- `scripts/verify/verify-cart-storefront-boundary.mjs` locks the storefront
  core/transport/UI split, native host runtime, GraphQL fallback, and removal
  of the legacy API layer.
- Storefront repricing calls the pricing-owned `PricingReadPort` with a
  variant-first request and full resolved-price projection; it no longer calls
  `PricingService::resolve_variant_price` directly.
- The compiled commerce checkout channel-inventory regression executes the
  in-process cart checkout provider before product and inventory preflight.
  It is bounded provider-consumer evidence; lifecycle recovery and fallback
  execution remain open.
- Compiled provider-consumer evidence:
  `basic::complete_checkout_builds_order_payment_and_fulfillment_flow` proves
  snapshot/begin/complete lifecycle execution, while
  `validation::complete_checkout_rejects_line_item_without_channel_visible_inventory`
  proves snapshot/begin/release execution. Both run from
  `checkout_service_test` against the in-process provider.
- Storefront payment-collection reads use the owner-managed
  `CartCheckoutPort` factory rather than constructing `CartService` directly.
- Storefront REST and GraphQL cart reads and mutations consume
  `CartStorefrontPort`; the port preserves tenant, actor, channel, locale,
  deadline, and write-idempotency context at the owner boundary.
- Admin cart-promotion preview and application consume `CartPromotionPort`,
  with scope validation and owner-side typed error mapping.

## Open results

1. **Keep cart-owned UI within the cart boundary.** Do not add quantity
   increase, add-to-cart, or checkout controls to the package until their
   cross-domain validation and orchestration contract is explicitly owned and
   composed.
   **Depends on:** commerce-owned checkout and validation contracts.
   **Done when:** any new surface consumes an owner-owned public contract with
   no cart business logic or presentation duplicated in the umbrella.

2. **Prove checkout provider fallback and recovery behavior against a live adapter.**
   Turn the locked in-process/remote case matrix into provider execution and
   fallback evidence before considering FBA promotion.
   **Depends on:** a commerce consumer and remote-adapter test environment.
   **Done when:** deadline, typed-error, degraded-mode, and snapshot parity are
   proven for the published `cart.checkout.v2` contract, including write
   idempotency, context update, lifecycle recovery, and degraded behavior.

3. **Prove cart ports through live provider-consumer execution.**
   Commerce production adapters use `CartCheckoutPort`, `CartStorefrontPort`,
   and `CartPromotionPort`; static evidence confirms no direct
   `CartService` construction outside owner-side composition.
   **Depends on:** compiled or live provider-consumer execution.
   **Done when:** transport execution covers checkout, storefront writes,
   promotion application, fallback, and recovery behavior.

4. **Document operational changes with checkout changes.** Add diagnostics only
   where runtime pressure identifies a concrete cart or snapshot failure mode,
   and update module and commerce documentation in the same change.
   **Depends on:** observed runtime signals or a changed checkout contract.
   **Done when:** the runbook, metrics, and recovery guidance match the actual
   owner and orchestration boundaries.

## Verification

- `npm run verify:cart:storefront-boundary`
- `npm run verify:ecommerce:fba`
- `cargo xtask module validate cart`
- `cargo xtask module test cart`
- Targeted cart lifecycle, snapshot, repricing, shipping-selection, and
  checkout-preflight tests.

## Change rules

1. Keep cart persistence and storefront presentation in this module.
2. Update local docs, `rustok-module.toml`, and the umbrella commerce plan with
   a cross-module checkout contract change.
3. Update this status block and `docs/modules/registry.md` with any FFA/FBA
   boundary change.
