# Commerce REST storefront Product list owner-read cutover

Status: `source_complete_unvalidated`

Date: 2026-08-10

## Scope

This bounded slice moves mounted `GET /store/products` behind a Product-owned,
host-composed read capability. Commerce no longer queries Product entities or constructs
`CatalogService` on the mounted list route.

The existing Product catalog runtime remains the host composition point. Its embedded
profile now installs optional `ProductStorefrontHttpReadPort`; external profiles do not
receive that capability implicitly and therefore fail closed until the host explicitly
supplies an implementation.

## Why a separate compatibility capability is required

The existing `ProductCatalogReadPort::list_legacy_storefront_products` is the compatibility
projection for mounted legacy GraphQL and is deliberately left unchanged. Reusing it for
REST would alter observable legacy REST behavior:

- GraphQL legacy list admits at most 48 rows while REST pagination clamps to 100;
- GraphQL legacy list uses `"Untitled product"` when no translation can be projected,
  while REST emits an empty title;
- GraphQL legacy list prefers the Product shipping-profile column before metadata,
  while REST derives this list field from metadata only and defaults to `"default"`;
- REST historically applies public-channel visibility after the active/published/filter
  query and before pagination/total calculation.

The new optional `ProductStorefrontHttpReadPort::list_legacy_storefront_http_products`
therefore owns those REST compatibility semantics without mutating the GraphQL contract.

## Preserved REST semantics

The Product-owned projection preserves the mounted list behavior:

1. tenant, active status, and non-null publication filters;
2. exact optional vendor and product-type filters;
3. optional raw title `LIKE %search%` across Product translations — the historical helper
   accepts a locale argument but does not currently restrict the search by locale;
4. published-at descending, then created-at descending ordering;
5. in-memory public-channel metadata visibility before total/pagination;
6. page normalization to at least 1 and page-size clamping to 1..=100 at the HTTP boundary;
7. translation projection in requested locale, then tenant default locale, then first
   available translation;
8. empty title/handle when no translation exists;
9. existing Product tag hydration with requested/default locale;
10. metadata-only shipping-profile normalization/defaulting;
11. unchanged `ProductListItem` JSON projection and pagination metadata.

The storefront channel-enabled guard remains before the Product owner call.

## Context and failure behavior

The mounted handler creates a read `PortContext` with:

- admitted tenant id;
- stable Commerce storefront Product service actor;
- effective request/list locale;
- normalized public channel when present;
- page-bound correlation id;
- two-second deadline.

Product owner failures are mapped to the existing public REST families:

- validation -> `400 commerce_store_product_invalid`;
- not found -> `404 commerce_store_not_found`;
- unavailable/timeout -> `503 commerce_store_product_unavailable`;
- conflict/forbidden/invariant -> `500 commerce_store_product_failed`.

If an external `ProductCatalogReadRuntime` has not explicitly installed the optional HTTP
list capability, Commerce fails closed with `503 commerce_store_product_unavailable` and
does not construct an embedded fallback.

Diagnostics at the mounted boundary contain bounded owner kind, code length,
retryability, correlation, tenant-presence and public-envelope facts. Raw owner/backend
messages are not logged or returned by the new boundary.

## Mounted topology

`controllers/store/mod.rs` now mounts the original `products.rs` as
`products_legacy` and mounts `products_owner_list.rs` as `products`.
The new module owns `list_products` and provides thin annotated wrappers that delegate
Product detail, region, and shipping-option requests to the unchanged legacy handlers.
This preserves the existing OpenAPI handler markers while changing only the Product list
implementation. The old direct list implementation remains compiled compatibility source
but is not mounted.

The existing storefront Product detail remains on
`ProductCatalogReadPort::read_storefront_product_projection`.

## Topology status

The canonical ecommerce P0 item to move remaining mounted Commerce REST/GraphQL
Product, Order, Payment, and Fulfillment concrete service construction behind
host-composed owner ports remains open. This slice only removes the mounted storefront
Product list violation.

## Validation status

Per maintainer instruction, no tests, Cargo commands, Node verifiers, formatter, REST
scenarios, workflows, CI reruns, database scenarios, provider calls, restart scenarios,
or remote-adapter scenarios were executed. Source guards added/updated by this slice are
source-reviewed only and were not run.