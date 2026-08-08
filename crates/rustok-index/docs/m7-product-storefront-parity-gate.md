# M7 Product Storefront Index parity gate

Status: `core_postgres_packet_source_complete_execution_and_eav_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

The Product-owned EAV resolver, pure localized shadow query builder and non-serving owner-first shadow
executor are source-complete. A first current-key PostgreSQL owner-vs-shadow packet is now retained in
source, but it has **not** been executed or admitted by the implementation agent.

## Product-owned attribute-filter resolution — source complete

`ProductCatalogSchemaReadPort::resolve_storefront_attribute_filters` is the only Product metadata boundary
used by shadow execution. Existing external adapters remain source-compatible through the fail-closed
default implementation.

The owner resolver preserves mounted list semantics for definition eligibility, typed scalar parsing,
localized requested/fallback text behavior and Select/Multiselect UUID/code resolution. It returns neutral
`ProductAttributeTermExpr` values and Product owns the canonical term grammar. Distribution only translates
those owner-owned expressions into `attribute_terms` Index predicates.

## Shadow query and executor — source complete

`build_product_storefront_index_shadow_query` accepts only Product-owned resolved filters and maps
Active/published-only, trusted current public-channel membership, optional category, canonical EAV terms,
any-locale title `TextLike`, requested/fallback title+handle, paired timestamp order plus matching Product-ID
direction, bounded offset pagination and exact count.

`ProductStorefrontIndexShadowExecutor` composes only host-selected `ProductCatalogReadRuntime` and
`SharedIndexQueryRuntime`. It executes the Product owner list first. Product schema resolution, localized
query build and `execute_localized_query` happen only after owner success, and projected failures cannot
replace the successful authoritative owner result.

For channel-scoped evidence the caller must provide a trusted current slug/UUID pair. The executor checks
presence only; it does not independently prove slug↔UUID correspondence. Channel-less requests remain
projected fail-closed.

## Core PostgreSQL owner-vs-shadow packet — source complete, execution pending

`storefront_shadow_postgres_tests.rs` is a crate-internal opt-in PostgreSQL packet and does not publish a new
production evidence API. It uses:

- current Product routing key `4` through `PRODUCT_SCHEMA_ROUTING_KEY`;
- real Product and Index migrations;
- a real Product channel-visibility relation resolver/freshness path;
- real Product source registry + mutation store materialization;
- persisted Index schema registration and canonical localized query runtime;
- real `ProductCatalogReadRuntime` owner reads;
- the non-serving `ProductStorefrontIndexShadowExecutor`.

The source packet retains these core scenarios:

- requested-locale projection after a **third-locale** title match;
- fallback-locale projection;
- an identity having neither requested nor fallback locale;
- `%` wildcard, `_` wildcard and backslash-escaped `_` behavior;
- one logical Product identity despite multiple physical locale rows;
- exact count;
- equal owner timestamps with both Asc and Desc Product-ID tie-breaks;
- first/second offset page boundaries and `has_next`/`has_more` agreement;
- trusted public-channel membership.

The packet intentionally records one remaining projection adapter gap: when neither requested nor fallback
translation exists, owner Storefront returns public placeholders (`"Untitled product"` and empty handle),
while the generic localized Index projection correctly returns SQL/`IndexValue::Null`. A future serving
adapter must map this null state to the owner placeholders **after** Index page identity/order/count are
fixed. Raw null-vs-placeholder values must not be called field-equivalent.

## Remaining fail-closed parity/evidence gates

1. The new core PostgreSQL packet still needs maintainer execution and review; source presence is not
   evidence admission.
2. Scalar/localized EAV and Select/Multiselect option code/direct UUID/missing-option `Never` need a second
   owner-vs-shadow PostgreSQL packet.
3. Owner title search has no explicit length bound while Index `TextLike` is bounded to 1024 UTF-8 bytes.
4. Owner title `LIKE` uses deployment/default collation while Index String SQL uses deterministic
   `COLLATE "C"`.
5. Channel-less owner requests mean metadata-unrestricted only; current `sales_channel_ids` cannot
   distinguish unrestricted from restricted-to-all-current-channels.
6. Owner page depth is wider than the Index 10,000 offset bound.
7. Taxonomy tag names must be hydrated only after Product page identity/order/count is fixed.
8. Shadow execution has no serving-latency/deadline policy and therefore must remain non-serving.
9. Historical Product PostgreSQL packets still need routing key `4` / current 15-field actualization; never
   add a key-3 runtime compatibility path.

## Next source slice

Add the second retained PostgreSQL packet for Product EAV parity. It should seed real filterable Product
attributes/options and prove owner-vs-shadow behavior for scalar terms, localized requested/fallback text,
Select/Multiselect option code, direct UUID and missing-option `Never`. Keep the core packet and EAV packet
separate so failures identify the owner/query layer precisely.

After those source packets exist, maintainer execution can determine the next correction/admission step.
Search-bound/collation and channel-less/deep-page policy remain explicit gates rather than silent narrowing.

## Source guards

- `verify-product-storefront-attribute-filter-terms.mjs` locks Product-owned EAV resolution;
- `verify-index-product-storefront-shadow-adapter.mjs` locks Product-term → localized query translation;
- `verify-index-product-storefront-shadow-executor.mjs` locks owner-first non-serving execution;
- `verify-index-product-storefront-equivalence-postgres-packet.mjs` locks the current-key core PostgreSQL
  packet and its null-vs-placeholder evidence;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native;
- localized runtime/order/TextLike guards continue to lock generic Index semantics.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
