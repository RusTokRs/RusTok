# Cart storefront native error safety

Status: `source_ready_unvalidated`

The mounted SSR Cart storefront transport uses a dedicated safe adapter. The existing
client/hydrate server-function contract remains in `native_server_adapter.rs`; SSR selects
`native_server_adapter_ssr.rs` through the transport module.

## Closed source leak

The previous mounted native path exposed technical text through direct `ServerFnError`
construction for:

- tenant and optional-auth context extraction;
- missing host `TransactionalEventBus` composition;
- cart and line-item parsing failures;
- unexpected customer lookup failures;
- Cart owner storage, validation, transition, tax-boundary, repricing, decrement, and
  removal failures.

The safe adapter now keeps original causes in structured SSR diagnostics and returns
static public envelopes. Pricing remains behind `PortError`; its already-sanitized owner
message is preserved while the complete typed error is logged internally.

## Preserved behavior

- The three endpoint names and request/response DTOs are unchanged.
- Empty cart selection still returns an empty storefront-cart workspace.
- Missing carts on the read endpoint still return `cart: null`.
- Customer ownership and authentication checks are unchanged.
- Repricing still occurs before the cart DTO is returned.
- Decrement still removes a line item at quantity one and otherwise reprices the next
  quantity.
- Explicit native/GraphQL transport selection is unchanged.

## Evidence boundary

This slice is source-only. It does not claim compilation, server-function registration,
transport parity, runtime logging, or error-envelope execution evidence.

Suggested checks:

```bash
node scripts/verify/verify-cart-storefront-native-error-safety.mjs
node scripts/verify/verify-cart-storefront-boundary.mjs
cargo check -p rustok-cart-storefront --all-features
```
