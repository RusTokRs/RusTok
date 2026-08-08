# M7 Product Storefront localized query architecture

Status: `runtime_source_complete_text_pattern_and_evidence_pending`.

## Decision

Keep the current owner Storefront behavior and exactly one current Product Index schema. Close the
remaining locale/search mismatch in the **Index query layer** with a generic localized-entity identity
fold over the existing physical locale rows.

Do not narrow Product owner search/fallback semantics merely to fit `IndexQueryScope`, and do not add a
Storefront-only Product routing key or compatibility source. Locale folding is a query-shape concern, not
a new immutable Product schema version.

## Owner contract preserved

`CatalogService::list_published_products_with_query` remains authoritative until cutover evidence passes.
The replacement path must preserve:

- one result identity per Product;
- title search matching any Product translation;
- requested locale -> fallback locale result projection;
- owner placeholder behavior when neither requested nor fallback translation exists;
- Product status/publication/category/channel/EAV identity predicates;
- timestamp ordering with stable Product identity tie-break;
- pagination/exact count over Product identities, not physical translation rows.

## Generic identity model

The fold operates only for `LocaleMode::Required`. Ordinary `IndexQuery` stays exact-locale and unchanged.
The folded logical identity is `(tenant_id, schema_ref, entity_id)` while physical storage, mutation,
replay, absence and freshness remain locale-keyed.

`LocalizedEntityQuery` keeps `query.scope.locale` as requested locale, stores canonical fallback separately,
and exposes a separate root-only `any_locale_filter` for identity-level existential matching.

Generic Index schemas intentionally do not label fields as owner-localized. The fold therefore has an
explicit `localized_projection_fields` role set. A listed field is projected only from:

1. an admitted requested-locale row;
2. otherwise an admitted fallback-locale row;
3. otherwise SQL null.

Unlisted fields are read from a deterministic admitted identity anchor. This explicit classification
prevents a third-locale anchor from leaking `title`, `handle` or other localized content when the
requested/fallback rows are absent. It does not change the Product schema fingerprint.

The initial compiler/runtime is intentionally root-only. Linked query paths fail validation until linked
fold semantics and their own availability evidence are added. This still covers the current Product
Storefront list contract, which can express its identity predicates through root fields such as
`sales_channel_ids` and `attribute_terms`.

## Query validation and cursor identity

`SchemaRegistry::validate_localized_entity_query`:

- requires `LocaleMode::Required`;
- reuses ordinary field/operator/value validation;
- shares one bounded node budget between ordinary and any-locale filters;
- rejects linked paths in this compiler/runtime slice;
- requires every `localized_projection_fields` entry to be a selected root scalar field;
- rejects localized projection fields from the ordinary identity filter and identity ordering.

`LocalizedCursorCodec` uses scoped wire version `3`; ordinary exact-locale cursors remain on version `2`.
Its fingerprint binds fold mode, tenant/schema, requested/fallback roles, ordinary filter,
`any_locale_filter`, canonical localized projection roles and ordering. A token cannot cross fold modes or
be reused after changing projection/search/fallback semantics.

## PostgreSQL fold compiler

`SchemaRegistry::compile_postgres_localized_page_query` compiles one page statement and optional exact
count without modifying the ordinary PostgreSQL compiler.

The page compiler uses canonical physical aliases:

- `t0` — deterministic admitted identity anchor;
- `t1` — requested-locale projection row;
- `t2` — fallback-locale projection row when distinct from requested;
- `t3` — any-locale existential predicate row;
- `t4` — lower-locale anti-duplicate anchor candidate.

Every physical role is an `index_entities AS "tN"` relation with the ordinary
`"tN".is_deleted = FALSE` anchor. The existing generic `PostgresQueryEntityAdmission` can therefore
inject current owner freshness rules into every participating row role without a Product-specific compiler
branch.

One Product identity survives page/count selection by choosing the lexicographically lowest **admitted**
physical locale row as `t0`: an identity is rejected as a duplicate when an admitted same-identity `t4`
with a lower locale key exists. De-duplication therefore happens before ordering, lookahead, limit and
exact count.

Ordinary identity filters and ordering are evaluated on `t0`. `any_locale_filter` is compiled into an
`EXISTS` over `t3`. Localized projected fields use row-presence `CASE`, not value-level `COALESCE`, so a
present requested row with a nullable field does not incorrectly fall through to fallback content.

Exact count uses the same `t0`/`t3`/`t4` identity boundary but intentionally omits `t1`/`t2`: requested or
fallback projection availability does not change owner Product identity count.

The compiler emits a dedicated `LocalizedQueryPlanFingerprint` in addition to the ordinary plan
fingerprint. It binds the canonical fallback, any-locale predicate and localized projection roles.

## Decoder

`SchemaRegistry::decode_postgres_localized_query_page` verifies both ordinary and localized plan
fingerprints, exact column contracts, lookahead bounds and exact-count shape.

For unlisted fields it preserves the ordinary schema nullability/type/cardinality contract. For an explicit
localized projection field only, SQL null is additionally valid and means that neither requested nor
fallback physical locale row was admitted. That null is an Index-level absence signal; the later Product
Storefront adapter must apply the existing owner placeholder behavior instead of selecting an arbitrary
third translation.

Lookahead produces `LocalizedIndexCursor` through `LocalizedCursorCodec`; no ordinary exact-locale cursor
is emitted or accepted by this path.

## Runtime execution — source complete

`IndexQueryPort` now publishes an explicit `execute_localized_query` method. The trait default fails closed
with a stable contract-preparation error, so existing non-PostgreSQL/custom adapters remain source
compatible without silently claiming localized semantics.

`SharedIndexQueryRuntime` forwards the localized capability to the host-selected query port.

`PostgresIndexQueryPort::execute_localized_query` implements the canonical execution boundary:

1. require PostgreSQL backend;
2. derive the same persisted schema contracts from the embedded ordinary root query;
3. compile `CompiledPostgresLocalizedPageQuery`;
4. apply query-path availability plus `PostgresQueryEntityAdmission` to the compiled fold **before**
   beginning storage execution;
5. begin one transaction and configure it as `REPEATABLE READ, READ ONLY`;
6. verify tenant-scoped persisted schema readiness/fingerprint/JSON/status inside that snapshot;
7. execute page SQL and optional exact-count SQL in the same snapshot;
8. decode only through `decode_postgres_localized_query_page`;
9. commit successful read snapshots or roll back on any error.

Admission preparation is deliberately outside storage execution. A malformed or incompatible trusted
admission template fails closed before page/count statements run. Persisted readiness remains inside the
same snapshot as data reads.

The runtime reuses the ordinary page-row mapping, exact-count mapping, schema readiness verifier, bind
mapping and transaction finalization rather than creating a second storage policy.

Because the current fold validator rejects linked paths, query-path link-target availability is a no-op for
this initial localized runtime. The same admission helper is still called so future linked support cannot
silently bypass the established execution boundary.

## Remaining title-search primitive

Any-locale title search remains an identity predicate. A matching physical locale row may admit an
identity, but that row never becomes result localization merely because it matched.

The current generic filter algebra still lacks a scalar string text-pattern/substring operator matching
the Product owner `LIKE %search%` semantics. The next source slice must add one generic bounded scalar
text-pattern primitive and compile it inside `any_locale_filter`; adding LIKE/substring only to ordinary
exact-locale filtering remains insufficient.

## Storefront adapter boundary

After scalar text-pattern support exists, the Product adapter must:

- map Active + published-only/category/channel visibility predicates;
- resolve public Product attribute/option codes to canonical `attribute_terms`;
- classify `title`/`handle` and other returned localized fields in `localized_projection_fields`;
- use folded any-locale title search;
- batch-hydrate localized Taxonomy tag names after the Product page is fixed;
- preserve owner page/per-page, exact count, ordering and ID tie-break semantics.

Taxonomy localization stays Taxonomy-owned; Product Index stores stable tag UUIDs only.

## Required retained evidence

Before Storefront traffic can move, PostgreSQL equivalence must cover at least:

1. requested translation present;
2. requested absent + fallback present;
3. requested/fallback absent while another translation exists;
4. search match only in requested locale;
5. search match only in fallback locale;
6. search match only in a third locale while projection still follows requested/fallback roles;
7. duplicate matches in multiple locales yielding one Product identity/exact count;
8. interleaved physical locale rows with stable global identity ordering/page boundaries;
9. cursor continuation and exact-count parity;
10. delayed/stale locale rows excluded by owner freshness;
11. persisted schema readiness failure before authoritative query results;
12. fresh runtime/recomposition/replay redelivery preserving the same grouped page;
13. current Product routing-key rebuild/promotion rather than a historical lower key.

Linked-target lag evidence remains required before adding linked paths to folded execution.

## Deliberate limits

This slice does not:

- change ordinary `IndexQuery` or its compiler/cursor;
- add scalar text-pattern matching;
- implement the Product Storefront Index adapter;
- actualize/execute retained Product PostgreSQL packets;
- add Product typed events or bypass event-digest admission;
- rebuild/promote a tenant Product schema;
- switch Storefront traffic.

## Next implementation slice

Add a generic bounded scalar string text-pattern primitive to the Index filter contract and PostgreSQL
compiler, then allow it inside `LocalizedEntityQuery::any_locale_filter`. Keep the operator generic and
schema-validated; do not add a Product-specific SQL branch. After that, implement the Product Storefront
adapter plus retained owner-vs-Index localized-query PostgreSQL evidence before any traffic cutover.

## Source guards

- `scripts/verify/verify-index-localized-query-contract.mjs` locks query/projection/cursor roles.
- `scripts/verify/verify-index-localized-query-postgres-fold.mjs` locks the root-only page/count compiler,
  physical alias/admission anchors and decoder.
- `scripts/verify/verify-index-localized-query-runtime.mjs` locks fail-closed port publication, host
  forwarding, readiness/admission-before-execution and one-snapshot page/count runtime semantics.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
