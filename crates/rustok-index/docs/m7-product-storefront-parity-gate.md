# M7 Product Storefront Index parity gate

Status: `shadow_executor_source_complete_postgres_equivalence_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

The Product-owned EAV resolver, pure localized shadow query builder and non-serving owner-first shadow
executor are source-complete. Source completion is not PostgreSQL equivalence evidence and does not admit
Index output for serving.

## Product-owned attribute-filter resolution — source complete

`ProductCatalogSchemaReadPort::resolve_storefront_attribute_filters` is the only Product metadata boundary
used by shadow execution. Existing external adapters remain source-compatible through the fail-closed
default implementation.

The owner resolver preserves mounted list semantics for definition eligibility, typed scalar parsing,
localized requested/fallback text behavior and Select/Multiselect UUID/code resolution. It returns neutral
`ProductAttributeTermExpr` values and Product owns the canonical term grammar. Distribution only translates
those owner-owned expressions into `attribute_terms` Index predicates.

## Shadow query adapter — source complete

`build_product_storefront_index_shadow_query` accepts only `ProductResolvedAttributeFilter`, validates the
resolved filter identities against the authoritative `StorefrontProductListQuery`, and translates
`Term/And/Or/Not/Never` into root Index predicates. `Never` is represented by a bind-free false predicate
using the current Product schema's required/non-null `id` invariant.

The builder maps Active/published-only, trusted current public-channel membership, optional primary
category, canonical EAV terms, any-locale title `TextLike`, requested/fallback title+handle, paired timestamp
order plus matching Product-ID direction, bounded offset pagination and exact count. It remains pure: no DB,
Product service construction or Index execution occurs inside it.

## Non-serving shadow executor — source complete

`ProductStorefrontIndexShadowExecutor` composes only host-selected `ProductCatalogReadRuntime` and
`SharedIndexQueryRuntime`.

Execution order is deliberately owner-first:

1. `ProductCatalogReadPort::list_filtered_published_products` produces the authoritative result;
2. only after owner success, the optional Product schema-read capability resolves EAV filters;
3. the pure shadow builder creates `LocalizedEntityQuery`;
4. `SharedIndexQueryRuntime::execute_localized_query` executes with the existing persisted-readiness,
   owner-admission and one-snapshot localized runtime contract.

If schema resolution, query build, readiness/admission or Index execution fails, that error is retained only
inside `projected`; it cannot replace the successful owner result. The executor is not mounted into
Storefront serving.

For channel-scoped evidence the caller must provide a **trusted current** slug/UUID pair. The executor checks
only that both identities are present and non-empty/non-nil; it does not independently prove slug↔UUID
correspondence. Channel-less requests remain projected fail-closed.

The built-in comparison is intentionally narrow: ordered Product identity list, exact count and page
`has_more` boundary. It does **not** claim scalar/localized projection, tag hydration, search-collation or
full semantic equivalence. Those belong to retained PostgreSQL evidence.

## Remaining fail-closed parity gates

1. Owner title search has no explicit length bound while Index `TextLike` is bounded to 1024 UTF-8 bytes.
2. Owner title `LIKE` uses deployment/default collation while Index String SQL uses deterministic
   `COLLATE "C"`.
3. Channel-less owner requests mean metadata-unrestricted only; current `sales_channel_ids` cannot
   distinguish unrestricted from restricted-to-all-current-channels.
4. Owner page depth is wider than the Index 10,000 offset bound.
5. Taxonomy tag names must be hydrated only after Product page identity/order/count is fixed.
6. Shadow execution has no serving-latency/deadline policy and therefore must remain non-serving.

## Next source slice: retained PostgreSQL equivalence

Add a Product Storefront localized PostgreSQL packet/harness that exercises the actual owner path and the
non-serving shadow executor side-by-side. It must cover at least:

- requested translation, fallback translation and neither requested nor fallback;
- third-locale/all-translations title search plus `%`, `_` and backslash wildcard cases;
- duplicate locale matches yielding one Product identity and exact count;
- scalar and localized EAV terms;
- Select/Multiselect option code, direct UUID and missing-option `Never` behavior;
- trusted public-channel membership;
- equal timestamps under both Asc and Desc Product-ID tie-breaks;
- pagination and exact count;
- stale locale exclusion, readiness/admission failure and replay/restart behavior;
- explicit search-bound/collation evidence rather than silently declaring parity.

Historical Product PostgreSQL packets still need routing key `4` / current 15-field actualization. Never add
a key-3 runtime compatibility path.

Taxonomy tag hydration must be retained only after the Product page is fixed and must not change Product
identity/order/count.

## Source guards

- `verify-product-storefront-attribute-filter-terms.mjs` locks Product-owned EAV resolution;
- `verify-index-product-storefront-shadow-adapter.mjs` locks Product-term → localized query translation;
- `verify-index-product-storefront-shadow-executor.mjs` locks owner-first non-serving execution and forbids
  direct DB/service/PostgreSQL-port construction;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native;
- localized runtime/order/TextLike guards continue to lock generic Index semantics.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
