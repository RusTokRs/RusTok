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

## Cart owner diagnostic boundary

Cart owner failures are recorded by concrete error type together with the owner operation,
Cart storefront consumer, optional correlation id, stable public code, retryability, and
bounded context facts. Request tenant and resolved tenant UUIDs are represented only by
non-nil facts. Cart and line-item UUIDs are represented only by presence and non-nil facts.
Channel id, channel slug, and locale are represented only by presence, non-nil, and length
facts; their complete values are not logged.

The complete `CartError` payload is not recorded. Existing owner classification remains
unchanged:

- database failures and unavailable, timeout, or invariant tax-boundary failures use
  `tracing::error!`;
- validation, not-found, conflict, forbidden, and other domain rejections use
  `tracing::warn!`;
- all existing public message, public code, and retryability mappings are preserved;
- `ServerFnError::new(public_message)` remains the final public envelope.

## Existing mounted safety contract

The mounted adapter continues to provide static public envelopes for:

- missing host `TransactionalEventBus` composition;
- cart and line-item parsing failures;
- Cart owner storage, validation, transition, tax-boundary, repricing, decrement, and
  removal failures.

Pricing remains behind `PortError`; its already-sanitized owner message is preserved.
This bounded slice does not claim that the remaining pricing or identifier diagnostics are
correlation-safe. Pricing and identifier diagnostics remain separate open cleanup slices.

## Preserved behavior

- The three endpoint names and request/response DTOs are unchanged.
- Empty cart selection still returns an empty storefront-cart workspace.
- Missing carts on the read endpoint still return `cart: null`.
- Customer lookup, ownership, authentication checks, and not-found handling are unchanged.
- Cart owner variant classification, public messages, public codes, retryability, and
  technical/rejection severity split are unchanged.
- Repricing still occurs before the cart DTO is returned.
- Decrement still removes a line item at quantity one and otherwise reprices the next
  quantity.
- Explicit native/GraphQL transport selection is unchanged.
- Cart input, pricing, and missing-variant mapper behavior is unchanged in this Cart-owner-only
  slice.

## Static evidence

`scripts/verify/verify-cart-storefront-native-error-safety.mjs` requires the type-only
framework-context, customer, and Cart owner diagnostics; rejects complete Cart causes and raw
Cart identity/context fields; preserves customer not-found handling, every Cart owner public
mapping, both severity paths, all three endpoints, and shared DTO mapping; and leaves execution
claims open.

The source evidence remains in
`crates/rustok-cart/contracts/evidence/storefront-native-error-safety-source.json`.

## Evidence boundary

This slice is source-only. It does not claim compilation, server-function registration,
transport parity, runtime logging, or error-envelope execution evidence. Ecommerce FFA/FBA
and the broad ecommerce correlation-safe mapper cleanup are not promoted by this change.

Suggested checks:

```bash
node scripts/verify/verify-cart-storefront-native-error-safety.mjs
node scripts/verify/verify-cart-storefront-boundary.mjs
cargo check -p rustok-cart-storefront --all-features
```

These commands were intentionally not run by the implementation agent.
