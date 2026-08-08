# M7 canonical Product graph source

Status: `single_current_product_and_storefront_query_source_complete_execution_admission_pending`.

The selected distribution publishes one current Product Index contract and one current ProductVariant
contract. Parallel Product compatibility implementations remain removed.

The generic Index `SchemaRef` still contains a positive numeric routing key because it participates in
persisted schema/entity/link/inbox/replay identities. Current Product runtime code owns exactly one such
Product key, `4`; lower keys are historical storage identities only.

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

## Localized identity and Storefront query boundary

One physical Product Index entity still corresponds to one stored `product_translations.locale` row. The source
does not fabricate a requested-locale record from a fallback translation, and exact-locale absence remains a
source/readiness fact.

Storefront parity no longer tries to force requested/fallback semantics into that physical source shape. The
generic localized query path folds physical rows by logical identity `(tenant_id, schema_ref, entity_id)` and
projects requested locale -> fallback locale -> no localized row **after** matching/grouping semantics are
fixed. Logical identity is established before ordering, pagination and exact count, and entity-ID tie breaking
matches owner ascending/descending ordering.

Owner Storefront title search remains across **all Product translations**, not only the requested locale. The
localized Index contract represents that as an any-locale identity predicate while projecting only the
requested/fallback localized row. Generic scalar String `TextLike` is source-complete, and Product owns the
1022-byte effective Storefront search bound required to fit the Index 1024-byte `%...%` pattern budget.

The retained PostgreSQL localized fold/runtime and Storefront core packets cover this architecture in source.
Deployment/default collation parity remains an execution/admission gate because the Index PostgreSQL compiler
uses deterministic `COLLATE "C"` while the Product owner query uses its deployment/default collation.

## Product public projection after the fixed page

Generic Index results remain Product-neutral. When neither requested nor fallback Product translation exists,
raw `title`/`handle` stay `IndexValue::Null`.

The Product-specific public projection is derived only **after** raw page identity/order/count boundaries are
fixed:

- `title: Null` -> `"Untitled product"`;
- `handle: Null` -> `""`.

Product tags are also resolved after page selection through `ProductStorefrontTagReadPort`, keyed by the
already-selected Product IDs rather than only `tag_ids`. That preserves Taxonomy requested->fallback/canonical
name resolution and Product's legacy normalized `metadata.tags` fallback without adding localized tag names to
the Product Index schema.

## Storefront request-shape policy

Current key `4` has explicit fail-closed request-shape limits:

- trusted non-empty public channel slug + non-nil Channel UUID is Index-eligible;
- channel-less owner requests remain owner-native because `sales_channel_ids` cannot distinguish metadata-
  unrestricted Products from restricted Products that resolve to every current Channel;
- owner-valid offsets through `10_000` are Index-eligible;
- deeper owner-valid pages remain owner-native without clamp or cursor rewriting.

No visibility sentinel, Product key5 approximation, or compatibility schema is used to bypass those semantic
limits.

## Single-current immutable replacement

The previous Product schema fingerprint was not changed in place. Current runtime code uses one monotonically
higher internal routing key (`4`) and derives deterministic Product replay UUIDs with
`derive_index_schema_source_event_id`, so current-key rebuild deliveries cannot collide with lower-key delivery
identities for the same owner mutation coordinates.

The replacement source does not register or select a lower Product runtime contract. Tenant promotion remains
staged:

1. ordinary-register the exact current key4 immutable contract;
2. replay/rebuild key4 completely;
3. prove exact readiness/freshness/parity/restart evidence;
4. call `register_current` to retire lower active persisted Product keys;
5. require lower-key readiness/query execution to fail closed as inactive;
6. only then admit an authoritative Product Index consumer for that tenant.

A retained PostgreSQL promotion/restart packet now exists in source and uses production Product/Index
migrations, current distribution composition, current Product source, mutation storage, schema registration,
readiness/query verification and fresh runtime composition. Its lower key3 contract is storage/probe-only; it
does not reconstruct or select a historical key3 Product implementation. The packet remains unexecuted by the
implementation agent.

## Complete mutation ordering

Product scalar/translation/Variant-membership/EAV/tag state advances under the Product owner revision boundary.
Resolved SalesChannel membership advances under `relation_epoch`.

`product_index_graph_projection_snapshots.projection_epoch` remains the complete Product mutation
`source_version`. Live replay requires the projection Product watermark to equal the current Product revision
and joins the exact retained relation epoch referenced by projection state.

The current source additionally materializes Product-owned tag UUIDs and canonical typed EAV terms in the same
PostgreSQL source read. EAV commands advance Product `index_revision` and graph projection before their refresh
ledger is captured.

Product hard-delete replay also uses projection epoch but does not decode live Storefront fields or require a
live relation freshness witness because the mutation removes the Product graph.

## Relation freshness gate

`product_sales_channel_index_relation_freshness_snapshots` identifies the exact retained relation epoch and
records observed Product source version, canonical Product visibility key, and tenant Channel identity
generation.

For every live Product row, the source fails closed unless the witness matches current visibility and Channel
identity generation and is not newer than current Product owner revision. Missing/stale freshness therefore
cannot publish a live Product mutation. Product locale absence uses the same gate.

Freshness-only Channel changes do not fabricate Product relation/projection epochs when resolved UUID membership
and Product record state are unchanged.

## Non-serving budget boundary

The owner-first Storefront evidence path has a separate post-owner budget policy and budgeted projection
executor. Eligibility requires a host-measured remaining request budget, bounded Index/tag phases, safety
margin and Product tag capability. Eligible projected and tag phases are wrapped with outer Tokio timeouts while
the already-successful owner result remains authoritative.

A deterministic storage-free timeout packet is retained in source. It remains execution/admission pending and
does not mount the Index path into Storefront traffic.

## Detailed contracts

- [Product graph projection ledger](../../rustok-product/docs/index-graph-projection-ledger.md)
- [Product-SalesChannel relation ledger](../../rustok-product/docs/index-sales-channel-relation-ledger.md)
- [Product-SalesChannel freshness witness](../../rustok-product/docs/index-sales-channel-relation-freshness.md)
- [Product typed EAV terms](./m7-product-attribute-term-contract.md)
- [Product current-schema promotion](./m7-product-current-schema-promotion.md)
- [Storefront parity gate](./m7-product-storefront-parity-gate.md)
- [Storefront public projection](./m7-product-storefront-public-projection.md)
- [Storefront tag hydration](./m7-product-storefront-tag-hydration.md)
- [Storefront serving budget](./m7-product-storefront-serving-budget-policy.md)
- [Cross-owner resolver](./m7-product-sales-channel-resolver.md)

## Still open

The Product graph/query/Storefront source boundaries above are source-complete. Remaining gates are execution,
admission, and later serving composition:

- maintainer execution/admission of the retained Product key4 promotion/restart packet;
- maintainer execution/review of current-key Storefront core/EAV/collation and actualized Product PostgreSQL
  packets;
- deployment-specific admission of owner/default vs Index `COLLATE "C"` title-search parity;
- maintainer execution/admission of deterministic timeout/latency evidence;
- remaining stale-locale/readiness/admission/restart evidence not already covered by focused packets;
- Product typed event family/routes only after the separate M5 event-contract digest gate;
- real tenant stage/rebuild/promote only after evidence admission;
- final eligible Storefront traffic cutover last, while channel-less/deep-page shapes stay owner-native.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL execution, migrations, workflows, and CI are
maintainer-run. The implementation agent did not execute them.
