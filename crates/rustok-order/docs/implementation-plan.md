# Implementation plan for `rustok-order`

## Current state

`rustok-order` owns order lifecycle, snapshots, adjustments, tax lines,
transactional outbox events, returns storage, and the module-owned admin and
storefront packages. The server composes owner-owned dashboard analytics only;
it does not query order events directly. `rustok-commerce` provides checkout
orchestration, not an order-service facade or duplicate order transport.

The post-order foundation includes returns and item validation plus a preview /
apply / cancel order-change skeleton. It deliberately does not perform payment
or fulfillment side effects. Checkout completion is owner-owned through
`CheckoutCompletionPort`; the public GraphQL and native storefront paths use
the same typed request/result contract.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `order.checkout_completion.v1` in
  `crates/rustok-order/contracts/order-fba-registry.json`.
- Static contract evidence:
  `crates/rustok-order/contracts/evidence/order-contract-test-static-matrix.json`.
- `scripts/verify/verify-order-admin-boundary.mjs`,
  `scripts/verify/verify-order-storefront-boundary.mjs`, and
  `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` lock the
  module/commerce UI and transport ownership split.

## Open results

1. **Complete the post-order domain layer.** Evolve returns into explicit
   refund, exchange, claim, and order-change resolutions with owner-controlled
   lifecycle transitions and idempotent integration boundaries; do not move
   payment or fulfillment state transitions into this module.
   **Depends on:** published payment and fulfillment orchestration contracts.
   **Done when:** each resolution has typed references, failure semantics,
   outbox behavior, and targeted lifecycle tests.

2. **Prove checkout transport parity beyond the embedded owner path.** Keep
   GraphQL, native server-function, and remote-adapter fallback behavior aligned
   for completion, result, and status reads.
   **Depends on:** the commerce checkout runtime and a remote adapter test
   environment.
   **Done when:** the contract-test matrix has executable remote evidence and
   fallback behavior supports a justified status promotion.

3. **Keep order and commerce documentation synchronized.** Update local
   README/admin docs, manifest metadata, central registry, and the umbrella
   commerce plan whenever order lifecycle, checkout snapshots, or post-order
   ownership changes.
   **Depends on:** the change-owning contract.
   **Done when:** no owner-specific server GraphQL artifact, duplicate DTO, or
   stale cross-module responsibility remains.

## Verification

- `npm run verify:ecommerce:fba`
- `npm run verify:order:admin-boundary`
- `npm run verify:order:storefront-boundary`
- `npm run verify:commerce:storefront-transport-handoff`
- `cargo xtask module validate order`
- `cargo xtask module test order`
- Targeted order lifecycle, snapshot, outbox, and post-order tests.

## Change rules

1. Keep order writes and snapshots within this module; use public contracts for
   payment, fulfillment, and commerce orchestration.
2. Update local and umbrella commerce documentation in the same change as a
   cross-module order contract.
3. Update this status block and `docs/modules/registry.md` with any UI or FBA
   boundary change.
