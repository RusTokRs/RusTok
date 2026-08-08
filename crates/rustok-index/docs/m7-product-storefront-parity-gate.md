# M7 Product Storefront Index parity gate

Status: `localized_runtime_and_text_pattern_source_complete_adapter_and_evidence_pending`.

## Purpose

The Product Storefront catalog remains owner-native. The current Product Index source now has the
Storefront scalar/EAV state, the localized identity fold, PostgreSQL compiler/decoder, fail-closed runtime,
and a generic bounded scalar String `TextLike` operator usable inside folded `any_locale_filter`.

The Product Storefront adapter and retained owner-vs-Index PostgreSQL evidence are still pending. Source
completion is not traffic-cutover evidence.

Storefront must continue to execute `CatalogService::list_published_products_with_query`.

## Owner Storefront contract

`StorefrontProductListQuery` accepts optional title search/category, `published_at` or `created_at`
sorting in either direction, up to eight typed Product EAV predicates, and page/per-page pagination.
Owner execution additionally enforces tenant scope, Active status, non-null `published_at`, public
SalesChannel visibility, exact count, stable timestamp+ID ordering, Product translation fallback, and
localized tag projection.

Two translation details remain authoritative:

1. `product_title_search_condition` uses an `EXISTS` over `product_translations` and does **not** restrict
   the matching row to requested/fallback locale;
2. result projection uses `pick_product_translation(items, requested, fallback)`.

The title helper builds `format!("%{search}%")` and executes `pt.title LIKE $1`. Current Product list input
normalizes whitespace but does not impose an explicit search-length bound.

## Single current Product Index coverage

Current Product runtime code publishes one Product schema with 15 fields and two links. Product Index emits
one physical entity per stored translation locale and does not fabricate requested/fallback rows. The
physical model remains correct; Storefront equivalence is handled by the query fold rather than another
Product schema or routing key.

Do **not** add another Product routing key merely to patch localized Storefront query semantics. The
current Product contract remains the only runtime Product implementation.

## Localized identity fold — source complete

The generic fold preserves logical result identity `(tenant_id, schema_ref, entity_id)` while physical
storage remains locale-keyed.

`LocalizedEntityQuery` provides requested locale through `query.scope.locale`, canonical fallback,
root-only `any_locale_filter`, and explicit `localized_projection_fields` for requested -> fallback -> null
output semantics. `LocalizedCursorCodec` uses dedicated scoped wire version `3`; ordinary exact-locale
cursors remain on version `2`.

The initial fold deliberately rejects linked query paths. Current Product Storefront list identity
predicates can still use root materialized fields such as `sales_channel_ids` and `attribute_terms`.

## PostgreSQL compiler/decoder — source complete

`SchemaRegistry::compile_postgres_localized_page_query` uses canonical physical aliases `t0` anchor, `t1`
requested row, `t2` fallback row, `t3` any-locale predicate row and `t4` lower-locale anti-duplicate
candidate.

All physical roles retain `is_deleted = FALSE` so generic `PostgresQueryEntityAdmission` can inject owner
freshness. De-duplication happens before ordering/lookahead/limit/exact-count. Requested/fallback projection
uses row-presence `CASE`; exact count is independent of requested/fallback projection availability.

`SchemaRegistry::decode_postgres_localized_query_page` verifies ordinary/localized plan identity,
column/count/lookahead contracts, allows extra SQL-null only for explicit localized projection fields, and
emits localized continuation cursors.

## PostgreSQL runtime — source complete

`IndexQueryPort` exposes explicit `execute_localized_query` with a fail-closed default.
`SharedIndexQueryRuntime` forwards it. Canonical `PostgresIndexQueryPort` applies availability and generic
owner admission before storage execution, then runs persisted schema readiness, page and optional exact
count in one `REPEATABLE READ, READ ONLY` transaction and decodes only through the localized decoder.

## Generic `TextLike` — source complete

`FilterExpr::TextLike(FieldPath, String)` is a generic Index filter variant appended after the existing
filter variants to preserve prior postcard discriminants.

Validation permits it only on a filterable scalar String field. The pattern is limited to 1024 UTF-8
bytes, rejects NUL and a trailing unpaired backslash, and uses PostgreSQL-compatible wildcard rules:
`%` for zero-or-more characters, `_` for one character, and `\` as the escape character.

Both ordinary and localized PostgreSQL compilers bind the pattern and emit `LIKE ... ESCAPE E'\\'`.
Ordinary linked/many filtering reuses the existing correlated filter machinery. The reference engine and
PostgreSQL equivalence fixture implement the same wildcard grammar.

The localized Product title predicate can therefore be represented as
`any_locale_filter = TextLike(title, format!("%{trimmed_search}%"))` without a Product-specific SQL branch.
A title match admits the Product identity but does not select the projected locale.

## Remaining search parity gates

The Product owner list currently has no explicit search-length bound, while generic `TextLike` is bounded
to 1024 UTF-8 bytes. The Storefront adapter must not silently truncate or reject owner-valid input.
Before cutover, the adapter/evidence slice must either prove an authoritative upstream <=1024-byte bound
or introduce a reviewed owner/API bound with matching validation evidence.

The owner title `LIKE` also uses the database default collation while Index String scalar SQL uses the
engine's deterministic `COLLATE "C"`. Retained PostgreSQL equivalence must establish the admitted
deployment/input contract before search parity can be promoted.

## Typed EAV and Taxonomy boundaries

Dynamic Product EAV remains represented by canonical UUID-keyed `attribute_terms`. Public Product
attribute/option codes must be resolved through Product owner metadata before building Index predicates.
Localized text EAV retains the owner fallback predicate defined in `m7-product-attribute-term-contract.md`.

Index stores stable Product tag UUIDs, not localized Taxonomy names. A future adapter must batch-hydrate
requested/fallback tag names only after the Product page is fixed.

## Immutable replacement boundary

The current Product routing key remains internal storage/replay identity. Lower persisted keys are
historical only. Promotion remains staged: register current key, rebuild/replay, prove readiness/parity,
`register_current` to retire lower active keys, then consumer cutover.

## Remaining work before Storefront cutover

1. implement the Product Storefront Index adapter over `execute_localized_query`;
2. map Active + published-only/category/channel/EAV/order/page/count semantics;
3. resolve Product attribute/option codes to canonical terms;
4. use `TextLike` for all-translations title search and explicitly resolve the search-bound/collation gates;
5. batch-hydrate localized Taxonomy tag names after page selection;
6. actualize retained Product PostgreSQL packets to routing key `4` / current 15-field contract;
7. retain owner-vs-Index localized equivalence for requested/fallback/third-locale search, wildcard escape,
   de-dup, ordering, pagination, exact count, stale locale materialization, readiness and restart;
8. extend linked folded paths only with explicit target-lag availability evidence;
9. stage/rebuild/promote the current Product key for a tenant;
10. only then select Index traffic.

## Source guards

- `verify-index-product-storefront-parity-gate.mjs` keeps Storefront owner-native and locks the physical
  source boundary;
- `verify-index-product-storefront-localized-query-architecture.mjs` locks the fold architecture;
- `verify-index-localized-query-contract.mjs` locks query/projection/cursor roles;
- `verify-index-localized-query-postgres-fold.mjs` locks compiler/decoder semantics;
- `verify-index-localized-query-runtime.mjs` locks readiness/admission/one-snapshot execution;
- `verify-index-text-like-filter.mjs` locks bounded String `TextLike`, PostgreSQL/reference semantics and
  the current Product owner title-search shape.

## Deliberate limits

This slice does not implement the Product Storefront adapter, resolve the owner search-length/collation
parity gates, execute/admit PostgreSQL evidence, rebuild/promote a tenant Product schema, add Product typed
events, or switch Storefront traffic.

## Maintainer verification

```bash
node scripts/verify/verify-index-text-like-filter.mjs
node scripts/verify/verify-index-localized-query-contract.mjs
node scripts/verify/verify-index-localized-query-postgres-fold.mjs
node scripts/verify/verify-index-localized-query-runtime.mjs
node scripts/verify/verify-index-product-storefront-localized-query-architecture.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
