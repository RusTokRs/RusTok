# GraphQL typed line-item diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only the typed storefront line-item GraphQL mapper in
`crates/rustok-commerce/src/graphql/mutations/typed_line_item_helpers.rs`.

The mapper already selected public policy from typed failure kinds and retained truthful source
owner, source operation, consumer operation, severity, and stable `CART_*` envelopes. Its diagnostic
events still formatted the complete owner source and emitted raw correlation, tenant, variant, and
optional product identities.

The affected source variants are:

- `sea_orm::DbErr` from product persistence;
- Pricing `PortError`;
- Inventory `CommerceError`;
- `serde_json::Error` from metadata parsing;
- local policy reasons.

## Bounded diagnostic projection

`StorefrontLineItemFailureSource::kind()` still projects the typed source kind before the payload is
consumed. The complete source is then moved into `StorefrontLineItemDiagnosticSource`, a zero-sized
diagnostic token with a custom `Debug` implementation whose only output is `redacted`.

The mapper now emits only closed identity facts:

- correlation ID: `absent`, `empty`, or `present`;
- tenant and variant UUIDs: `nil` or `non_nil`;
- optional product UUID: `absent`, `present_nil`, or `present_non_nil`.

It continues to retain:

- typed source kind;
- truthful source owner and owner operation;
- consumer operation;
- typed failure kind;
- requested quantity;
- channel-slug and locale lengths;
- public code and retryability;
- the static GraphQL boundary and event messages.

No database, pricing, inventory, metadata, or local-policy payload is formatted by either event.

## Preserved behavior

This work does not change:

- `ProductUnavailable`, `InventoryInsufficient`, `InputInvalid`, or
  `DependencyUnavailable` policy selection;
- public messages, codes, or retryability;
- error-level diagnostics for dependency failures;
- warning-level diagnostics for ordinary storefront rejections;
- Product persistence reads;
- the Pricing `resolve_product_price` request or `PriceResolutionContext` fields;
- the Inventory public-channel availability call;
- metadata parsing, shipping-profile selection, titles, pricing snapshots, or result DTOs;
- the two mounted typed helper exports and their arguments;
- compatibility helper routing or string classification in `safe_helpers.rs`.

The preserved public policies are:

- unavailable product → `CART_PRODUCT_UNAVAILABLE`, non-retryable;
- insufficient inventory → `CART_INVENTORY_INSUFFICIENT`, non-retryable;
- invalid resolve input → `CART_LINE_ITEM_INVALID`, non-retryable;
- other resolve failures → `CART_LINE_ITEM_RESOLUTION_FAILED`, retryable;
- other quantity-validation failures → `CART_INVENTORY_UNAVAILABLE`, retryable.

## Static guards

`verify-commerce-graphql-typed-line-item-diagnostic-safety.mjs` checks:

- typed source variants and constructors;
- source-kind projection before consuming redaction;
- the zero-sized diagnostic token and custom `Debug` output;
- correlation and UUID shape projection;
- absence of raw source, identity, locale, channel, metadata, SKU, and title diagnostics;
- preservation of severity, public policy, Pricing/Inventory delegations, `pricing_context`, and the
  two mounted typed exports.

The existing `verify-commerce-graphql-cart-helper-error-safety.mjs` was updated only for the typed
mapper contract. Its customer, legacy, compatibility-call-count, and layered-routing checks remain
unchanged.

## Remaining work

Still open:

- compatibility string classification in the legacy storefront line-item wrappers;
- storefront shared and cart-shipping mappers;
- tax and promotion diagnostics;
- native transports and remaining owner adapters;
- mounted execution, compilation, workflow, and CI evidence.

The broad ecommerce correlation-safe mapper cleanup remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-typed-line-item-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-typed-line-item-diagnostic-safety.mjs`
- `scripts/verify/verify-commerce-graphql-cart-helper-error-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI
were run. No compile or runtime status is claimed.
