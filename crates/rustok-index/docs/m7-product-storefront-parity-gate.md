# M7 Product Storefront Index parity gate

Status: `source_complete_query_adapter_and_evidence_pending`.

## Purpose

The Product Storefront catalog remains owner-native until a retained owner-vs-Index equivalence packet
is executed and admitted. The single current Product Index source now materializes the Storefront fields
and typed EAV query state that were previously missing, but source coverage alone does not authorize a
traffic cutover.

Storefront must continue to execute `CatalogService::list_published_products_with_query` until the
remaining adapter, tag hydration, staged rebuild/readiness, and equivalence gates are complete.

## Owner Storefront contract

`StorefrontProductListQuery` accepts:

- optional title search;
- optional primary category UUID;
- sort by `published_at` or `created_at`;
- ascending or descending direction;
- up to eight typed `code=value` Product attribute predicates;
- page/per-page pagination.

Owner execution additionally enforces tenant scope, Active status, non-null `published_at`, public
SalesChannel visibility, category/title filtering, typed EAV predicates, stable timestamp+ID ordering,
exact count, Product translation fallback, and localized tag projection.

The result payload returns `id`, `status`, `title`, `handle`, `seller_id`, `vendor`, `product_type`,
localized tag names, `created_at`, and `published_at`.

## Single current Product Index coverage

Current Product runtime code publishes one Product schema with 15 fields and two links:

- existing graph/content fields: `id`, `status`, `title`, `handle`, `description`, `vendor`,
  `product_type`, `primary_category_id`, `variant_ids`, `sales_channel_ids`;
- Storefront scalar fields: `seller_id`, `created_at`, `published_at`;
- stable Taxonomy tag identities: `tag_ids`;
- typed EAV query machinery: `attribute_terms`;
- links `variants` and `sales_channels`.

`created_at` and `published_at` are sortable. `published_at` is nullable/filterable so published-only
admission can be represented explicitly. `attribute_terms` is a non-selectable filterable
`Many<String>` field using the canonical Product attribute-term grammar.

The Product source still uses `projection_epoch` as its complete mutation clock and retains existing
Product/SalesChannel freshness plus linked-target availability rules.

## Tags remain Taxonomy-owned

Index stores stable `product_tags.term_id` UUIDs, not localized Taxonomy names. Copying Taxonomy
translations into Product Index would create a second freshness clock that Product does not own.

A Storefront Index adapter must therefore hydrate localized tag names from Taxonomy after Product page
selection, preserving the same requested/fallback locale behavior as the owner path. That hydration must
be bounded/batched and included in parity evidence before cutover.

## Typed EAV representation

Dynamic EAV attributes do not create dynamic Index fields. Public Product attribute/option codes are
resolved through Product owner metadata to stable attribute/option UUIDs; the adapter then builds
`Contains(attribute_terms, term)` expressions.

Localized text uses the exact owner predicate:

`requested-value OR (NOT requested-present AND fallback-value)`

See `m7-product-attribute-term-contract.md` for the current grammar and normalization.

## Immutable replacement boundary

The old Product fingerprint was not modified in place. Current runtime code uses one monotonically
higher internal Product routing key, derives replay IDs with `derive_index_schema_source_event_id`, and
publishes no lower Product compatibility implementation.

Lower persisted Product keys are historical storage identities only. Promotion remains an operator and
evidence action:

1. ordinary-register the current key;
2. rebuild/replay the current key completely;
3. prove exact schema readiness, parity, freshness, restart, and inbox isolation;
4. call `PostgresSchemaRegistrationStore::register_current` to retire lower active persisted keys;
5. only then allow authoritative Storefront selection.

There is no same-key fingerprint replacement and no parallel Product v4/v5 route.

## Remaining source/evidence work before cutover

The schema/source gap is closed, but Storefront traffic stays blocked until all of these are complete:

1. translate `StorefrontProductListQuery` to the current `IndexQuery` contract;
2. resolve Product attribute and option codes to canonical term predicates;
3. preserve Active + published-only admission, title/category filters, both sort keys/directions, stable
   ID tie-break, pagination, and exact count;
4. decode the current Product projection into the public Storefront result;
5. batch-hydrate localized tag names through Taxonomy with requested/fallback parity;
6. retain and execute PostgreSQL owner-vs-Index equivalence for the full matrix, including locale,
   visibility, typed EAV, ordering, pagination, exact count, stale linked targets, and restart;
7. stage/rebuild/promote the current Product key for the tenant before traffic selection.

## Source guard

`scripts/verify/verify-index-product-storefront-parity-gate.mjs` checks that:

- native Storefront traffic has not silently switched to Index;
- the owner query/DTO contract remains explicit;
- the single current Product schema contains the required Storefront fields and canonical EAV terms;
- Product replay identity is schema-scoped;
- old Product runtime compatibility branches are absent;
- same-key schema replacement remains rejected and staged supersession remains available;
- cutover remains query-adapter/evidence gated.

## Deliberate limits

This source slice does not execute tenant rebuild/supersession, implement the Storefront Index adapter,
hydrate Taxonomy tags through Index traffic, add public typed Product event contracts, or switch traffic.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-attribute-term-contract.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-schema-supersession.mjs
node scripts/verify/verify-index-query-contract.mjs
node scripts/verify/verify-product-storefront-boundary.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
