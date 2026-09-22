# Cart storefront native client error safety

Status: **source-ready / unvalidated**

## Scope

This slice closes the final native client transport escape for the Cart storefront
operations:

- `fetch_cart`;
- `decrement_line_item`;
- `remove_line_item`.

The mounted SSR adapter already returns static public envelopes and keeps owner and
framework causes in structured server diagnostics. The hydrate/client compatibility
adapter still exposes `ApiError::ServerFn(String)`, and `execute_selected_transport`
records the error `Display` inside the public `UiTransportError`. An unexpected
framework, serialization, or server-function string could therefore become visible
after leaving the native adapter.

## Client boundary

Each native closure now creates `NativeClientErrorContext` before calling the
unchanged native adapter. The final native `ApiError` is mapped before transport
aggregation.

`ApiError::Validation` is preserved unchanged because it carries the existing
Cart core validation contract. Technical native variants fail closed to:

`Cart storefront request could not be completed`

The original technical error is retained only in structured diagnostics with:

- owner and owner operation;
- per-call correlation ID;
- stable code and boundary;
- selected cart, locale, cart, and line-item presence/character lengths.

The diagnostic event does not contain cart IDs, line-item IDs, locale values, cart
contents, pricing data, customer data, or other request payload values.

## Preserved behavior

- Explicit native/GraphQL transport selection is unchanged.
- No fallback is introduced.
- The three GraphQL call contexts and error mappings are unchanged.
- `CartTransportError` remains the `UiTransportError` alias.
- Request and response DTOs are unchanged.
- The default and SSR native adapters and mounted server functions are unchanged.
- Existing validation and transport-envelope test source is unchanged.

## Evidence boundary

This is source-only evidence. It does not prove compilation, hydrate/SSR behavior,
browser rendering, mounted endpoint execution, diagnostic emission, workflows, CI,
or production readiness. Cart FFA/FBA status is not promoted.

Suggested checks:

```bash
node scripts/verify/verify-cart-storefront-native-client-error-safety.mjs
node scripts/verify/verify-cart-storefront-native-error-safety.mjs
node scripts/verify/verify-cart-storefront-graphql-error-safety.mjs
node scripts/verify/verify-cart-storefront-boundary.mjs
cargo check -p rustok-cart-storefront
cargo check -p rustok-cart-storefront --features hydrate
cargo check -p rustok-cart-storefront --features ssr
```
