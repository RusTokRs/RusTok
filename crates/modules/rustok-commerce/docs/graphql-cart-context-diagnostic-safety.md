# GraphQL cart store-context diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice hardens only `cart_context_boundary` in
`crates/rustok-commerce/src/graphql/mutations/safe_cart.rs`.

The boundary converts `StoreContextError` into a cloneable GraphQL-safe public envelope for the two
store-context resolution call sites used by storefront Cart mutations. Public policy was already
selected from typed variants, but the diagnostic event still formatted the complete owner error.
That could expose tenant UUIDs, validation text, currency/region details, boundary codes and messages,
or a database cause.

## Redacted diagnostic boundary

The conversion now performs these steps in order:

1. select the existing typed public message, code, retryability, and `error_kind`;
2. shadow the complete `StoreContextError` with a zero-sized `StoreContextDiagnosticError`;
3. emit the same error-level event using the redacted diagnostic token;
4. return the unchanged cloneable public envelope.

`StoreContextDiagnosticError` has a custom `Debug` implementation whose only output is
`redacted`. It contains no fields and cannot retain a tenant, region, currency, validation,
boundary, database, or correlation payload.

The event still retains only bounded policy facts:

- owner `rustok_commerce.store_context`;
- typed `error_kind`;
- public code;
- public retryability;
- operation `resolve_store_context`;
- boundary `commerce_graphql_cart`;
- the existing static event message.

## Preserved behavior

This work does not change:

- `StoreContextError` variants or owner-service behavior;
- the two `StoreContextService::new` and `resolve_context` resolver paths;
- GraphQL error pass-through through `BoundaryError::Graphql`;
- the cloneable `BoundaryError::Public` representation;
- public messages, codes, or retryability;
- Cart owner diagnostics;
- Pricing owner diagnostics;
- resolver inclusion, shims, or mutation signatures.

The preserved public policies are:

- `TenantNotFound` → `STORE_CONTEXT_NOT_FOUND`, non-retryable;
- `Validation` and `CurrencyRegionMismatch` → `STORE_CONTEXT_REQUEST_INVALID`, non-retryable;
- `TenantBoundary` and `RegionBoundary` → `STORE_CONTEXT_RESOLUTION_FAILED`, non-retryable;
- `Database` → `STORE_CONTEXT_TEMPORARILY_UNAVAILABLE`, retryable.

## Static guards

The existing `verify-commerce-graphql-cart-context-error-safety.mjs` continues to guard the typed
public boundary and resolver routing.

The new `verify-commerce-graphql-cart-context-diagnostic-safety.mjs` additionally checks:

- the zero-sized diagnostic token and custom redacted `Debug`;
- typed envelope selection before payload shadowing;
- exactly one shadow and one diagnostic event;
- shadowing before event emission and public-envelope construction;
- absence of stringification and raw owner payload fields;
- preservation of all typed public policies and both resolver call sites.

## Remaining work

Still open:

- typed line-item source and identity diagnostics;
- compatibility string classification;
- storefront shared and cart-shipping mappers;
- tax and promotion diagnostics;
- native transports and remaining owner adapters;
- mounted execution, compilation, workflow, and CI evidence.

The broad ecommerce correlation-safe mapper cleanup remains open.

## Evidence

- `crates/rustok-commerce/contracts/evidence/graphql-cart-context-diagnostic-safety-source-review.json`
- `scripts/verify/verify-commerce-graphql-cart-context-diagnostic-safety.mjs`

## Validation disclosure

No tests, Node verifiers, formatting, Cargo commands, mounted GraphQL scenarios, workflows, or CI
were run. No compile or runtime status is claimed.
