# Storefront shipping enrichment diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only the owner-error diagnostic emitted by
`log_cart_delivery_group_enrichment_error` in
`crates/rustok-commerce/src/storefront_shipping.rs`.

The typed enrichment boundary already returned `FulfillmentResult<CartResponse>`, delegated to
`FulfillmentService::list_shipping_options`, and classified all owner variants before choosing error
or warning severity. Its events still formatted the complete `FulfillmentError` and emitted raw
request identities and locale/channel strings.

## Bounded diagnostic projection

The mapper still classifies these owner variants:

- `Validation`;
- `ShippingOptionNotFound`;
- `FulfillmentNotFound`;
- `InvalidTransition`;
- `Database`.

It still preserves the established owner code, owner kind, retryability, exact owner operation,
boundary, severity, and static event messages.

Before either event, the source now projects only:

- tenant UUID shape: `nil` or `non_nil`;
- cart UUID shape: `nil` or `non_nil`;
- public channel slug shape: `absent`, `empty`, or `present`;
- requested locale shape: `absent`, `empty`, or `present`;
- tenant default locale shape: `absent`, `empty`, or `present`.

The event error field is a zero-sized `StorefrontShippingDiagnosticError`. Its `Debug`
implementation always writes `redacted`.

No validation detail, shipping-option or fulfillment identity, transition text, database error,
tenant/cart UUID, channel slug, or locale string is formatted by these events.

## Preserved behavior

This work does not change:

- `enrich_cart_delivery_groups_typed` or its `FulfillmentResult<CartResponse>` contract;
- `FulfillmentService::new` construction;
- the `list_shipping_options(tenant_id, requested_locale, tenant_default_locale)` call;
- direct typed owner-error propagation from that call;
- currency and public-channel filtering;
- shipping-profile compatibility filtering;
- option summary projection;
- delivery-group selected-option reconciliation;
- database failures using `tracing::error!`;
- non-database owner rejections using `tracing::warn!`;
- the compatibility wrapper signature and delegation.

## Explicitly separate remaining work

The compatibility wrapper still maps the typed owner failure to
`CommerceError::Validation(error.to_string())`. That non-`PortError` public conversion is not changed
by this diagnostic-only slice and remains open.

The storefront HTTP cart-shipping mapper in `controllers/store/mod.rs`, the shared storefront context,
customer, and channel mappers, tax, promotion, native transports, and remaining ecommerce adapters
also remain open.

The broad canonical correlation-safe mapper cleanup remains unchecked.

## Static guard

`scripts/verify/verify-commerce-storefront-shipping-enrichment-diagnostic-safety.mjs` checks:

- the complete typed owner classification and owner codes;
- severity selection before source shadowing;
- UUID and optional-text shape projection;
- the zero-sized redacted diagnostic token;
- one error event and one warning event;
- absence of raw owner payload, identity, channel, and locale diagnostics;
- preservation of typed enrichment delegation and option projection;
- preservation of the legacy public conversion as an explicitly separate open boundary.

The verifier was added but not executed.

## Evidence

- `crates/rustok-commerce/contracts/evidence/storefront-shipping-enrichment-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-storefront-shipping-enrichment-diagnostic-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted HTTP or GraphQL scenarios, workflows, or
CI were run. No compile or runtime status is claimed.
