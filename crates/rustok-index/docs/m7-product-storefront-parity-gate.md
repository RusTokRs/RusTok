# M7 Product Storefront Index parity gate

Status: `shadow_query_adapter_source_complete_owner_resolution_and_evidence_pending`.

## Purpose

The mounted Product Storefront catalog remains owner-native and must continue to execute
`CatalogService::list_published_products_with_query`.

The Index side is now source-complete through localized identity folding, bounded `TextLike`, explicit
Asc/Desc Product-ID tie-break direction, PostgreSQL runtime execution, and a **pure shadow/evidence query
builder** in `rustok-distribution`. That builder translates only owner inputs whose semantics are already
representable. It is not wired to Storefront traffic and does not execute Index queries itself.

## Owner contract still authoritative

The owner list enforces tenant scope, Active status, non-null `published_at`, Product channel visibility,
optional primary category, typed Product EAV filters, all-translations title `LIKE`, exact count,
requested/fallback Product translation projection, localized Taxonomy tag names and stable ordering.

Owner ordering uses two timestamp terms plus Product ID in the same direction:

- `published_at, created_at, id`; or
- `created_at, published_at, id`.

The localized Index compiler now has an explicit `identity_order_direction`, so the shadow builder can map
owner Asc/Desc to both timestamp terms and the final Product-ID tie-break.

## Shadow query adapter — source complete

`crates/rustok-distribution/src/product_index/storefront_shadow.rs` owns
`build_product_storefront_index_shadow_query`.

For currently representable requests it maps:

- Product status -> `Eq(status, "active")`;
- published-only -> `IsNull(published_at, false)`;
- a trusted current public channel UUID -> `Contains(sales_channel_ids, channel_id)`;
- optional primary category -> `Eq(primary_category_id, category_id)`;
- Product-owned resolved EAV predicates -> canonical `attribute_terms` predicates only;
- title search -> folded `TextLike(title, "%...%")` in `any_locale_filter`;
- localized output -> requested/fallback `title` and `handle`;
- owner public fields -> Product root projection including `tag_ids` for later Taxonomy hydration;
- owner timestamp order -> the same two timestamp fields/direction;
- owner Product-ID tie-break -> the same localized identity direction;
- owner page/per-page -> bounded Index offset pagination;
- exact count -> enabled.

The builder points only at the current Product routing key `4`. It is crate-visible for retained evidence
harnesses, not public API.

## Pure boundary: no owner bypass

The shadow builder does not import or construct `DatabaseConnection`, `CatalogService`,
`ProductCatalogSchemaService`, Product EAV tables, or the Index execution runtime. It only converts an
already validated owner query plus Product-owned resolved canonical EAV predicates into
`LocalizedEntityQuery`.

This is intentional. Product attribute definitions can already be read through
`ProductCatalogSchemaReadPort`, but current owner read capabilities do not expose the option-code/option-ID
resolution required to reproduce Select/Multiselect filters without direct Product table access.

The next Product-owned capability must resolve each public `ProductAttributeFilter` into the same canonical
Index term predicate used by the Product source. Only then may the shadow executor compose metadata
resolution with this query builder.

## Explicit fail-closed parity gaps

The builder refuses to pretend parity where the owner contract is wider than the current Index query
contract:

1. **Channel-less visibility** — owner `None` channel means metadata-unrestricted only. Current
   `sales_channel_ids` materializes unrestricted as membership in all current channels, so it cannot
   distinguish unrestricted from a restricted Product that currently contains every channel. A public
   channel UUID is therefore mandatory for the shadow builder.
2. **Deep page offsets** — owner page is not upper-bounded, while Index offset depth is capped at 10,000.
   The builder returns `OffsetTooDeep` instead of silently changing the page.
3. **Search length** — owner search has no explicit length bound; `TextLike` is capped at 1024 UTF-8 bytes.
   The builder rejects an over-bound pattern rather than truncating it.
4. **Search collation** — owner title `LIKE` uses deployment/default collation while Index String SQL uses
   deterministic `COLLATE "C"`. Source inspection does not claim equivalence.
5. **Public EAV option resolution** — Select/Multiselect public option codes still require a Product-owned
   code-to-ID resolver before canonical `attribute_terms` can be built.

These are evidence/capability gates, not reasons to weaken the owner contract.

## Taxonomy boundary

Product Index stores stable `tag_ids`, not localized Taxonomy names. The shadow query selects those IDs,
but localized Taxonomy hydration remains a separate post-page owner boundary. Product page selection must
be fixed before tag-name hydration so Taxonomy work cannot change Product pagination or exact count.

## Current routing/replacement boundary

Product runtime still publishes exactly one current 15-field Product schema on internal routing key `4`.
Lower persisted Product keys remain historical only. Do not add a key-3 compatibility implementation to
make retained evidence pass.

## Remaining work before traffic cutover

1. add Product-owned canonical Storefront attribute-filter resolution, including option-code -> option-ID;
2. compose a shadow executor over that resolver + `execute_localized_query` without mounting it in
   Storefront traffic;
3. add retained owner-vs-Index PostgreSQL equivalence for requested/fallback/third-locale projection,
   title wildcard behavior, category, EAV, channel membership, Asc/Desc equal-timestamp ties, pagination,
   exact count, stale locale exclusion, readiness and restart;
4. resolve/admit search-length and collation parity;
5. resolve channel-less unrestricted visibility or keep that request shape owner-native;
6. decide the authoritative policy for owner pages beyond the Index offset bound;
7. batch-hydrate localized Taxonomy tags after the shadow Product page;
8. actualize historical Product PostgreSQL packets to routing key `4` / current 15-field contract;
9. execute/admit current replacement evidence and stage/rebuild/promote Product key `4`;
10. only then consider Storefront traffic selection.

## Source guards

- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native;
- `verify-index-product-storefront-localized-query-architecture.mjs` locks fold/search/order semantics;
- `verify-index-product-storefront-shadow-adapter.mjs` locks the pure fail-closed shadow translation;
- `verify-index-localized-query-runtime.mjs` locks readiness/admission/one-snapshot execution;
- `verify-index-localized-identity-order.mjs` locks owner-compatible Product-ID tie-break direction;
- `verify-index-text-like-filter.mjs` locks generic bounded LIKE semantics.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-storefront-shadow-adapter.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-product-storefront-localized-query-architecture.mjs
node scripts/verify/verify-index-localized-query-runtime.mjs
node scripts/verify/verify-index-localized-identity-order.mjs
node scripts/verify/verify-index-text-like-filter.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
