# Ecommerce Product schema read recheck — 2026-08-08

## Scope

Rechecked the canonical ecommerce execution plan and the currently mounted Product GraphQL read paths against `main` at `dfddce9f57916a712d531db38e75c87e7c45e8cf`, then continued the Product schema-read cutover from `aaa496887fd1492f3feca8ac52261458ce25705e`.

The source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`. This packet does not promote FBA/FFA or verification status and does not replace that plan.

## Recheck findings

1. `ProductCatalogSchemaReadPort` publishes the effective-form aggregate capability added by PR #3182. The mounted `productEffectiveForm` resolver was still constructing `ProductCatalogSchemaService` directly and is cut over in the continuation slice below.
2. The mounted `productAttributeValues` resolver still constructs `ProductCatalogSchemaService` directly. PR #3203 published the transport-neutral owner capability needed for its next consumer cutover.
3. `storefrontCatalogSearchOptions` remains a direct `ProductCatalogSchemaService` consumer and still needs an owner projection/cutover.
4. The active `CommerceQueryRoot` mounts both `query::CommerceQuery` and `product_catalog::ProductCatalogQuery`. The newer `adminProductCatalog`/`storefrontProductCatalog` paths use `ProductCatalogReadRuntime`, while legacy mounted `product`/`products` roots in `query::CommerceQuery` still contain direct `CatalogService`/Product entity read paths. Therefore the broad ecommerce invariant requiring typed owner boundaries on every production path must remain open; source inspection does not justify a status promotion.

## PR #3203 source change

- Published `ProductAttributeValuesRequest` and optional `ProductCatalogSchemaReadPort::read_product_attribute_values`.
- Kept existing adapters source-compatible with a stable fail-closed `product.attribute_values_unavailable` default.
- Implemented the in-process Product owner capability through `ProductCatalogSchemaService::load_product_attribute_values`, preserving canonical `PortContext` policy, tenant, locale, deadline, correlation, and stable public `PortError` mapping.
- Extended `verify-product-catalog-schema-read-port.mjs` so the owner capability is locked in source while the mounted `productAttributeValues` consumer remains explicit follow-up debt.

## Effective-form continuation

- Cut mounted `productEffectiveForm` to the host-selected `ProductCatalogSchemaReadPort::read_effective_form` capability.
- Preserve current-tenant admission, `PRODUCTS_READ`, authenticated actor, trimmed locale, request channel, bounded deadline, and the shared correlation-aware Product GraphQL public error mapper.
- Preserve the existing GraphQL input rule: `product_id` wins when supplied; otherwise `category_id` is required.
- Consume the Product-owned aggregate projection directly, so Commerce no longer reconstructs group labels, definitions, options, and missing-definition invariants for this resolver.
- Extend the schema-read source guard to require the mounted effective-form owner-port path and forbid direct `ProductCatalogSchemaService` construction in it.

## Remaining execution order

1. Cut mounted `productAttributeValues` to `ProductCatalogSchemaReadPort::read_product_attribute_values`.
2. Publish/cut the owner projection needed by `storefrontCatalogSearchOptions`.
3. Reconcile or retire the legacy mounted `product`/`products` GraphQL roots so direct Product service/entity reads no longer violate the every-production-path FBA invariant.
4. Continue remaining Product schema writes and lifecycle command cutovers from the canonical ecommerce plan.
5. Only after the source cutovers, run the plan-listed static, compile, parity, remote-profile, restart, and backend evidence before changing promotion status.

## Verification state

No tests, checks, formatters, or runtime verification were executed in these slices per maintainer instruction. All execution/promotion gates remain unchanged.
