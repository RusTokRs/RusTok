# Product shared public-error diagnostic safety

Status: **source-ready / unvalidated**

## Scope

This slice closes the payload-diagnostic gap in Product-owned `map_product_public_error`, the shared owner mapper used by GraphQL and native admin/storefront consumers.

The mapper still accepts `CommerceError`, creates one `ProductPublicError`, and preserves its safe message, stable code, retryability, generated correlation id, and display reference. Consumer adapters, requests, responses, fallback behavior, and transport composition are unchanged.

All eight current `CommerceError` variants remain mapped:

- database;
- product not found;
- duplicate handle;
- duplicate SKU;
- validation;
- no variants;
- cannot delete published;
- core failure.

## Retained diagnostic shape

The private structured event retains static operation, boundary, public code, retryability, correlation id, and a closed error variant.

Dynamic owner payload is represented only by aggregate shape:

- text field count and total character length;
- UUID field count and non-nil count;
- opaque payload presence for database and core failures.

Database/core errors, product UUIDs, handles, locales, SKUs, validation messages, and the complete `CommerceError` debug/display payload are not recorded.

## Preserved behavior

- every public message is unchanged;
- every public code is unchanged;
- retryability is unchanged;
- `Uuid::new_v4()` still creates the correlation id;
- `ProductPublicError::Display` still renders message, code, and reference;
- the root Product export and all GraphQL/native consumers are unchanged.

## Evidence

- `crates/rustok-product/contracts/evidence/product-public-error-diagnostic-safety-source.json`
- `crates/rustok-product/contracts/evidence/product-public-error-diagnostic-safety-source-review.json`
- `scripts/verify/verify-product-public-error-diagnostic-safety.mjs`

## Remaining gaps

Compile, verifier execution, mounted GraphQL/native runtime evidence, and the broader ecommerce mapper cleanup remain open. This source slice does not promote Product FFA/FBA or transport status.

No test, verifier, formatter, Cargo, workflow, or CI command was executed for this source slice.
