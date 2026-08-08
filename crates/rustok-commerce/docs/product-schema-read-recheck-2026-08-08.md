# Ecommerce Product schema read recheck — 2026-08-08

## Scope

Rechecked the canonical ecommerce execution plan and the currently mounted Product GraphQL read paths against `main` at `dfddce9f57916a712d531db38e75c87e7c45e8cf`.

The source of truth remains `crates/rustok-commerce/docs/implementation-plan.md`. This packet does not promote FBA/FFA or verification status and does not replace that plan.

## Recheck findings

1. `ProductCatalogSchemaReadPort` already publishes the effective-form aggregate capability added by PR #3182, but the mounted `productEffectiveForm` resolver still constructs `ProductCatalogSchemaService` directly. The plan correctly keeps the remaining Product schema-read cutover open.
2. The mounted `productAttributeValues` resolver also constructs `ProductCatalogSchemaService` directly. Before this slice there was no transport-neutral schema-read capability for that projection, so a consumer-only cutover would have bypassed the owner boundary contract.
3. `storefrontCatalogSearchOptions` remains a direct `ProductCatalogSchemaService` consumer and is still explicit follow-up debt.
4. The active `CommerceQueryRoot` mounts both `query::CommerceQuery` and `product_catalog::ProductCatalogQuery`. The newer `adminProductCatalog`/`storefrontProductCatalog` paths use `ProductCatalogReadRuntime`, while legacy mounted `product`/`products` roots in `query::CommerceQuery` still contain direct `CatalogService`/Product entity read paths. Therefore the broad ecommerce invariant requiring typed owner boundaries on every production path must remain open; source inspection does not justify a status promotion.

## Source change in this slice

- Publish `ProductAttributeValuesRequest` and optional `ProductCatalogSchemaReadPort::read_product_attribute_values`.
- Keep existing adapters source-compatible with a stable fail-closed `product.attribute_values_unavailable` default.
- Implement the in-process Product owner capability through `ProductCatalogSchemaService::load_product_attribute_values`, preserving canonical `PortContext` policy, tenant, locale, deadline, correlation, and stable public `PortError` mapping.
- Extend `verify-product-catalog-schema-read-port.mjs` so the owner capability is locked in source while the mounted `productAttributeValues` consumer is explicitly required to remain uncut in this capability-only slice.

## Remaining execution order

1. Cut mounted `productEffectiveForm` to `ProductCatalogSchemaReadPort::read_effective_form`.
2. Cut mounted `productAttributeValues` to `ProductCatalogSchemaReadPort::read_product_attribute_values`.
3. Publish/cut the owner projection needed by `storefrontCatalogSearchOptions`.
4. Reconcile or retire the legacy mounted `product`/`products` GraphQL roots so direct Product service/entity reads no longer violate the every-production-path FBA invariant.
5. Only after the source cutovers, run the plan-listed static, compile, parity, remote-profile, restart, and backend evidence before changing promotion status.

## Verification state

No tests, checks, formatters, or runtime verification were executed in this slice per maintainer instruction. All execution/promotion gates remain unchanged.
