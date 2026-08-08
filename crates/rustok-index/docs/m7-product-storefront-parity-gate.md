# M7 Product Storefront Index parity gate

Status: `localized_runtime_source_complete_text_pattern_adapter_and_evidence_pending`.

## Purpose

The Product Storefront catalog remains owner-native. The single current Product Index source materializes
the Storefront scalar fields, stable tag identities, and typed EAV query state that were previously
missing. The remaining localized Product identity mismatch now has a selected architecture, explicit
query/cursor contract, PostgreSQL fold compiler/decoder, and fail-closed PostgreSQL runtime execution.
Generic scalar text-pattern support, the Product Storefront adapter, and retained owner-vs-Index evidence
are still pending.

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

## Single current Product Index coverage

Current Product runtime code publishes one Product schema with 15 fields and two links. Product Index
still emits one physical entity per stored translation locale. It does not fabricate requested/fallback
rows. This physical model remains correct; Storefront equivalence is resolved by the query fold, not by
another Product schema/routing key.

Do **not** add another Product routing key merely to patch localized Storefront query semantics. The
current Product contract remains the only runtime Product implementation.

## Localized identity fold — source complete

The selected generic fold preserves logical result identity
`(tenant_id, schema_ref, entity_id)` while physical storage remains locale-keyed.

`LocalizedEntityQuery` provides:

- requested locale through `query.scope.locale`;
- canonical fallback locale;
- root-only `any_locale_filter` for identity-level existential matching;
- explicit `localized_projection_fields` for requested -> fallback -> null output semantics.

`LocalizedCursorCodec` uses dedicated scoped wire version `3`; ordinary exact-locale cursors remain on
version `2` and cannot cross modes.

The initial fold deliberately rejects linked query paths. Current Product Storefront list identity
predicates can still be expressed through root fields such as `sales_channel_ids` and `attribute_terms`.
Linked folded paths remain separately evidence-gated.

## PostgreSQL compiler/decoder — source complete

`SchemaRegistry::compile_postgres_localized_page_query` uses canonical physical aliases:

- `t0` deterministic admitted identity anchor;
- `t1` requested projection row;
- `t2` fallback projection row;
- `t3` any-locale predicate row;
- `t4` lower-locale anti-duplicate candidate.

All physical roles retain the ordinary `is_deleted = FALSE` anchor so trusted generic
`PostgresQueryEntityAdmission` can inject current owner freshness before execution. De-duplication occurs
before ordering/lookahead/limit/exact-count. Requested/fallback projection uses row-presence `CASE`, and
exact count is independent of projection-row availability.

`SchemaRegistry::decode_postgres_localized_query_page` verifies ordinary plus localized plan identity,
column/count/lookahead contracts, allows extra SQL-null only for explicit localized projection fields,
and emits dedicated localized continuation cursors.

## PostgreSQL runtime — source complete

`IndexQueryPort` now exposes explicit `execute_localized_query`. Its default implementation fails closed,
keeping existing adapters source-compatible without claiming unsupported semantics.

`SharedIndexQueryRuntime` forwards the capability to the host-selected port.

The canonical `PostgresIndexQueryPort` execution path:

1. requires PostgreSQL;
2. compiles the localized page/count contract;
3. applies query-path availability plus generic owner entity admission to all compiled physical aliases
   before storage execution;
4. begins one `REPEATABLE READ, READ ONLY` transaction;
5. verifies tenant-scoped persisted schema status/fingerprint/JSON inside that snapshot;
6. executes page and optional exact count in the same snapshot;
7. decodes only through `decode_postgres_localized_query_page`;
8. commits successful read snapshots and rolls back failures.

This reuses the ordinary readiness verifier, bind mapping, row mapping, exact-count mapping, and
transaction finalization. No second storage policy is introduced.

## Remaining search gap

A scalar substring/LIKE operator alone was previously insufficient because exact-locale querying could
not preserve any-locale admission plus requested/fallback projection. The fold now solves that identity
problem, but the generic filter algebra still lacks a scalar string text-pattern primitive matching the
owner `LIKE %search%` behavior.

The next source slice must add one generic bounded scalar string text-pattern operator and compile it in
ordinary root SQL so it can be used safely inside `any_locale_filter`. It must not introduce a
Product-specific SQL branch.

## Typed EAV and Taxonomy boundaries

Dynamic Product EAV remains represented by canonical UUID-keyed `attribute_terms`. Public Product
attribute/option codes must be resolved through Product owner metadata before building Index predicates.

Index stores stable Product tag UUIDs, not localized Taxonomy names. A future Storefront adapter must
batch-hydrate requested/fallback tag names only after the Product page is fixed.

## Immutable replacement boundary

The current Product routing key remains internal storage/replay identity. Lower persisted keys are
historical only. Promotion remains staged: register current key, rebuild/replay, prove readiness/parity,
`register_current` to retire lower active keys, then consumer cutover.

## Remaining work before Storefront cutover

1. add generic scalar text-pattern matching usable in folded `any_locale_filter`;
2. implement the Product Storefront Index adapter;
3. map Active + published-only/category/channel/EAV/order/page/count semantics;
4. resolve Product attribute/option codes to canonical terms;
5. batch-hydrate localized Taxonomy tag names;
6. actualize retained Product PostgreSQL packets to routing key `4` / current 15-field contract;
7. retain and execute owner-vs-Index localized equivalence, including requested/fallback/third-locale
   search, de-dup, ordering, pagination, exact count, stale locale materialization, readiness and restart;
8. extend linked folded paths only with explicit target-lag availability evidence;
9. stage/rebuild/promote the current Product key for a tenant;
10. only then select Index traffic.

## Source guards

- `verify-index-product-storefront-parity-gate.mjs` keeps Storefront owner-native and locks the physical
  source boundary;
- `verify-index-product-storefront-localized-query-architecture.mjs` locks the fold architecture;
- `verify-index-localized-query-contract.mjs` locks query/projection/cursor roles;
- `verify-index-localized-query-postgres-fold.mjs` locks compiler/decoder semantics;
- `verify-index-localized-query-runtime.mjs` locks fail-closed port publication, readiness, admission and
  one-snapshot execution.

## Deliberate limits

This slice does not add scalar text-pattern matching, implement the Product Storefront adapter, execute or
admit PostgreSQL evidence, rebuild/promote a tenant Product schema, add Product typed events, or switch
Storefront traffic.

## Maintainer verification

```bash
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
