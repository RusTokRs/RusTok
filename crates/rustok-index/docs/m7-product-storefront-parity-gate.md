# M7 Product Storefront Index parity gate

Status: `source_complete_cutover_blocked_by_contract_gap`.

## Purpose

The Product Storefront catalog remains owner-native. This gate prevents an Index traffic cutover from
being described as ready merely because the canonical Product graph can answer basic Product, Variant,
and SalesChannel queries.

The current Storefront list contract and the current immutable Product Index schema do **not** have full
query/result parity. Until that gap is closed through one deliberately admitted current contract, the
Storefront must continue to execute `CatalogService::list_published_products_with_query` and must not
silently switch to `SharedIndexQueryRuntime`.

This slice changes no traffic, Product schema, schema key, migration, query semantics, owner clock, or
compatibility route.

## Current owner Storefront contract

`StorefrontProductListQuery` accepts:

- optional title search;
- optional primary category UUID;
- sort by `published_at` or `created_at`;
- ascending or descending direction;
- up to eight typed `code=value` Product attribute predicates;
- page/per-page pagination.

`CatalogService::list_published_products_with_query` additionally enforces:

- tenant scope;
- `ProductStatus::Active`;
- non-null `published_at`;
- public SalesChannel visibility from Product owner metadata;
- category filtering;
- title search over Product translations;
- typed EAV attribute filtering through active Product-scoped filterable definitions;
- stable `published_at/created_at/id` ordering;
- exact total before page slicing;
- locale/fallback translation selection;
- owner tag projection.

The public `StorefrontProductListItem` returns:

- `id`;
- `status`;
- `title`;
- `handle`;
- `seller_id`;
- `vendor`;
- `product_type`;
- `tags`;
- `created_at`;
- `published_at`.

## Current canonical Product Index coverage

The selected Product Index contract currently has exactly one runtime Product schema and exposes:

- `id`;
- `status`;
- `title`;
- `handle`;
- `description`;
- `vendor`;
- `product_type`;
- `primary_category_id`;
- `variant_ids`;
- `sales_channel_ids`;
- links `variants` and `sales_channels`.

That contract already covers tenant/locale scoping, current Product authority, title/category filtering,
Product-to-SalesChannel visibility membership, exact count, linked target freshness/availability, and
keyset/offset query execution.

It does **not** currently materialize these Storefront list requirements:

- `seller_id` result payload;
- Product `tags` result/filter semantics;
- `created_at` result + sort key;
- `published_at` result + non-null published-only admission + sort key;
- dynamic typed EAV `attribute_filters`.

Therefore current Index rows cannot reproduce the owner Storefront list contract exactly.

## Why the missing fields cannot be appended silently

`PostgresSchemaRegistrationStore` treats one `(tenant, module, entity, schema_version)` contract as
immutable. Re-registering the same `SchemaRef` with a different fingerprint or JSON contract returns
`VersionConflict`; inserting a non-increasing schema version returns `NonMonotonicVersion`.

The current Product routing key is already persisted and intentionally represents the one selected
canonical Product contract. Appending Storefront-only fields under that same key would create
fingerprint drift and fail schema readiness. It would not be a safe "current code only" change.

Conversely, adding a `Product v4` compatibility branch only to make Storefront work would violate the
single-current-contract direction. This gate therefore rejects both shortcuts:

- no silent same-key fingerprint replacement;
- no parallel v4/v5 compatibility source/route.

## Single-current replacement persistence is source complete

Generic Index persistence now has an explicit replacement primitive documented in
`m4-single-current-schema-supersession.md`:

`PostgresSchemaRegistrationStore::register_current`.

It atomically registers/resolves one monotonically higher current routing key for an exact
tenant/module/entity identity and marks every lower active persisted key `retired` in the same
transaction. Ordinary `register` does not gain this behavior.

Retirement does not rewrite or delete historical entity/link/inbox/replay rows. Their immutable schema
rows remain for foreign-key integrity, while exact persisted readiness and query execution reject a
retired schema as non-authoritative.

This solves the persistence-side **single-current** replacement mechanism without adding a parallel
runtime compatibility branch. It does not by itself expand Product or make a new Product key ready.

## Cutover admission rule

Product Storefront Index cutover remains **false** until one future Product replacement proves all of the
following:

1. one current Product Index contract covers every Storefront result field and ordering/filter input
   that is claimed by the cutover;
2. typed Product attribute predicates have an admitted Index representation and exact owner parity, or
   the Storefront contract explicitly stops claiming those filters before cutover;
3. the new immutable Product contract is installed through explicit single-current supersession and a
   complete new-key rebuild/replay, not same-key fingerprint replacement;
4. every lower persisted Product key is retired and no old Product schema is runtime-selected in
   parallel;
5. exact tenant schema readiness succeeds for the new current key after rebuild;
6. retained PostgreSQL equivalence compares owner-native and Index results for locale fallback,
   visibility, search, category, typed attributes, both sort keys/directions, pagination, exact count,
   and linked-target lag;
7. only after those checks may the Storefront transport select Index as an authoritative provider.

Until then, owner-native PostgreSQL remains authoritative for the catalog list.

## Source guard

`scripts/verify/verify-index-product-storefront-parity-gate.mjs` intentionally checks the current gap as
well as the current traffic boundary. If Product Index coverage changes, the same PR must update this
parity document/guard instead of silently making the guard pass by removing requirements.

The guard verifies that:

- Storefront native transport still invokes the Product owner service and does not import Index runtime
  query types;
- owner query/DTO source still exposes the required controls/result fields listed above;
- the current Product Index schema still lacks the known missing Storefront fields;
- same-key schema fingerprint reuse remains rejected by generic Index registration;
- the current Index implementation plan keeps Storefront traffic cutover evidence-gated.

## Deliberate limits

This slice does not change the Product routing key or Product fields. It does not purge persisted Product
Index state, invent a second Storefront Product entity, duplicate EAV data, weaken schema
fingerprint/readiness checks, or switch traffic.

The next Product schema replacement must use one higher internal routing key as a storage identity while
publishing only that one current Product contract in runtime code. The old key is historical storage, not
a compatibility implementation.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
node scripts/verify/verify-index-schema-supersession.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
node scripts/verify/verify-product-storefront-boundary.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.