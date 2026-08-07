# M7 Product attribute term contract

Status: `source_complete_materialized_rebuild_pending`.

## Purpose

Product Storefront accepts dynamic typed EAV filters by attribute `code`, while Index schemas are
immutable and static. The single current Product source therefore materializes one static field:

`attribute_terms: Many<String>`

No Index field is created per tenant attribute. Public codes remain Product-owned lookup inputs; Index
terms use stable owner identities.

## Term grammar

Each term has exactly four pipe-separated components:

`<attribute_uuid>|<kind>|<locale_hex>|<value_hex>`

There is deliberately no format-version prefix. Current code owns exactly one grammar.

- `attribute_uuid` is `product_attributes.id`;
- `kind` is one fixed semantic kind;
- `locale_hex` is lower-case hexadecimal UTF-8 of the owner locale string, or empty for non-localized
  values;
- `value_hex` is lower-case hexadecimal UTF-8 of the canonical typed value, or empty for a presence
  marker.

The Storefront request path supplies canonical requested/fallback locale strings. Owner SQL and Index
term filters therefore compare the same strings. An old non-canonical EAV locale row that the owner
query would not match is not made authoritative merely by Index materialization.

## Canonical kinds

The current grammar uses:

- `text` for non-localized text/textarea/richtext equality;
- `localized_text` for one localized text value;
- `localized_present` for existence of a localized row even when `value_text` is null;
- `integer` for base-10 signed i64;
- `decimal` for normalized decimal text with insignificant trailing scale removed;
- `boolean` for `true` or `false`;
- `date` for `YYYY-MM-DD`;
- `datetime` for UTC Unix epoch microseconds;
- `option` for canonical option UUID text used by select/multiselect.

JSON is intentionally absent because the owner Storefront contract rejects JSON attribute filters.

## Stable identity choices

Terms use Product attribute UUID, not mutable public attribute code. Select/multiselect terms use option
UUID, not option code. A future Storefront Index adapter resolves current code/option input through
Product owner metadata before constructing the same term.

Product tags are separate from EAV. The single current Product schema materializes stable Taxonomy
`tag_ids`; localized tag names remain Taxonomy-owned and are hydrated after Index selection rather than
copied into the Product clock.

## Localized fallback equivalence

Owner localized text filtering is:

1. requested-locale row has the requested value; or
2. no requested-locale row exists at all and the fallback-locale row has the requested value.

The source therefore materializes both `localized_text` and `localized_present`. The Index predicate is:

`requested-value OR (NOT requested-present AND fallback-value)`

`attribute_terms.rs::localized_text_filter` constructs that exact `FilterExpr` shape.

## PostgreSQL materialization

`PRODUCT_ATTRIBUTE_TERMS_CTE` is used directly by the single current Product source. It reads only:

- non-detached Product attribute values;
- non-archived, filterable Product/both attributes;
- localized value rows where applicable;
- option memberships for select/multiselect.

The CTE normalizes values to the same bytes as the Rust helpers and emits one sorted, deduplicated JSONB
term array per Product. Localized EAV terms for all owner locales are carried in every localized Product
Index record so requested/fallback EAV filtering remains independent from Product translation fallback.

`attribute_terms` is filterable, non-sortable, and non-selectable. It is query machinery, not public
Storefront payload.

## Owner clock boundary

The existing `ProductAttributeValuesChanged` command advances Product `index_revision`, graph
`projection_epoch`, and Product locale refresh state in one owner transaction. No EAV-specific clock was
added.

## Single-current replacement

Current Product runtime code now publishes one higher internal routing key and only one Product schema.
The Product source:

- materializes `attribute_terms`;
- derives deterministic replay delivery IDs with `derive_index_schema_source_event_id`;
- retains `projection_epoch` as the complete Product mutation clock;
- keeps the same ProductVariant and SalesChannel graph links.

Lower persisted Product keys are historical storage identities only. Production promotion still follows
the explicit staged sequence:

1. ordinary-register the current key;
2. rebuild/replay it fully;
3. prove readiness/parity/freshness/restart evidence;
4. call `register_current` to retire lower active persisted keys;
5. only then allow an authoritative consumer cutover.

The numeric key is an internal storage/replay identity, not a public Product API version or a parallel
compatibility implementation.

## Deliberate limits

This source slice does not:

- execute tenant schema registration, rebuild, or supersession;
- switch Storefront traffic;
- add public typed Product event contracts;
- copy localized Taxonomy tag names into Product Index;
- claim PostgreSQL execution evidence.

## Maintainer verification

```bash
cargo test -p rustok-distribution --features mod-product attribute_terms --lib -- --nocapture
node scripts/verify/verify-index-product-attribute-term-contract.mjs
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-eav-owner-clock.mjs
node scripts/verify/verify-index-product-storefront-parity-gate.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
