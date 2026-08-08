# M7 Product Storefront Index parity gate

Status: `shadow_query_and_product_owned_eav_resolution_source_complete_execution_evidence_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

The Index-side shadow query builder is source-complete, and Product now owns the missing public
Storefront attribute-filter resolution capability. The next slice is shadow execution/equivalence, not
consumer cutover.

## Product-owned attribute-filter resolution — source complete

`ProductCatalogSchemaReadPort` now exposes optional
`resolve_storefront_attribute_filters`. Existing external adapters remain source-compatible because the
default implementation fails closed with
`product.storefront_attribute_filter_resolution_unavailable`.

The in-process Product implementation resolves each canonical `ProductAttributeFilter` using the same
owner rules as the mounted SQL list path:

- at most eight filters, case-insensitive duplicate-code rejection and existing code/value bounds;
- definition eligibility is tenant-scoped, active, filterable and `product|both` scope;
- attribute lookup is case-insensitive by code;
- text is exact value matching;
- integer/decimal/boolean/date/datetime parsing is shared with the owner SQL condition builder;
- localized text is represented as `requested-value OR (NOT requested-present AND fallback-value)`;
- Select/Multiselect accepts a UUID directly or resolves an exact active option code to its option UUID;
- missing option code and nil UUID resolve to neutral `ProductAttributeTermExpr::Never`, preserving the
  owner SQL empty-result behavior rather than inventing a validation error;
- JSON remains unsupported exactly as on the owner list path.

Product exports a neutral `ProductAttributeTermExpr` (`Term`, `And`, `Or`, `Not`, `Never`) and owns the
canonical term grammar `attribute_uuid|kind|hex(locale)|hex(value)`. Product does not depend on
`rustok-index`.

`rustok-distribution` no longer owns a second Rust term encoder: its Product Index helpers delegate term
identity to `rustok-product`. The PostgreSQL Product source CTE remains the materialization-side mirror of
the same grammar.

## Shadow query adapter — source complete

`build_product_storefront_index_shadow_query` remains a pure crate-local translation into
`LocalizedEntityQuery`. It maps Active/published-only, trusted public-channel membership, optional primary
category, canonical `attribute_terms`, any-locale title `TextLike`, requested/fallback title+handle,
paired timestamp order plus matching Product-ID tie-break, bounded offset pagination and exact count.
It selects stable `tag_ids` for later Taxonomy hydration and uses only current Product routing key `4`.

The builder still does not execute Index queries, read Product EAV tables, hydrate Taxonomy names or mount
Storefront traffic.

## Remaining fail-closed parity gates

1. Owner title search has no explicit length bound while Index `TextLike` is bounded to 1024 UTF-8 bytes.
2. Owner title `LIKE` uses deployment/default collation while Index String SQL uses deterministic
   `COLLATE "C"`.
3. Channel-less owner requests mean metadata-unrestricted only; current `sales_channel_ids` cannot
   distinguish unrestricted from restricted-to-all-current-channels.
4. Owner page depth is wider than the Index 10,000 offset bound.
5. Taxonomy tag names must be hydrated only after Product page identity/order/count is fixed.

## Next source slice

Compose a **shadow executor/equivalence harness** that:

- obtains `ProductCatalogSchemaReadPort` from the host-selected Product read runtime;
- resolves public EAV filters through `resolve_storefront_attribute_filters`;
- translates neutral Product term expressions into canonical Index `attribute_terms` filters;
- executes only through `execute_localized_query` with existing readiness/admission/snapshot semantics;
- retains owner and Index results side-by-side without serving Index output to Storefront;
- batch-hydrates Taxonomy tags only after the Product page is fixed;
- records PostgreSQL equivalence across requested/fallback/third-locale search, EAV option code/UUID,
  localized EAV fallback, channel membership, Asc/Desc equal timestamps, count, pagination, stale rows and
  restart/readiness cases.

Historical Product PostgreSQL packets still need routing key `4` / current 15-field actualization. Do not
add a key-3 runtime compatibility path.

## Source guards

- `verify-product-storefront-attribute-filter-terms.mjs` locks Product ownership, shared typed parsing,
  option resolution, neutral expression shape and distribution delegation;
- `verify-index-product-storefront-shadow-adapter.mjs` locks the pure fail-closed shadow translation;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native;
- localized runtime/order/TextLike guards continue to lock the generic Index execution contract.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
