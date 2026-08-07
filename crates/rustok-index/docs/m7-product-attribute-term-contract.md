# M7 Product attribute term contract

Status: `source_complete_materialization_pending`.

## Purpose

Product Storefront accepts dynamic typed EAV filters by attribute `code`, but Index schemas are immutable
and static. Adding one Index field per Product attribute would turn tenant data into runtime schema drift
and would require a new schema fingerprint whenever an attribute is added, renamed, archived, or made
filterable.

The canonical representation is therefore one static Product field:

`attribute_terms: Many<String>`

The field is filterable and is intended to be non-sortable. Attribute definitions remain Product-owned;
a future Storefront Index adapter resolves the public `code=value` request to an exact Product attribute
UUID/type and builds one or more `Contains(attribute_terms, term)` predicates.

This contract introduces no dynamic Index field and no versioned compatibility family.

## Term grammar

Each term has exactly four pipe-separated components:

`<attribute_uuid>|<kind>|<locale_hex>|<value_hex>`

There is deliberately no format-version prefix. The current code owns exactly one grammar.

- `attribute_uuid` is the canonical lower-case UUID of `product_attributes.id`;
- `kind` is one of the fixed semantic kinds below;
- `locale_hex` is lower-case hexadecimal UTF-8 for a canonical locale, or empty for non-localized
  values;
- `value_hex` is lower-case hexadecimal UTF-8 for the canonical typed value, or empty for a locale
  presence marker.

Hex encoding makes the delimiter collision-free while preserving exact owner text bytes.

## Canonical kinds

The current grammar uses:

- `text` for non-localized text/textarea/richtext equality;
- `localized_text` for one localized text value;
- `localized_present` for the existence of a localized value row, regardless of whether its
  `value_text` is null;
- `integer` for base-10 signed i64;
- `decimal` for normalized decimal text with insignificant trailing scale removed;
- `boolean` for `true` or `false`;
- `date` for `YYYY-MM-DD`;
- `datetime` for UTC Unix epoch microseconds;
- `option` for canonical option UUID text used by select/multiselect.

`json` attributes are intentionally absent because the owner Storefront contract rejects JSON
`attribute_filters`.

## Stable identity choices

Terms use `product_attributes.id`, not the mutable public attribute code. A code rename therefore does
not force Product rematerialization merely to rewrite every term; the Storefront adapter resolves the
current code to the stable UUID before querying Index.

Select/multiselect terms use `product_attribute_options.id`, not option code. The current owner contract
accepts a raw option UUID directly. When a public request supplies an option code, the future adapter
must resolve only a non-archived option code to its UUID before constructing the same term.

Product tag names are not represented by this EAV grammar. The replacement Product schema will retain
stable taxonomy term UUIDs and localize their names through the Taxonomy owner boundary rather than
copying Taxonomy translations into the Product clock.

## Localized text fallback equivalence

The owner predicate for localized text is:

1. requested-locale row exists with the requested value; or
2. **no requested-locale row exists at all**, and fallback-locale row exists with the requested value.

The second condition cannot be represented by value terms alone. Therefore source materialization emits
both:

- `localized_text` for non-null translation values;
- `localized_present` for every translation row, including rows whose value text is null.

The Index filter is exactly:

- if requested locale equals fallback locale: requested `localized_text` term;
- otherwise: `requested-value OR (NOT requested-present AND fallback-value)`.

`attribute_terms.rs::localized_text_filter` builds that `FilterExpr` shape directly.

## PostgreSQL source equivalence

`PRODUCT_ATTRIBUTE_TERMS_CTE` is the canonical set-based source fragment. It reads only:

- non-detached `product_attribute_values`;
- non-archived `product_attributes`;
- `is_filterable = TRUE`;
- Product or both scope;
- localized value translations where applicable;
- option memberships for select/multiselect.

Those predicates match the owner catalog filter admission boundary.

The CTE normalizes scalar values to the same bytes as the Rust term helpers and emits a sorted,
deduplicated JSONB term array per Product. It is tenant-scoped by the Product source `$1` parameter and
does not join through Product translation locale.

Localized EAV terms intentionally carry all available locales in every localized Product Index record;
that is what allows requested/fallback EAV filtering to remain exact even when Product translation
fallback is resolved separately.

## Owner clock boundary

PR #3197 made the existing `ProductAttributeValuesChanged` command advance the canonical Product
`index_revision` and locale refresh ledger in the same transaction. Therefore materializing these terms
does not require an EAV-specific clock.

Product graph mutation ordering remains `projection_epoch`; the Product owner component remains
`products.index_revision`.

## Replacement schema use

The next single-current Product replacement may add `attribute_terms` exactly once alongside the other
Storefront parity fields. Because the existing Product schema fingerprint is immutable, that replacement
must:

1. use one monotonically higher internal Product routing key;
2. publish only that replacement Product schema in new runtime code;
3. switch Product deterministic replay delivery IDs to `derive_index_schema_source_event_id`;
4. materialize `PRODUCT_ATTRIBUTE_TERMS_CTE` into the Product record;
5. stage/rebuild the new key through ordinary schema registration;
6. prove readiness/parity/freshness/restart evidence;
7. promote it through `register_current` so lower persisted Product keys become retired;
8. keep Storefront owner-native until retained owner-vs-Index equivalence is admitted.

The numeric key is an internal storage/replay identity, not a public Product API version or a parallel
compatibility implementation.

## Deliberate limits

This slice does not:

- change the current Product schema or numeric routing key;
- write `attribute_terms` into Index rows yet;
- switch Storefront traffic;
- add public event contracts;
- execute schema registration/rebuild/supersession;
- copy localized taxonomy tag names into Product Index;
- claim PostgreSQL execution evidence.

## Maintainer verification

Suggested commands, intentionally not run by the implementation agent:

```bash
cargo test -p rustok-distribution --features mod-product attribute_terms --lib -- --nocapture
node scripts/verify/verify-index-product-attribute-term-contract.mjs
node scripts/verify/verify-index-product-eav-owner-clock.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
