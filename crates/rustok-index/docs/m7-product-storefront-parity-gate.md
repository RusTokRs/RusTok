# M7 Product Storefront Index parity gate

Status: `source_architecture_selected_implementation_and_evidence_pending`.

## Purpose

The Product Storefront catalog remains owner-native. The single current Product Index source now
materializes the Storefront scalar fields, stable tag identities, and typed EAV query state that were
previously missing. A fresh owner-vs-Index recheck found a remaining **localized Product identity
mismatch**; the query-layer architecture for that mismatch is now selected, but implementation and
retained evidence are still pending.

Storefront must continue to execute `CatalogService::list_published_products_with_query`.

## Owner Storefront contract

`StorefrontProductListQuery` accepts optional title search/category, `published_at` or `created_at`
sorting in either direction, up to eight typed Product EAV predicates, and page/per-page pagination.

Owner execution additionally enforces tenant scope, Active status, non-null `published_at`, public
SalesChannel visibility, exact count, stable timestamp+ID ordering, Product translation fallback, and
localized tag projection.

Two translation details are authoritative today:

1. `product_title_search_condition` uses an `EXISTS` over `product_translations` for the Product and does
   **not** restrict that search row to the requested or fallback locale. A title match in any Product
   translation can therefore admit the Product.
2. Result projection calls `pick_product_translation(items, requested, fallback)`. If the requested
   translation is absent, the owner list can still return the Product using its fallback translation.

The public item then returns `id`, `status`, effective `title`, effective `handle`, `seller_id`, `vendor`,
`product_type`, localized tag names, `created_at`, and `published_at`.

## Single current Product Index coverage

Current Product runtime code publishes one Product schema with 15 fields and two links:

- identity/content: `id`, `status`, `title`, `handle`, `description`;
- Storefront owner scalars: `seller_id`, `vendor`, `product_type`, `primary_category_id`, `created_at`,
  `published_at`;
- stable Taxonomy identities: `tag_ids`;
- typed EAV query machinery: `attribute_terms`;
- graph membership: `variant_ids`, `sales_channel_ids`;
- links `variants` and `sales_channels`.

The source emits **one Index entity for each physically stored `product_translations.locale`**. It does
not fabricate a requested-locale row when that translation is absent. The Product absence provider
correctly reports that exact locale identity as absent; it is not a Storefront fallback synthesizer.

This physical storage model remains correct. Storefront equivalence is resolved at query composition,
not by introducing another Product schema or fabricating locale rows during replay.

## Selected locale/search architecture

A scalar substring/LIKE operator alone cannot close Storefront parity:

- searching only the current Index entity's `title` would miss owner matches that exist in another
  translation;
- querying only the requested locale would omit Products for which the owner list returns a fallback
  translation;
- issuing an independent fallback query after the requested query would not automatically preserve one
  owner-equivalent global sort, page boundary, exact count, and de-duplication contract.

The selected design is a generic **localized-entity identity fold** in the Index query layer. The
physical Index rows remain locale-keyed, but the folded Storefront page is grouped by Product entity
identity before exact count and pagination. Any current/admitted locale row may satisfy title search;
result localization is selected independently as requested locale, then fallback locale, then no
localized row. Existing schema readiness, Product freshness, and queried link-target availability remain
mandatory for every row participating in the fold.

Ordinary exact-locale `IndexQuery` remains unchanged. A consumer must not approximate the folded contract
by independently paging multiple locales and merging them afterward.

See `m7-product-storefront-localized-query-architecture.md` for the identity, search, projection,
ordering, cursor, freshness, and retained-evidence contract.

Do **not** add another Product routing key merely to patch this query semantic. The current Product
contract remains the only runtime Product implementation.

## Typed EAV representation

Dynamic EAV attributes do not create dynamic Index fields. Public Product attribute/option codes are
resolved through Product owner metadata to stable UUIDs; the adapter then builds
`Contains(attribute_terms, term)` predicates.

Localized text EAV uses the exact owner predicate:

`requested-value OR (NOT requested-present AND fallback-value)`

See `m7-product-attribute-term-contract.md`.

## Tags remain Taxonomy-owned

Index stores stable `product_tags.term_id` UUIDs, not localized Taxonomy names. A future Storefront Index
adapter must batch-hydrate requested/fallback tag names from Taxonomy after Product page selection. That
boundary remains evidence-gated and does not require Product to own Taxonomy translation freshness.

## Immutable replacement boundary

The previous Product fingerprint was not modified in place. Current runtime code publishes one higher
internal routing key, derives replay IDs with `derive_index_schema_source_event_id`, and publishes no
lower Product compatibility implementation.

Lower persisted Product keys are historical storage identities only. Promotion remains staged:

1. ordinary-register the current key;
2. rebuild/replay it completely;
3. prove exact readiness, freshness, inbox isolation, and query parity;
4. call `PostgresSchemaRegistrationStore::register_current` to retire lower active persisted keys;
5. only then allow an authoritative consumer cutover.

## Remaining work before Storefront cutover

Storefront traffic stays blocked until all of these are complete:

1. implement the selected generic localized-entity identity fold without changing ordinary exact-locale
   `IndexQuery` semantics;
2. add the required generic scalar text-pattern primitive inside the folded any-locale identity
   predicate;
3. translate Active + published-only, category, visibility, EAV, timestamp ordering, ID tie-break,
   pagination, and exact count to Index;
4. resolve Product attribute/option codes to canonical EAV terms;
5. batch-hydrate localized Taxonomy tag names;
6. actualize retained Product PostgreSQL packets to the current Product routing key/source contract;
7. retain and execute full owner-vs-Index equivalence for requested locale present/absent, fallback,
   cross-locale title matches, EAV, visibility, ordering, pagination, exact count, target lag, and
   restart;
8. stage/rebuild/promote the current Product key for the tenant;
9. only then select Index traffic.

## Source guards

`scripts/verify/verify-index-product-storefront-parity-gate.mjs` locks both sides of the current physical
mismatch and keeps Storefront owner-native.

`scripts/verify/verify-index-product-storefront-localized-query-architecture.mjs` locks the selected
query-layer decision:

- owner title search remains all-translations unless the same PR deliberately changes the owner
  contract;
- owner result projection still has requested/fallback translation selection;
- Product Index source remains one physical translation locale per entity and does not fabricate
  fallback rows;
- one current Product routing key (`4`) and schema-scoped replay identity remain selected;
- the localized fold is identity-level and happens before page/count semantics;
- implementation/evidence remain pending and traffic stays fail-closed.

## Deliberate limits

This slice selects and documents the localized query architecture but does not implement the fold, change
owner Storefront semantics, add another Product routing key, implement the Storefront Index adapter,
execute tenant rebuild, add public typed Product events, or switch traffic.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-storefront-localized-query-architecture.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-attribute-term-contract.mjs
node scripts/verify/verify-index-query-contract.mjs
node scripts/verify/verify-product-storefront-boundary.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
