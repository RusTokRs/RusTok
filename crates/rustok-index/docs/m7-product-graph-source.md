# M7 canonical Product graph source

Status: `canonical_source_complete_runtime_evidence_pending`

The selected distribution now publishes exactly one current Product Index contract and one current
ProductVariant contract. The old parallel Product/ProductVariant compatibility implementations have
been removed from runtime code.

The generic Index `SchemaRef` still contains a positive numeric `SchemaVersion` because it is part of
the storage and routing key for every module. That implementation detail no longer represents a set of
coexisting Product contracts: only one Product and one ProductVariant schema are registered.

## Canonical Product

`product-postgres-primary` emits one locale-required Product record containing:

- `id`;
- `status`;
- `title`;
- `handle`;
- `description`;
- `vendor`;
- `product_type`;
- `primary_category_id`;
- `variant_ids`;
- `sales_channel_ids`.

It materializes two many-cardinality links:

- `variants` to the current ProductVariant identity;
- `sales_channels` to the current SalesChannel identity.

Product visibility slugs remain Product-owner input for the cross-owner resolver. Transitional
`channel_restricted` and `allowed_channel_slugs` fields are no longer part of the Index schema because
resolved SalesChannel UUID membership is now the graph contract.

The source scans stable `(product_id, locale)` identities and requires exact locale-bearing targeted
loads. Physical deletes are retained through `product_index_tombstones` and emit the same canonical
Product `IndexMutation::Delete` contract.

## Canonical ProductVariant

`product-variant-postgres-primary` publishes one non-localized ProductVariant schema including its
stable `id` and current scalar fields. It scans by stable `variant_id`, supports exact targeted loads,
and uses `product_variant_index_tombstones` for retained deletes.

There is no reverse ProductVariant-to-Product link because Product is locale-required while
ProductVariant is not. Product owns the forward `variants` graph link.

## Complete Product mutation clock

Product scalar/translation/Variant-membership state advances under `products.index_revision`, while
resolved Product-to-SalesChannel membership advances under the Product-owned `relation_epoch`. A full
Product record cannot safely use either independent counter directly.

`product_index_graph_projection_snapshots` therefore owns `projection_epoch`, the only complete Product
record mutation `source_version`. The canonical Product source requires projection state, joins the
exact retained SalesChannel relation referenced by that projection, and fails closed when the live
Product revision is newer than the retained Product projection watermark.

The Product locale absence provider uses the same projection epoch and likewise refuses to manufacture
an absence watermark from a newer unprojected Product revision.

Detailed owner contract:
[Product graph projection ledger](../../rustok-product/docs/index-graph-projection-ledger.md).

## Ownership

`rustok-product` owns Product/Variant persistence, revisions, tombstones, relation snapshots, and graph
projection epochs. It has no `rustok-index` or `rustok-channel` dependency.

`rustok-distribution` owns selected cross-module conversion and Product visibility to Channel identity
resolution. It reads the Product-owned projection/relation state but never moves Channel SQL into the
Product crate.

## Freshness boundary

A correct monotonic projection clock does not by itself prove that SalesChannel relation membership is
fresh after Product visibility or Channel identity changes. The bounded resolver exists, but durable
convergence triggering or an admitted freshness watermark remains required before this graph becomes
authoritative.

Also still open:

- retained PostgreSQL replay/concurrency/restart/delete-recreate evidence;
- persisted tenant schema readiness evidence for the one current contract set;
- Product typed event family and concrete consumer routes after event-contract digest admission;
- tombstone retention/purge admission;
- Storefront cutover and full query equivalence evidence.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL execution, migrations, workflows, and CI
are maintainer-run. The implementation agent did not execute them.
