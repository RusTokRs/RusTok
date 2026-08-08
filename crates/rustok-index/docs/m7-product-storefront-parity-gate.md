# M7 Product Storefront Index parity gate

Status: `collation_packet_source_complete_execution_and_visibility_pending`.

## Current boundary

Mounted Storefront remains owner-native and continues to execute
`CatalogService::list_published_products_with_query`. No Index traffic switch is part of this state.

The Product-owned EAV resolver, pure localized shadow builder and non-serving owner-first shadow executor are
source-complete. Current-key core/EAV Storefront PostgreSQL packets, a focused title-search collation packet,
and the historical retained Product PostgreSQL packet set are retained in source on Product routing key `4`.
They have **not** been executed or admitted by the implementation agent.

## Product-owned Storefront search bound — source complete

Product owns `MAX_STOREFRONT_PRODUCT_SEARCH_BYTES = 1022` as the authoritative effective title-search input
bound, measured in UTF-8 bytes after whitespace normalization. The owner wraps the effective search as
`%{search}%`; the two wildcard bytes make the resulting pattern exactly representable by the generic Index
`TextLike` maximum of 1024 bytes.

`StorefrontProductListQuery::try_new*` validates the normalized input, and
`CatalogService::list_published_products_with_query` validates again before owner SQL construction so direct
public query-struct construction cannot bypass the contract. Over-bound input is rejected, never truncated.
The shadow builder imports the same Product-owned constant.

## Title-search collation packet — source complete, execution pending

`product_storefront_search_collation_postgres.rs` is a focused opt-in PostgreSQL packet that observes the
actual database/default Product title collation rather than manufacturing parity.

It runs real Product migrations, seeds the real `product_translations.title` column and compares, for the same
`%{search}%` patterns:

- owner-equivalent `translation.title LIKE $2` using the database/default collation and PostgreSQL default
  backslash escape;
- Index-equivalent `(translation.title COLLATE "C") LIKE $2 ESCAPE E'\\'` using the deterministic collation
  contract locked by the localized Index compiler.

The retained matrix covers ASCII upper/lower case, NFC and NFD Unicode forms, `_` and `%` wildcards, escaped
underscore/percent literals, and `straße` versus ASCII `STRASSE`. The packet records `lc_collate` in a mismatch
diagnostic and fails if the owner/default identity set differs from the `C` identity set.

This packet does **not** mutate the deployment collation, create a test collation, add `ILIKE`, lower-case the
owner field or weaken Index `COLLATE "C"`. Source presence is not collation admission; a maintainer-run packet
must agree for a deployment before this gate can be closed there.

## Product-owned EAV resolution and shadow execution

`ProductCatalogSchemaReadPort::resolve_storefront_attribute_filters` remains the Product metadata boundary
used by shadow execution. It preserves typed values, localized requested/fallback text and Select/Multiselect
UUID/code resolution, returning neutral `ProductAttributeTermExpr` values owned by Product.

Distribution translates those expressions into `attribute_terms` predicates. `Never` maps to a bind-free
false predicate using the current Product schema's required/non-null `id` invariant.

`ProductStorefrontIndexShadowExecutor` executes the authoritative Product owner list first. Only after owner
success does it resolve EAV metadata, build `LocalizedEntityQuery` and call `execute_localized_query`.
Projected failures cannot replace the successful owner result. It remains non-serving.

## Core and EAV PostgreSQL packets — source complete, execution pending

`storefront_shadow_postgres_tests.rs` retains requested/fallback/neither localized projection, third-locale
title matching, wildcard behavior, locale identity de-duplication, exact count, Asc/Desc equal-timestamp ties,
offset page boundaries and trusted public-channel membership on current Product key `4`.

It also records the public projection gap: when neither requested nor fallback translation exists, owner uses
`"Untitled product"` and empty handle while generic localized Index returns null. Final serving projection must
map that state only after Product page identity/order/count are fixed.

`storefront_shadow_eav_postgres_tests.rs` separately retains nonlocalized integer, localized
requested/fallback, Select code, direct option UUID, Multiselect code and missing/nil option `Never` behavior.

## Historical retained Product packets — key 4 source actualized

The retained Product locale-absence, materialized-freshness, Product/Channel convergence, Channel
identity-transition and linked-target recreate/availability/replay packets use current Product routing key
`4`. ProductVariant stays on key `2`, SalesChannel stays on key `1`, and no Product key-3 compatibility path
exists. Execution/review remains a maintainer gate.

## Remaining fail-closed parity/evidence gates

1. Maintainer execution/review of the Storefront core/EAV, collation and actualized retained Product packets.
2. Collation admission per deployment: any owner/default-vs-`C` matrix difference keeps cutover fail-closed.
3. Channel-less owner requests mean metadata-unrestricted only; current `sales_channel_ids` cannot represent
   that distinction exactly.
4. Owner page depth exceeds the Index 10,000 offset bound.
5. Final Storefront projection must map no-localized-row null title/handle to owner placeholders.
6. Taxonomy tag names must be hydrated only after Product page identity/order/count is fixed.
7. Shadow execution has no serving latency/deadline policy and remains non-serving.
8. Stale locale/readiness/admission/restart cases still require maintainer-executed retained evidence.

## Next source slice

Resolve channel-less visibility without inferring unrestricted metadata from resolved UUID membership. A
restricted Product may currently contain every channel and therefore have the same `sales_channel_ids` as an
unrestricted Product. Prefer a Product-owned materialized visibility identity/capability with the same
freshness fence as channel membership, and keep channel-less shadow execution fail-closed until the owner
semantic is representable exactly.

## Source guards

- `verify-product-storefront-attribute-filter-terms.mjs` locks Product-owned EAV resolution;
- `verify-product-storefront-search-bound.mjs` locks the Product-owned search-length contract;
- `verify-index-product-storefront-collation-postgres-packet.mjs` locks owner/default-vs-Index-`C` collation
  evidence without manufacturing a favorable collation;
- `verify-index-product-storefront-shadow-adapter.mjs` locks Product-term -> localized query translation;
- `verify-index-product-storefront-shadow-executor.mjs` locks owner-first non-serving execution;
- `verify-index-product-storefront-equivalence-postgres-packet.mjs` locks the core current-key packet;
- `verify-index-product-storefront-eav-equivalence-postgres-packet.mjs` locks the EAV current-key packet;
- `verify-index-product-postgres-key4-fixtures.mjs` locks actualized retained Product packets on key `4`;
- `verify-index-product-storefront-parity-gate.mjs` keeps mounted Storefront owner-native.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
