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

## FFA/FBA boundary

- FFA status: `phase_b_ready`
- FBA status: `in_progress`
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

## Open results

1. **Keep cart-owned UI within the cart boundary.** Do not add quantity
   increase, add-to-cart, or checkout controls to the package until their
   cross-domain validation and orchestration contract is explicitly owned and
   composed.
   **Depends on:** commerce-owned checkout and validation contracts.
   **Done when:** any new surface consumes an owner-owned public contract with
   no cart business logic or presentation duplicated in the umbrella.

2. **Execute the checkout provider contract against a live adapter.**
   Turn the locked in-process/remote case matrix into provider execution and
   fallback evidence before considering FBA promotion.
   **Depends on:** a commerce consumer and remote-adapter test environment.
   **Done when:** deadline, typed-error, degraded-mode, and snapshot parity are
   proven for the published `cart.checkout.v2` contract, including write
   idempotency and lifecycle recovery.

3. **Document operational changes with checkout changes.** Add diagnostics only
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
