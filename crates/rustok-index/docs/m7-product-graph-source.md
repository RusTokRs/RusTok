# M7 canonical Product graph source

Status: `single_current_product_source_complete_storefront_locale_query_gap_open`.

The selected distribution publishes one current Product Index contract and one current ProductVariant
contract. Parallel Product compatibility implementations remain removed.

The generic Index `SchemaRef` still contains a positive numeric routing key because it participates in
persisted schema/entity/link/inbox/replay identities. Current Product runtime code owns exactly one such
key; lower keys are historical storage identities only.

## Canonical Product graph

`product-postgres-primary` emits one locale-required Product record with 15 fields:

- identity/content: `id`, `status`, `title`, `handle`, `description`;
- Storefront owner scalars: `seller_id`, `vendor`, `product_type`, `primary_category_id`;
- stable Taxonomy identities: `tag_ids`;
- Storefront ordering/publication state: `created_at`, `published_at`;
- typed EAV query state: `attribute_terms`;
- graph membership: `variant_ids`, `sales_channel_ids`.

It materializes exactly two many-cardinality links:

- `variants` to current ProductVariant identity;
- `sales_channels` to current SalesChannel identity.

Product visibility slugs are resolver input only; they are not duplicated as transitional Index fields.
Localized tag names remain Taxonomy-owned; Product Index stores only stable tag UUIDs.

Physical Product/translation deletes remain represented by Product-owned tombstones and canonical
`IndexMutation::Delete` mutations.

## Localized identity boundary

One Product Index entity corresponds to one **physically stored** `product_translations.locale` row.
The source does not fabricate requested-locale records from a fallback translation. The absence provider
correctly treats a missing exact locale as absent.

That generic source rule is not yet Storefront-equivalent. Owner Storefront list search currently
matches title through any Product translation, and owner result localization can return a fallback
translation when the requested locale is absent. A current-locale scalar title filter or a second
independent fallback query cannot by itself preserve the same global admission, de-duplication, ordering,
page boundary, and exact count.

This is now an explicit Storefront query/source gap. Do not add another Product routing key or parallel
Product schema merely to work around it. The effective localized Storefront identity/search architecture
must be resolved at the generic query/owner-contract layer first.

## Single-current immutable replacement

The previous Product schema fingerprint was not changed in place. Current runtime code uses one
monotonically higher internal routing key and derives deterministic Product replay UUIDs with
`derive_index_schema_source_event_id`, so rebuild deliveries cannot collide with lower-key historical
`index_inbox` rows.

The replacement source does not register or select the lower Product runtime contract. Production
promotion remains staged:

1. ordinary-register the current immutable key;
2. replay/rebuild it completely;
3. prove exact readiness/freshness/parity/restart evidence;
4. call `register_current` to retire lower active persisted Product keys;
5. only then select an authoritative consumer.

This is one current contract, not a Product version matrix.

## Complete mutation ordering

Product scalar/translation/Variant-membership/EAV/tag state advances under the Product owner revision
boundary. Resolved SalesChannel membership advances under `relation_epoch`.

`product_index_graph_projection_snapshots.projection_epoch` remains the only complete Product mutation
`source_version`. Live replay requires the projection Product watermark to equal the current Product
revision and joins the exact retained relation epoch referenced by projection state.

The current source additionally materializes Product-owned tag UUIDs and canonical typed EAV terms in
the same PostgreSQL source read. EAV commands advance Product `index_revision` and graph projection
before their refresh ledger is captured.

Product hard-delete replay also uses projection epoch but does not decode live Storefront fields or
require a live relation freshness witness because the mutation removes the Product graph.

## Relation freshness gate

`product_sales_channel_index_relation_freshness_snapshots` identifies the exact retained relation epoch
and records observed Product source version, canonical Product visibility key, and tenant Channel
identity generation.

For every live Product row, the source fails closed unless the witness matches current visibility and
Channel identity generation and is not newer than current Product owner revision. Missing/stale
freshness therefore cannot publish a live Product mutation. Product locale absence uses the same gate.

Freshness-only Channel changes do not fabricate Product relation/projection epochs when resolved UUID
membership and Product record state are unchanged.

Detailed contracts:

- [Product graph projection ledger](../../rustok-product/docs/index-graph-projection-ledger.md)
- [Product-SalesChannel relation ledger](../../rustok-product/docs/index-sales-channel-relation-ledger.md)
- [Product-SalesChannel freshness witness](../../rustok-product/docs/index-sales-channel-relation-freshness.md)
- [Product typed EAV terms](./m7-product-attribute-term-contract.md)
- [Storefront parity gate](./m7-product-storefront-parity-gate.md)
- [Cross-owner resolver](./m7-product-sales-channel-resolver.md)

## Still open

The Product source contract itself is current, but production/Storefront admission still requires:

- staged current-key rebuild and final persisted supersession;
- actualization and execution of retained PostgreSQL packets on the current key/15-field contract;
- an explicit effective localized Product list identity/search/fallback architecture;
- any generic text-pattern primitive required by that chosen architecture;
- Storefront query translation and bounded Taxonomy tag-name hydration;
- full owner-vs-Index Storefront equivalence;
- Product typed event family/routes after event-contract digest admission;
- final traffic cutover evidence.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL execution, migrations, workflows, and CI
are maintainer-run. The implementation agent did not execute them.
