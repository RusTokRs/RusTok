# M7 Product Storefront localized query architecture

Status: `runtime_text_pattern_identity_order_source_complete_adapter_and_evidence_pending`.

## Decision

Keep the current owner Storefront behavior and exactly one current Product Index schema. Preserve physical
locale-keyed Index rows and close Storefront locale/search parity in the generic Index query layer with an
explicit localized-entity identity fold.

Ordinary `IndexQuery` remains exact-locale and keeps its existing stable ascending entity-ID tie-break.
Product-specific parity is expressed only through the explicit localized query contract; no second Product
routing key or Product-only SQL compiler branch is introduced.

## Owner contract preserved

`CatalogService::list_published_products_with_query` remains authoritative until retained equivalence and
cutover gates pass. Its relevant contract is:

- one result identity per Product;
- title search may match any Product translation;
- requested locale -> fallback locale result projection;
- owner placeholder behavior when neither requested nor fallback translation exists;
- Active/published/category/channel/EAV identity predicates;
- exact count over Product identities;
- ordering by the selected timestamp and Product ID in the **same** direction;
- page/per-page pagination after those identity predicates.

The owner title helper builds `%{search}%` and executes `pt.title LIKE $1` across all translations. It has no
locale predicate and `StorefrontProductListQuery` currently has no explicit search-length bound.

## Generic localized identity model

The fold operates only for `LocaleMode::Required`. Its logical result identity is
`(tenant_id, schema_ref, entity_id)` while physical storage, mutation, replay, absence and freshness remain
locale-keyed.

`LocalizedEntityQuery` carries:

- the ordinary exact-locale `IndexQuery` shape;
- requested locale in `query.scope.locale`;
- canonical fallback locale;
- root-only `any_locale_filter`;
- explicit `localized_projection_fields` for requested -> fallback -> null projection;
- explicit `identity_order_direction` for the final root entity-ID tie-break.

`identity_order_direction` defaults to `Asc` for compatibility with all localized queries created before
this capability. Validation accepts only `Asc` or `Desc`; aggregate order variants are rejected.

## Cursor and plan identity

`LocalizedCursorCodec` remains on localized wire version `3`; ordinary exact-locale cursors remain on
version `2`. The localized query fingerprint now also binds `identity_order_direction`, so a continuation
created under ascending identity ordering cannot be accepted under descending identity ordering and vice
versa. Existing localized v3 tokens from the earlier query-fingerprint shape fail closed by fingerprint
mismatch.

`LocalizedQueryPlanFingerprint` likewise binds `identity_order_direction` in addition to ordinary plan
fingerprint, fallback, any-locale predicate and localized projection roles.

## PostgreSQL fold ordering

The root-only localized compiler continues to use:

- `t0` deterministic admitted identity anchor;
- `t1` requested projection row;
- `t2` fallback projection row;
- `t3` any-locale predicate row;
- `t4` lower-locale anti-duplicate candidate.

Grouping/de-duplication occurs before ordering, lookahead, limit and exact count. Explicit `order_by` terms
keep their own direction/null semantics. The final `t0.entity_id` tie-break uses
`query.identity_order_direction`:

- `Asc` -> `ORDER BY ... entity_id ASC` and cursor continuation `entity_id > cursor.entity_id`;
- `Desc` -> `ORDER BY ... entity_id DESC` and cursor continuation `entity_id < cursor.entity_id`.

This closes the Product owner mismatch for descending timestamp pages where multiple Products share the
same timestamp. Ordinary exact-locale SQL compilation is deliberately unchanged.

## Existing runtime and text-pattern contract

The localized compiler/decoder/runtime remains source-complete:

- `IndexQueryPort` publishes `execute_localized_query` and `PostgresIndexQueryPort::execute_localized_query` wires localized execution;
- persisted schema readiness and generic `PostgresQueryEntityAdmission` are fail-closed;
- page and exact count execute in one `REPEATABLE READ, READ ONLY` transaction;
- localized results decode only through the localized decoder;
- `FilterExpr::TextLike` is a generic bounded scalar String predicate usable inside `any_locale_filter`;
- `%`, `_` and backslash escape use PostgreSQL `LIKE` semantics;
- the reference engine mirrors those wildcard semantics.

## Storefront adapter blockers discovered during recheck

Three explicit parity gates remain before an owner-facing adapter can claim completeness:

1. **Search bound:** Product owner search is not explicitly length-bounded, while generic `TextLike` is
   capped at 1024 UTF-8 bytes. The adapter must not silently truncate/reject owner-valid input.
2. **Search collation:** owner `LIKE` uses deployment/default collation while Index String scalar SQL uses
   deterministic `COLLATE "C"`; retained PostgreSQL evidence must establish the admitted contract.
3. **Channel-less visibility:** owner requests without a public channel admit only Products whose metadata
   visibility is unrestricted. Current `sales_channel_ids` materializes unrestricted visibility as
   membership in all current channels, so it cannot distinguish unrestricted from a restricted Product
   that happens to contain every current channel. A channel-less authoritative adapter must therefore
   fail closed until that distinction is materialized or otherwise proven by an owner capability.

For a request with a trusted current public channel ID, `sales_channel_ids` membership remains the intended
root predicate because the Product channel resolver maps unrestricted visibility to all current channels
and restricted visibility to its resolved allowed channels, under the existing freshness witness.

## Next adapter/evidence slice

The next source slice may build a shadow/evidence Product Storefront adapter only after preserving these
rules:

- requested/fallback Product fields are explicit localized projections;
- `TextLike(title, "%...%")` remains an any-locale identity predicate;
- ascending owner sort uses ascending identity tie-break;
- descending owner sort uses descending identity tie-break;
- Active, published-only, category, EAV, channel and exact-count semantics are root identity predicates;
- Product attribute/option inputs resolve through Product-owned metadata before canonical
  `attribute_terms` are emitted;
- Taxonomy tag names are hydrated only after the Product page is fixed;
- unresolved search-bound/collation/channel-less visibility cases fail closed;
- Storefront traffic remains owner-native until retained PostgreSQL equivalence is executed and admitted.

## Retained evidence required before cutover

Evidence must cover requested/fallback/third-locale cases, cross-locale search, wildcard escaping,
duplicate locale matches, exact count, repeated equal timestamps under both ascending and descending ID
tie-breaks, cursor continuation, stale locale exclusion, readiness/admission failure, restart/replay,
channel visibility, EAV predicates, search-bound/collation behavior and current Product routing key `4`.

Linked folded paths remain separately blocked on explicit target-availability evidence.

## Source guards

- `verify-index-localized-query-contract.mjs` locks query/projection/cursor roles;
- `verify-index-localized-query-postgres-fold.mjs` locks fold compiler/decoder semantics;
- `verify-index-localized-query-runtime.mjs` locks readiness/admission/snapshot execution;
- `verify-index-text-like-filter.mjs` locks generic bounded LIKE semantics;
- `verify-index-localized-identity-order.mjs` locks explicit localized entity-ID tie-break direction and
  proves ordinary exact-locale ordering remains unchanged.

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
