# Cart promotion owner failure context

Status: **source-ready / unvalidated**

## Scope

This source slice closes the remaining structured-context gap in the canonical
`CartPromotionPort` owner-service failure mapper in
`crates/rustok-cart/src/promotion_guard.rs`.

The earlier promotion hardening already established stable public envelopes and
owner/correlation/channel diagnostics. The immediately preceding admission slice
completed policy and write-semantics rejection diagnostics. This slice is limited
to failures returned by the cart owner service after preview or apply admission.

## Delivered source contract

Both canonical service paths remain unchanged:

- `read_cart_promotion_preview` routes all preview variants through `CartService`;
- `apply_cart_promotion` routes all apply variants through `CartService`;
- both continue to call `cart_promotion_error` on owner-service failure.

The mapper now constructs the same public `PortError` first, then emits one
structured event containing:

- the original internal `CartError`;
- owner `rustok_cart.promotion`;
- boundary `cart_promotion_owner_service`;
- exact preview or apply owner operation;
- correlation id and tenant id;
- actor, channel, and locale;
- causation id and traceparent;
- idempotency key and deadline;
- the owner-derived stable code;
- the actual mapped public code, `PortErrorKind`, and retryability.

Unavailable, timeout, and invariant failures use error severity. Validation,
not-found, conflict, forbidden, and other ordinary rejections use warning
severity. After diagnostics, the already-constructed public error is returned
unchanged.

## Preserved behavior

This slice does not change:

- preview or apply request DTOs;
- promotion scope/kind routing;
- cart service methods or promotion calculations;
- policy admission, target validation, or tenant parsing;
- tax-boundary propagation;
- validation, not-found, conflict, storage, or tax public codes/messages;
- retryability inherited from tax-boundary errors;
- the legacy compatibility provider in `crates/rustok-cart/src/ports.rs`;
- FBA or FFA status.

The public mapping remains:

- validation -> `cart.promotion_validation`;
- cart not found -> `cart.cart_not_found`;
- line item not found -> `cart.line_item_not_found`;
- invalid transition -> `cart.promotion_state_conflict`;
- database failure -> `cart.database_unavailable`;
- tax boundary -> the owner-provided typed kind, code, and retryability with the
  existing static public message.

## Static evidence

`scripts/verify/verify-cart-promotion-port-error-safety.mjs` now additionally
guards:

- exactly two preview/apply owner mapper callsites;
- public mapping before owner diagnostics;
- full available `PortContext` fields on owner-service failures;
- raw `CartError` plus owner and mapped public codes;
- mapped `PortErrorKind`, retryability, and explicit boundary identity;
- typed severity classification;
- return of the same mapped `PortError` after diagnostics;
- unchanged public constructors/messages and prior admission-context guards.

## Remaining gaps

The master ecommerce correlation-safe mapper task remains open for:

- remaining promotion consumer and transport adapters;
- mounted checkout compensation payment/order and inventory/cart context retention;
- remaining order, payment, fulfillment, inventory, customer, and tax consumers;
- non-`PortError` public envelopes;
- compile, runtime, remote-port, and cross-transport evidence.

No architecture status is promoted by source-only evidence.

## Suggested maintainer checks

These commands were intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-cart-promotion-port-error-safety.mjs
node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs
cargo check -p rustok-cart --all-features
```
