# M7 Product Storefront Index parity gate

Status: `search_bound_source_complete_collation_evidence_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

The Product-owned EAV resolver, pure localized shadow builder and non-serving owner-first shadow executor are
source-complete. Current-key core/EAV Storefront PostgreSQL packets and the historical retained Product
PostgreSQL packet set are retained in source on Product routing key `4`. They have **not** been executed or
admitted by the implementation agent.

## Product-owned Storefront search bound — source complete

Product now owns `MAX_STOREFRONT_PRODUCT_SEARCH_BYTES = 1022` as the authoritative effective title-search
input bound, measured in UTF-8 bytes after whitespace normalization. The owner SQL wraps the effective search
as `%{search}%`; the two ASCII wildcard bytes make the resulting pattern exactly representable by the generic
Index `TextLike` maximum of 1024 bytes.

`StorefrontProductListQuery::try_new*` validates the normalized input, and
`CatalogService::list_published_products_with_query` validates again immediately before constructing owner
SQL. The second check is deliberate because the query struct has public fields and may be constructed without
its helpers. Over-bound input is rejected; it is never truncated.

The Index shadow builder imports the same Product-owned constant through
`rustok_product::services::MAX_STOREFRONT_PRODUCT_SEARCH_BYTES`. Distribution no longer owns a duplicate
Product-specific pattern bound. A manually constructed over-bound owner query remains fail-closed in shadow
translation. Generic Index validation continues to own the independent 1024-byte `TextLike` pattern contract.

This closes the source-level search-length mismatch only. It does not establish PostgreSQL collation parity.

## Product-owned EAV resolution and shadow execution

`ProductCatalogSchemaReadPort::resolve_storefront_attribute_filters` remains the only Product metadata
boundary used by shadow execution. It preserves mounted owner semantics for typed values, localized
requested/fallback text and Select/Multiselect UUID/code resolution, returning neutral
`ProductAttributeTermExpr` values owned by Product.

Distribution translates those owner-owned expressions into `attribute_terms` predicates. `Never` maps to a
bind-free false predicate using the current Product schema's required/non-null `id` invariant.

`ProductStorefrontIndexShadowExecutor` executes the authoritative Product owner list first. Only after owner
success does it resolve EAV metadata, build `LocalizedEntityQuery` and call `execute_localized_query`.
Projected failures cannot replace the successful owner result. It remains non-serving.

## Core PostgreSQL packet — source complete, execution pending

`storefront_shadow_postgres_tests.rs` uses current Product routing key `4`, real Product/Index migrations,
channel relation freshness, Product source/mutation materialization, persisted Index schema registration,
real Product owner reads and the non-serving shadow executor.

It retains requested/fallback/neither localized projection, third-locale title matching, `%`/`_`/escaped `_`
LIKE behavior, locale identity de-duplication, exact count, Asc/Desc equal-timestamp Product-ID ties, offset
page boundaries and trusted public-channel membership.

It also records the remaining public projection adapter gap: owner uses `"Untitled product"` and empty handle
when neither requested nor fallback translation exists, while generic localized Index returns null. Final
serving projection must map that null state only after Product page identity/order/count are fixed.

## EAV PostgreSQL packet — source complete, execution pending

`storefront_shadow_eav_postgres_tests.rs` is a separate crate-internal opt-in packet on routing key `4`.
It retains nonlocalized integer, localized requested/fallback, Select exact option code, direct option UUID,
Multiselect exact option code and missing/nil option `Never` behavior, including owner/Index identity/count
agreement.

The EAV fixture deliberately does not call `save_product_attribute_values`: Product EAV owner-clock command
semantics have a separate retained gate, so query resolution/materialization failures remain attributable.

## Historical retained Product packets — key 4 source actualized

The retained Product locale-absence, materialized-freshness, Product/Channel convergence, Channel
identity-transition and linked-target recreate/availability/replay packets use current Product routing key
`4` while preserving their scenarios. ProductVariant stays on key `2`, SalesChannel stays on key `1`, and no
Product key-3 compatibility path exists.

`verify-index-product-postgres-key4-fixtures.mjs` locks that boundary. Source actualization does not claim that
the packets pass; execution/review remains a maintainer gate.

## Remaining fail-closed parity/evidence gates

1. Maintainer execution/review of the Storefront core/EAV and actualized retained Product PostgreSQL packets.
2. Owner/default PostgreSQL `pt.title LIKE $1` collation vs Index deterministic `COLLATE "C"`.
3. Channel-less owner requests mean metadata-unrestricted only; current `sales_channel_ids` cannot represent
   that distinction exactly.
4. Owner page depth exceeds the Index 10,000 offset bound.
5. Final Storefront projection must map no-localized-row null title/handle to owner placeholders.
6. Taxonomy tag names must be hydrated only after Product page identity/order/count is fixed.
7. Shadow execution has no serving latency/deadline policy and remains non-serving.
8. Stale locale/readiness/admission/restart cases still require maintainer-executed retained evidence.

## Next source slice

Retain explicit PostgreSQL collation evidence for owner all-translations `pt.title LIKE $1` versus Index
`TextLike` compiled with deterministic `COLLATE "C"`. Include ASCII, non-ASCII/case-sensitive and
wildcard/escape cases. Do not weaken deterministic Index collation merely to match one deployment's database
default; if the owner deployment collation cannot preserve equivalence, keep the cutover fail-closed and make
the deployment/owner contract explicit.

## Source guards

- `verify-product-storefront-attribute-filter-terms.mjs` locks Product-owned EAV resolution;
- `verify-product-storefront-search-bound.mjs` locks the Product-owned 1022-byte effective search bound and
  proves that adding the two owner LIKE wildcards equals the generic 1024-byte Index `TextLike` bound;
- `verify-index-product-storefront-shadow-adapter.mjs` locks Product-term -> localized query translation;
- `verify-index-product-storefront-shadow-executor.mjs` locks owner-first non-serving execution;
- `verify-index-product-storefront-equivalence-postgres-packet.mjs` locks the core current-key packet;
- `verify-index-product-storefront-eav-equivalence-postgres-packet.mjs` locks the EAV current-key packet;
- `verify-index-product-postgres-key4-fixtures.mjs` locks actualized retained Product packets on key `4`;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
