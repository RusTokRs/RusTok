# Storefront shipping public envelope safety

Status: **source-ready / unvalidated**

## Scope

This slice closes only the non-`PortError` public conversion in the compatibility function
`enrich_cart_delivery_groups` from
`crates/rustok-commerce/src/storefront_shipping.rs`.

The function already delegated to `enrich_cart_delivery_groups_typed`, logged the typed
`FulfillmentError` through the bounded diagnostic introduced by the preceding shipping-enrichment
work, and returned `CommerceResult<CartResponse>`. After logging, it still copied the complete owner
error text into `CommerceError::Validation`.

The compatibility result now contains one static message:

`Cart shipping details are temporarily unavailable`

## Preserved boundary

This change does not alter the compatibility function signature or argument order:

- database connection;
- tenant ID;
- cart payload;
- public channel slug;
- requested locale;
- tenant default locale;
- `CommerceResult<CartResponse>` return type.

The function still calls `enrich_cart_delivery_groups_typed` once with those arguments in the same
order. On failure it still calls `log_cart_delivery_group_enrichment_error` before constructing the
compatibility result.

The returned variant remains `CommerceError::Validation` so existing compatibility callers keep the
same error type. Only the dynamic owner message has been removed from the returned payload.

## Preserved typed behavior

The typed enrichment path remains unchanged:

- `FulfillmentService::list_shipping_options` is called with tenant and locale context;
- owner errors propagate through `FulfillmentResult<CartResponse>`;
- currency filtering is unchanged;
- public-channel visibility filtering is unchanged;
- shipping-profile compatibility filtering is unchanged;
- option summaries and selected-option reconciliation are unchanged.

The bounded diagnostic also remains unchanged. It preserves typed owner classification, owner code,
owner kind, retryability, error/warn severity, and the `list_shipping_options` operation while using a
redacted source token and shape-only request context.

## Callers

This slice does not edit GraphQL queries, GraphQL mutation helpers, storefront HTTP handlers, or
routing. Existing compatibility callers receive the same `CommerceError::Validation` variant, now
with the static message rather than validation, identity, transition, or database details from the
fulfillment owner.

## Static guards

The following guards were advanced without being executed:

- `verify-commerce-storefront-shipping-public-envelope-safety.mjs` checks the exact compatibility
  signature, argument order, typed call, diagnostic-before-mapping order, static envelope, and absence
  of owner-error stringification;
- `verify-commerce-storefront-shipping-enrichment-context.mjs` now requires bounded diagnostics and
  the stable compatibility envelope;
- `verify-commerce-storefront-cart-shipping-http-error-safety.mjs` preserves all HTTP mapper and typed
  enrichment checks while requiring the stable compatibility mapping;
- `verify-commerce-storefront-shipping-enrichment-diagnostic-safety.mjs` retains all redaction,
  classification, and severity checks and advances its compatibility assertion to the static message.

## Remaining work

This slice does not close:

- raw diagnostics in `map_storefront_cart_shipping_error`;
- shared storefront context, customer, and channel mapper cleanup;
- direct GraphQL compatibility routing cleanup;
- tax and promotion mapper cleanup;
- native transport cleanup;
- remaining ecommerce adapters.

The broad canonical correlation-safe mapper cleanup remains unchecked.

## Evidence

- `crates/rustok-commerce/contracts/evidence/storefront-shipping-public-envelope-safety-source-review.json`
- `scripts/verify/verify-commerce-storefront-shipping-public-envelope-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted HTTP or GraphQL scenarios, workflows, or
CI were run. No compile or runtime status is claimed.
