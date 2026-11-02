# Cart storefront native error safety

Status: `source_ready_unvalidated`

The mounted SSR Cart storefront transport uses a dedicated safe adapter. The existing
client/hydrate server-function contract remains in `native_server_adapter.rs`; SSR selects
`native_server_adapter_ssr.rs` through the transport module.

## Framework context diagnostic boundary

Tenant and optional-auth extraction both delegate to one shared framework-context mapper.
Framework context extraction errors are recorded by Rust type only together with the Cart
storefront owner, exact extraction operation, stable internal code, and native boundary.
The mapper and its tenant/auth wrappers do not require a `Debug` implementation and do not
log the complete framework error payload.

The public messages remain static:

- `Storefront tenant context is temporarily unavailable`;
- `Storefront authentication context is temporarily unavailable`.

## Cart input diagnostic boundary

Cart input failures are recorded by concrete error type together with the Cart storefront
owner, caller-selected parser operation, stable internal code, and native boundary. The
complete `CartCoreError` payload is not recorded.

The existing warning severity and public forwarding remain unchanged:

- input rejections continue to use `tracing::warn!`;
- cart-id parsing continues to use `cart.storefront_cart_id_invalid` and
  `Invalid cart selection`;
- line-item parsing continues to use `cart.storefront_line_item_id_invalid` and
  `Invalid cart line item selection`;
- `ServerFnError::new(public_message)` remains the final public envelope.

The storefront read, decrement, and remove flows retain their existing five parser operations.

## Customer diagnostic boundary

Customer lookup failures are recorded by concrete error type together with the Customer
owner operation, Cart storefront consumer, optional correlation id, and bounded context
facts. Tenant and user UUIDs are represented only by non-nil facts. Request tenant,
channel id, channel slug, and locale are represented only by presence, non-nil, and length
facts; their complete values are not logged.

The complete `CustomerError` payload is not recorded. The public envelope remains static:

- `Customer information is temporarily unavailable`.

`CustomerByUserNotFound` continues to map to `Ok(None)` before the diagnostic mapper, so the
existing anonymous/no-customer behavior is unchanged.

## Guarded owner and shared quantity boundary (2026-10-30)

Native read/decrement/remove now use the same guarded Cart provider as GraphQL/REST. Customer
ownership is still checked from the authenticated customer; the owner validates guest capability
before sanitizing metadata. The public projection is not re-authorized as a guest secret.

Decrement reads once, derives its target from that observation, and invokes the shared Commerce
quantity workflow with selected Product/Pricing and the explicit Inventory owner. A decrement to
removal retains the original Cart pricing observation, so a raced quantity change cannot delete
a newer line. Explicit remove also carries the original observation. No raw CartService mutation,
stored-price scaling, Product-stock fallback or duplicate native price/discount math remains.

Owner failures use the common `buyer_native::port_error`: diagnostics retain kind, exact retry
advice and code length, never complete owner payload/code/message. Technical unavailable/timeout/
invariant failures log at error; ordinary refusals log at warn. The public message is always:

`Cart operation could not be confirmed. Inspect the cart before trying again.`

The native endpoint names, input shapes, selected-transport policy and public cart DTO are
unchanged. The previous local Cart/Pricing error mappers were removed after migrating all their
live callers; their per-kind public text is no longer the native error contract. The shared
actual request context retains tenant/actor agreement, correlation, locale and claims.

## Verification and limits

- `node scripts/verify/verify-cart-storefront-native-error-safety.mjs`
- `node scripts/verify/verify-cart-priced-write-context.mjs`

These are source-only guards, not transport/test execution. New Rust and MJS regression sources
remain unrun. The guard is not authorization, a monotonic Cart revision, stock/price hold,
receipt, exactly-once execution or a new retry policy. Existing per-dispatch native keys do not
provide durable replay. Remote providers, SQL/locking, mounted/native/browser behavior, session
revocation fencing, context-patch-plus-repricing atomicity and downstream event effects remain
unverified/open in the [owner plan](implementation-plan.md).
