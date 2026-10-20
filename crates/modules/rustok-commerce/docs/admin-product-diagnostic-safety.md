# Admin product diagnostic safety

Status: **source-ready / unvalidated**

## Current boundary (2026-10-18)

The shared Product HTTP path now has separate typed-read and owner-port error mappers.
`map_admin_product_error` handles five legacy shared read failure sites. Its typed
Product error enables product-not-found identity adoption and HTTP policy selection;
afterward the error is shadowed by a diagnostic type whose Debug output is always `redacted`.
Tenant/actor are `nil` / `non_nil`; optional Product identity is
`absent` / `present_nil` / `present_non_nil`. Raw errors/UUIDs are not mapper log fields.

The selected owner command/read paths use `map_admin_product_port_error`: shared delete,
publish, unpublish and archive plus mounted list/detail/create/update. It logs bounded
identifier shapes, owner-code length, retryability, static kind/public policy, and trusted
request correlation. It never logs the owner's message, code text or Debug payload.
Permissions, revision-required/conflict semantics, Forbidden/503 mapping and stable success
responses remain explicit. Source guards isolate mapper function bodies: unrelated retry-key
hashing/formatting must not be mistaken for an unsafe error payload.

## Authenticated scope and request provenance

Shared and mounted Product handlers require matching TenantContext, AuthContext and
RequestContext tenant/principal dimensions before IO or receipt admission. Authenticated
scope mismatch is 403; absent authentication is still rejected by the extractor.
Mounted read owner contexts preserve upstream request correlation instead of synthesizing
it from Product identity or pagination. Missing RequestContext is not invented by the handler.

Shipping Profile validation is no longer a mutable HTTP preflight: it runs inside Product
receipt execution through ProductShippingProfilePort. Completed receipt replay remains
independent of live Shipping Profiles. See the [write boundary](../../rustok-product/docs/write-boundary.md#receipt-aware-shipping-profile-admission).

## Evidence limits

The three diagnostic/route/helper source guards track current Product/Order port boundaries,
not removed broad Commerce variants or old direct command service calls. Their execution
and WASM formatting are source-only evidence; test suites, Rust compilation, mounted
extractors, real owner providers, persistence and runtime behavior remain unexecuted here.
The metadata artifact records that distinction; it is not a runtime certification packet.
The broader ecommerce correlation-safe mapper and non-`PortError` envelope cleanup remains open.


## Product dependency classification (2026-10-18)

Product admin and storefront HTTP now share a static policy derived from Product's exhaustive
nested Core classifier. Forbidden is 403, unavailable dependencies are 503, validation is 400,
referenced-resource not-found is 404, and invariant failures remain 500 with the existing per-surface
fallback code. Generic Product Conflict is 409. The direct Product GraphQL/native shared public
mapper and owner-port GraphQL preserve the same classes; definitive command outcomes require
both a known domain code and an actual domain kind. No raw dependency message is rendered.
See the [owner contract](../../rustok-product/docs/product-public-error-diagnostic-safety.md#taxonomy-and-nested-core-continuation-2026-10-18).
New HTTP/GraphQL Rust scenario sources are unrun. Standalone source checks are not authenticated
mounted runtime, log-capture, compilation or CI evidence.
