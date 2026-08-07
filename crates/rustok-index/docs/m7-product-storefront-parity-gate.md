# M7 Product Storefront Index parity gate

Status: `source_gap_locale_search_and_fallback_adapter_evidence_pending`.

## Purpose

The Product Storefront catalog remains owner-native. The single current Product Index source now
materializes the Storefront scalar fields, stable tag identities, and typed EAV query state that were
previously missing, but a fresh owner-vs-Index recheck found a remaining **localized Product identity
mismatch**. Source coverage must not be described as complete until that boundary is resolved.

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

This is correct for the generic localized Index source contract, but it is not yet equivalent to the
owner Storefront list semantics above.

## Remaining locale/search source gap

A scalar substring/LIKE operator alone cannot close Storefront parity:

- searching only the current Index entity's `title` would miss owner matches that exist in another
  translation;
- querying only the requested locale would omit Products for which the owner list returns a fallback
  translation;
- issuing an independent fallback query after the requested query would not automatically preserve one
  owner-equivalent global sort, page boundary, exact count, and de-duplication contract.

Therefore the next Storefront query work must first choose and prove one explicit architecture for
**effective localized Product list identity**. Acceptable designs must preserve owner semantics without
adding another parallel Product compatibility schema. Examples include a generic bounded localized
fallback/union query capability or an explicitly changed owner Storefront contract, but no choice is
claimed by this document yet.

Do **not** add another Product routing key merely to patch this query semantic. The current Product
contract remains the only runtime Product implementation while this query-layer decision is pending.

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

1. resolve the effective localized Product identity/search/fallback architecture described above;
2. add any required generic text-pattern/query primitive only after that architecture is fixed;
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

## Source guard

`scripts/verify/verify-index-product-storefront-parity-gate.mjs` intentionally locks both sides of the
current mismatch:

- owner title search remains all-translations unless the same PR deliberately changes the owner
  contract;
- owner result projection still has requested/fallback translation selection;
- Product Index source remains one physical translation locale per entity and does not fabricate
  fallback rows;
- native Storefront traffic has not silently switched to Index;
- one current 15-field Product schema and schema-scoped replay identity remain selected;
- cutover remains fail-closed.

## Deliberate limits

This slice does not choose a new locale-union/fallback architecture, change owner Storefront semantics,
add another Product routing key, implement the Storefront Index adapter, execute tenant rebuild, add
public typed Product events, or switch traffic.

## Maintainer verification

```bash
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
