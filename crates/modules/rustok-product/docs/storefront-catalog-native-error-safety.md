# Product storefront native read safety

Status: **source-ready / unvalidated**

## Current boundary (2026-10-30)

Catalog list, detail, catalog schema/search options, facets and buyer quote server functions use
one required `product_buyer_scope`. It resolves the host, tenant, trusted request context, optional
auth and transactional event bus fallibly, then applies Commerce storefront admission before owner
IO. Anonymous/no-channel contexts remain valid; rejected contexts never become optional defaults.

The host-selected `ProductCatalogReadRuntime` supplies catalog and optional schema capabilities.
Missing schema support refuses rather than returning empty filter options or constructing a second
local schema resolver. The in-process runtime remains the owner's explicit composition default.

A two-second Tokio endpoint budget covers admission and the subsequent owner calls. Port contexts
receive the remaining budget. A timeout drops the read future; this does **not** establish database
cancellation, remote-provider execution or a reservation. Cart mutation deadlines and receipts are
not certified by this Product read budget.

## Public and diagnostic policy

Technical and extractor failures return the owner public envelope with a fresh opaque reference.
The server logs bounded error categories, stable codes, presence/length/count facts and correlation
only. Raw context, tenant, auth token, locale, channel and backend error payloads are not copied to
these diagnostic events or public technical messages. Input/service classification continues
through the existing Product public mapper; service validation is not removed to satisfy a guard.

The native catalog keeps the typed Product list query, first page/per-page bounds and optional
currency summary. Detail and quote reads preserve actual sparse SKU UUIDs. The Leptos support
resource propagates schema/facet errors instead of disguising them as empty controls.

## Verification boundary

`node scripts/verify/verify-product-storefront-catalog-native-error-safety.mjs` checks these source
contracts, not mounted behavior. The existing Product storefront boundary guard composes it with
the GraphQL safety guard. Regression fixtures have been updated but **not executed**.

Scoped source checks and Scalar syntax/formatting do not prove Rust compilation, shared-context
inheritance at runtime, timeouts/cancellation, SSR/WASM hydration, auth/cookie continuity, remote
capabilities or browser accessibility. Those remain maintainer gates; no FFA/FBA status is promoted.
