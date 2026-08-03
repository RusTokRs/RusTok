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

## Pricing diagnostic boundary

Pricing failures are recorded by concrete error type together with the Pricing owner
operation, Cart storefront consumer, optional correlation id, bounded request and identity
facts, owner code, owner kind, owner retryability, and native boundary. Request tenant,
resolved tenant, cart, and line-item UUIDs are represented only by non-nil facts. Channel id,
channel slug, and locale are represented only by presence, non-nil, and length facts; their
complete values are not logged.

The complete `PortError` payload is not recorded. Existing pricing behavior remains
unchanged:

- unavailable, timeout, and invariant-violation kinds use `tracing::error!`;
- all other owner kinds use `tracing::warn!`;
- `owner_code`, `owner_kind`, and `owner_retryable` remain in both diagnostic paths;
- the domain-sanitized owner message continues through `ServerFnError::new(error.message)`.

## Existing mounted safety contract

The mounted adapter continues to provide static public envelopes for:

- missing host `TransactionalEventBus` composition;
- cart and line-item parsing failures;
- Cart owner storage, validation, transition, tax-boundary, repricing, decrement, and
  removal failures.

Missing-variant identifiers remain a separate open cleanup slice. This bounded change does
not modify its raw tenant, cart, or line-item UUID diagnostics.

## Preserved behavior

- The three endpoint names and request/response DTOs are unchanged.
- Empty cart selection still returns an empty storefront-cart workspace.
- Missing carts on the read endpoint still return `cart: null`.
- All five cart and line-item parsing operations, codes, warning severity, and public messages
  are unchanged.
- Customer lookup, ownership, authentication checks, and not-found handling are unchanged.
- Cart owner variant classification, public messages, public codes, retryability, and
  technical/rejection severity split are unchanged.
- Pricing owner classification, owner metadata, sanitized public message forwarding, and
  technical/rejection severity split are unchanged.
- Repricing still occurs before the cart DTO is returned.
- Decrement still removes a line item at quantity one and otherwise reprices the next
  quantity.
- Explicit native/GraphQL transport selection is unchanged.
- Missing-variant mapper behavior is unchanged in this input-only slice.

## Static evidence

`scripts/verify/verify-cart-storefront-native-error-safety.mjs` requires type-only framework,
Cart input, customer, Cart owner, and pricing diagnostics; rejects every complete error payload
in the mounted adapter; preserves all five parser operations and their codes, customer
not-found handling, every Cart owner public mapping, Pricing owner metadata, both severity
paths, all three endpoints, and shared DTO mapping; and leaves execution claims open.

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
