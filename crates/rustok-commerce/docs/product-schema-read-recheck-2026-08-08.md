# Ecommerce Product schema read recheck — 2026-08-08

## Scope

Rechecked the canonical ecommerce execution plan and the currently mounted Product GraphQL read paths against `main` at `dfddce9f57916a712d531db38e75c87e7c45e8cf`, then continued the Product schema-read and legacy catalog-read cutover from `aaa496887fd1492f3feca8ac52261458ce25705e` through the current `main` line.

The source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`. This packet does not promote FBA/FFA or verification status and does not replace that plan.

## Recheck findings

1. `ProductCatalogSchemaReadPort` publishes the effective-form aggregate capability added by PR #3182, and mounted `productEffectiveForm` is now cut over to that host-selected owner capability.
2. PR #3203 published `ProductCatalogSchemaReadPort::read_product_attribute_values`, and mounted `productAttributeValues` is now cut over to that owner capability.
3. `storefrontCatalogSearchOptions` was the remaining mounted direct `ProductCatalogSchemaService` schema-read consumer. Its current projection needs only categories and filterable/sortable attributes, so the already-published `list_categories` and `list_attributes` owner capabilities preserve the existing data and ordering semantics without a new Product projection.
4. The active `CommerceQueryRoot` mounts both `query::CommerceQuery` and `product_catalog::ProductCatalogQuery`. The newer `adminProductCatalog`/`storefrontProductCatalog` paths already use `ProductCatalogReadRuntime`; the mounted legacy admin `product`/`products` roots were still direct `CatalogService` / Product-entity consumers and are cut over in the continuation below.
5. The legacy mounted `storefrontProduct` / `storefrontProducts` roots remain direct `CatalogService` / Product-entity consumers. Therefore the broad ecommerce invariant requiring typed owner boundaries on every production path remains open; source inspection does not justify a status promotion.

## PR #3203 source change

- Published `ProductAttributeValuesRequest` and optional `ProductCatalogSchemaReadPort::read_product_attribute_values`.
- Kept existing adapters source-compatible with a stable fail-closed `product.attribute_values_unavailable` default.
- Implemented the in-process Product owner capability through `ProductCatalogSchemaService::load_product_attribute_values`, preserving canonical `PortContext` policy, tenant, locale, deadline, correlation, and stable public `PortError` mapping.
- Extended `verify-product-catalog-schema-read-port.mjs` so the owner capability is locked in source while the mounted consumer remained explicit follow-up debt.

## Effective-form continuation

- Cut mounted `productEffectiveForm` to the host-selected `ProductCatalogSchemaReadPort::read_effective_form` capability.
- Preserve current-tenant admission, `PRODUCTS_READ`, authenticated actor, trimmed locale, request channel, bounded deadline, and the shared correlation-aware Product GraphQL public error mapper.
- Preserve the existing GraphQL input rule: `product_id` wins when supplied; otherwise `category_id` is required.
- Consume the Product-owned aggregate projection directly, so Commerce no longer reconstructs group labels, definitions, options, and missing-definition invariants for this resolver.
- Extend the schema-read source guard to require the mounted effective-form owner-port path and forbid direct `ProductCatalogSchemaService` construction in it.

## Attribute-values continuation

- Cut mounted `productAttributeValues` to the host-selected `ProductCatalogSchemaReadPort::read_product_attribute_values` capability published in PR #3203.
- Preserve current-tenant admission, `PRODUCTS_READ`, authenticated actor, trimmed locale, request channel, bounded deadline, and the shared correlation-aware Product GraphQL public error mapper.
- Preserve the existing GraphQL response shape by mapping the Product-owned `ProductAttributeValueRecord` projection through the existing `GqlProductAttributeValue` conversion.
- Extend the schema-read source guard to require `ProductAttributeValuesRequest`, the mounted owner-port call, and no direct `ProductCatalogSchemaService` construction in this resolver.

## Storefront search-options continuation

- Cut mounted `storefrontCatalogSearchOptions` to the host-selected `ProductCatalogSchemaReadPort` using its existing `list_categories` and `list_attributes` capabilities; no new Product port method is required.
- Preserve storefront-channel admission, the required non-empty trimmed locale, current-tenant scope, the `commerce-storefront-graphql` service actor, request channel, bounded deadline, and the shared correlation-aware Product GraphQL public error mapper.
- Preserve the exact GraphQL projection: category values remain category ids with path/name/code fallback labels; attribute options remain limited to filterable or sortable attributes and keep the existing `label (code)` formatting.
- Extend the schema-read source guard to require both owner calls and the existing option-projection semantics while forbidding direct `ProductCatalogSchemaService` construction in this resolver.

## Legacy admin Product read continuation

- Publish optional fail-closed `ProductCatalogReadPort::list_legacy_admin_products` / `LegacyAdminProductsRequest` so existing remote/test adapters remain source-compatible until they explicitly implement the exact legacy list projection.
- Implement the in-process compatibility capability through Product's existing owner query path, preserving status/vendor/search filtering, created-at-desc ordering, page default/clamp semantics, requested/default locale fallback, `Untitled product`, normalized metadata-aware shipping-profile fallback, tags, and the existing list projection.
- Cut mounted legacy admin GraphQL `product` to host-selected `ProductCatalogReadPort::read_product_projection`, preserving current-tenant and `PRODUCTS_READ` admission, the pre-existing storefront-channel admission, authenticated actor, requested/default locale fallback, request channel, bounded deadline, detail projection, and `NotFound -> null` behavior.
- Cut mounted legacy admin GraphQL `products` to `list_legacy_admin_products`, preserving `PRODUCTS_LIST`, current tenant, authenticated actor, request channel/deadline, exact legacy filters/order/pagination/projection, telemetry path, and the shared stable Product GraphQL public error mapper.
- Extend the Product admin GraphQL source guard so both modern and legacy admin read roots require the host-selected runtime/ports and direct Product service/entity access is forbidden inside the cut-over resolvers.

## Remaining execution order

1. Cut legacy mounted `storefrontProduct` / `storefrontProducts` away from direct Product service/entity reads while preserving published/channel visibility, id-or-handle detail semantics, vendor/product-type/search filtering, inventory enrichment, ordering, pagination, locale fallback, and response projection.
2. Continue remaining Product schema writes and lifecycle command cutovers from the canonical ecommerce plan.
3. Only after the source cutovers, run the plan-listed static, compile, parity, remote-profile, restart, and backend evidence before changing promotion status.

## Verification state

No tests, checks, formatters, or runtime verification were executed in these slices per maintainer instruction. All execution/promotion gates remain unchanged.
