# M7 Product Storefront localized query architecture

Status: `runtime_and_text_pattern_source_complete_adapter_and_evidence_pending`.

## Decision

Keep the current owner Storefront behavior and exactly one current Product Index schema. Preserve the
physical locale-keyed Index rows and close Storefront locale/search parity in the generic Index query layer
with an explicit localized-entity identity fold.

Do not narrow owner search/fallback behavior merely to fit ordinary `IndexQueryScope`, and do not add a
Storefront-only Product routing key or compatibility schema. Locale folding and text matching are query
semantics, not immutable Product storage identities.

## Owner contract preserved

`CatalogService::list_published_products_with_query` remains authoritative until retained equivalence and
cutover gates pass. Its relevant contract is:

- one result identity per Product;
- title search may match any Product translation;
- requested locale -> fallback locale result projection;
- owner placeholder behavior when neither requested nor fallback translation exists;
- Active/published/category/channel/EAV identity predicates;
- exact count and stable timestamp + Product-ID ordering over Product identities;
- page/per-page pagination after those identity predicates.

The current Product title helper trims an empty search away, then constructs `format!("%{search}%")` and
executes PostgreSQL `pt.title LIKE $1` in an `EXISTS` over all Product translations. It has no locale
predicate and currently has no Product-level search-length limit.

## Generic identity model

The fold operates only for `LocaleMode::Required`. Ordinary `IndexQuery` stays exact-locale and unchanged.
The folded logical identity is `(tenant_id, schema_ref, entity_id)` while physical storage, mutation,
replay, absence and freshness remain locale-keyed.

`LocalizedEntityQuery` keeps `query.scope.locale` as requested locale, stores canonical fallback separately,
and exposes root-only `any_locale_filter` for identity-level existential matching.

Generic schemas intentionally do not claim that particular fields are owner-localized. The fold therefore
uses explicit `localized_projection_fields`. A listed field is projected only from:

1. an admitted requested-locale row;
2. otherwise an admitted fallback-locale row;
3. otherwise SQL null.

Unlisted fields are read from a deterministic admitted identity anchor. This prevents an arbitrary third
locale from leaking title/handle content when requested/fallback rows are absent, without changing the
Product schema fingerprint.

The current compiler/runtime remains root-only. Linked folded paths fail validation until linked semantics
and target-availability evidence are introduced explicitly. The current Storefront list can express its
root identity predicates through materialized fields such as `sales_channel_ids` and `attribute_terms`.

## Query validation and cursor identity

`SchemaRegistry::validate_localized_entity_query` requires `LocaleMode::Required`, reuses ordinary
field/operator/value validation, shares one bounded filter-node budget across ordinary and any-locale
filters, rejects linked paths, and requires every localized projection entry to be a selected root scalar.
Localized projection fields cannot drive ordinary identity filtering or ordering.

`LocalizedCursorCodec` uses scoped wire version `3`; ordinary exact-locale cursors remain on version `2`.
The localized query fingerprint binds fold mode, tenant/schema, requested/fallback roles, ordinary filter,
`any_locale_filter`, canonical localized projection roles and ordering. A continuation cannot cross modes
or survive a changed text pattern/search shape.

## PostgreSQL fold compiler and decoder

`SchemaRegistry::compile_postgres_localized_page_query` compiles one page statement and optional exact
count. Canonical physical aliases are:

- `t0` — deterministic admitted identity anchor;
- `t1` — requested-locale projection row;
- `t2` — fallback-locale projection row when distinct from requested;
- `t3` — any-locale existential predicate row;
- `t4` — lower-locale anti-duplicate anchor candidate.

Every role retains the ordinary `is_deleted = FALSE` anchor so generic `PostgresQueryEntityAdmission` can
inject owner freshness without a Product-specific compiler branch. One identity survives before
ordering/lookahead/limit/count by excluding an admitted lower-locale `t4` row.

Ordinary identity filters and ordering are evaluated on `t0`; `any_locale_filter` is an `EXISTS` over
`t3`. Requested/fallback projection uses row-presence `CASE`, not value-level `COALESCE`. Exact count uses
the same identity/search boundary but intentionally omits requested/fallback projection rows.

`SchemaRegistry::decode_postgres_localized_query_page` verifies ordinary and localized plan fingerprints,
column/count/lookahead contracts and emits only localized continuation cursors. SQL null beyond physical
field nullability is accepted only for an explicit localized projection field and means no admitted
requested/fallback row exists.

## Runtime execution — source complete

`IndexQueryPort` publishes `execute_localized_query`; its default implementation fails closed so custom or
non-PostgreSQL adapters do not silently claim the capability. `SharedIndexQueryRuntime` forwards the
capability.

`PostgresIndexQueryPort::execute_localized_query`:

1. requires PostgreSQL;
2. derives persisted schema contracts from the embedded ordinary root query;
3. compiles the localized page/count contract;
4. applies query-path availability plus `PostgresQueryEntityAdmission` before storage execution;
5. starts one `REPEATABLE READ, READ ONLY` transaction;
6. verifies persisted tenant schema status/fingerprint/JSON in that snapshot;
7. executes page and optional exact count in the same snapshot;
8. decodes only through `decode_postgres_localized_query_page`;
9. commits successful reads and rolls back failures.

The path reuses ordinary bind mapping, row mapping, exact-count handling, readiness verification and
transaction finalization.

## Generic `TextLike` — source complete

`FilterExpr::TextLike(FieldPath, String)` is appended after all pre-existing filter variants so existing
postcard enum discriminants remain stable. It is generic Index behavior, not Product-specific behavior.

Validation requires:

- a filterable scalar (`FieldCardinality::One`) String field;
- pattern size at most 1024 UTF-8 bytes;
- no NUL byte;
- no trailing unpaired backslash escape.

Pattern semantics are explicit PostgreSQL `LIKE` semantics:

- `%` matches zero or more characters;
- `_` matches one character;
- `\` escapes the following wildcard or literal character.

Both the ordinary PostgreSQL compiler and the localized alias compiler bind the pattern as
`PostgresBindValue::Text` and compile `LIKE ... ESCAPE E'\\'`. Ordinary linked/many filtering reuses the
existing correlated `compile_many_exists` path; the localized fold stays root-only and can use the same
operator inside `any_locale_filter`.

The in-memory reference engine and the PostgreSQL equivalence reference fixture implement the same
wildcard/escape grammar over Unicode scalar values. Query/cursor fingerprints already serialize
`FilterExpr`, so changing a `TextLike` pattern changes continuation identity without another cursor wire
version.

For the current Product owner search, the future adapter can build `%{trimmed_search}%` and place
`TextLike(title, pattern)` inside `any_locale_filter`. A search match in `t3` admits the Product identity but
does not choose the projected locale.

## Remaining Product search-bound mismatch

`TextLike` is deliberately bounded, while the current `StorefrontProductListQuery` has no explicit search
length bound. Therefore source completion of `TextLike` does **not** by itself establish owner parity for
arbitrarily long search input.

Before Storefront cutover, the adapter/equivalence slice must resolve this explicitly. Acceptable outcomes
are to establish an existing authoritative upstream bound that is <= 1024 bytes, or introduce one
reviewed owner/API bound and retain matching validation evidence. The adapter must not silently truncate,
reject, or reinterpret an input that the authoritative owner path still accepts.

Retained PostgreSQL evidence must also cover the deployment collation used by owner title `LIKE` versus the
Index String scalar's deterministic `COLLATE "C"`; no general collation-equivalence claim is made by source
inspection.

## Storefront adapter boundary

The next Product adapter must:

- map Active + published-only/category/channel visibility predicates;
- resolve Product attribute/option codes to canonical `attribute_terms`;
- classify returned localized fields such as `title`/`handle` in `localized_projection_fields`;
- build folded any-locale title search with `TextLike`;
- preserve requested/fallback/placeholder behavior;
- preserve timestamp ordering, Product-ID tie-break, page/per-page and exact count;
- batch-hydrate localized Taxonomy tag names only after the Product page is fixed;
- fail closed on the unresolved owner-vs-Index search-bound/collation gates.

Taxonomy localization stays Taxonomy-owned; Product Index stores stable tag UUIDs only.

## Required retained evidence

Before Storefront traffic can move, PostgreSQL equivalence must cover at least:

1. requested translation present;
2. requested absent + fallback present;
3. requested/fallback absent while another translation exists;
4. title match only in requested locale;
5. title match only in fallback locale;
6. title match only in a third locale while projection still follows requested/fallback roles;
7. `%`, `_` and escaped wildcard search behavior;
8. duplicate matches in multiple locales yielding one Product identity/exact count;
9. interleaved locale rows with stable global identity ordering/page boundaries;
10. cursor continuation and exact-count parity;
11. delayed/stale locale rows excluded by owner freshness;
12. persisted schema readiness failure before authoritative results;
13. restart/recomposition/replay redelivery preserving the grouped page;
14. search-bound and database-collation parity for the admitted Storefront input contract;
15. current Product routing-key rebuild/promotion rather than a historical lower key.

Linked-target lag evidence remains required before adding linked paths to folded execution.

## Deliberate limits

This slice does not:

- implement the Product Storefront Index adapter;
- resolve the currently unbounded owner search-input contract;
- claim owner/Index collation equivalence;
- actualize or execute retained Product PostgreSQL packets;
- add Product typed events or bypass event-digest admission;
- rebuild/promote a tenant Product schema;
- switch Storefront traffic.

## Next implementation slice

Implement the Product Storefront Index adapter and a retained owner-vs-Index localized-query PostgreSQL
packet, while keeping traffic owner-native. The adapter slice must explicitly resolve the search-input
bound/collation gates before it can claim full search parity.

In parallel, actualize historical Product PostgreSQL packets to routing key `4` and the current 15-field
contract. Do not add a historical routing-key compatibility implementation.

## Source guards

- `scripts/verify/verify-index-localized-query-contract.mjs` locks query/projection/cursor roles.
- `scripts/verify/verify-index-localized-query-postgres-fold.mjs` locks fold compiler/decoder semantics.
- `scripts/verify/verify-index-localized-query-runtime.mjs` locks fail-closed runtime readiness/admission
  and one-snapshot page/count execution.
- `scripts/verify/verify-index-text-like-filter.mjs` locks bounded scalar String validation, PostgreSQL
  LIKE compilation, reference semantics and the current Product owner search shape.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
