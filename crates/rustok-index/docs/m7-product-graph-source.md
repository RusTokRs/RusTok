# M7 canonical Product graph source

Status: `canonical_source_and_freshness_gate_complete_runtime_evidence_pending`.

The selected distribution publishes one current Product Index contract and one current ProductVariant
contract. Parallel Product compatibility implementations remain removed.

The generic Index `SchemaRef` still contains a positive numeric `SchemaVersion` because that is a core
storage/routing key, not a Product compatibility matrix.

## Canonical Product graph

`product-postgres-primary` emits one locale-required Product record with current Product scalars,
`variant_ids`, and `sales_channel_ids`. It materializes exactly two many-cardinality links:

- `variants` to ProductVariant identity;
- `sales_channels` to SalesChannel identity.

Product visibility slugs are resolver input only; they are not duplicated as transitional Index
fields.

Physical Product/translation deletes remain represented by Product-owned tombstones and canonical
`IndexMutation::Delete` mutations.

## Complete mutation ordering

Product scalar/translation/Variant-membership state advances under `products.index_revision`; resolved
SalesChannel membership advances under `relation_epoch`.

`product_index_graph_projection_snapshots.projection_epoch` is the only complete Product mutation
`source_version`. Live replay requires the current projection Product watermark to equal the current
Product revision and joins the exact retained relation epoch referenced by projection state.

Product hard-delete replay also uses projection epoch but does not require a live relation freshness
witness, because the mutation removes the Product graph.

## Relation freshness gate

The Product relation owner now also retains
`product_sales_channel_index_relation_freshness_snapshots`. A witness identifies the exact retained
relation epoch and records:

- observed Product source version;
- canonical Product visibility key;
- observed tenant Channel identity generation.

`rustok-channel` owns `channel_index_identity_generations`, which advances transactionally for Channel
insert/delete/id/tenant/canonical-slug changes.

For every live Product row, the source derives the current visibility key from Product metadata and
reads the current tenant Channel generation in the same PostgreSQL statement. It fails closed unless a
witness for the projection's exact relation epoch has:

- the same visibility key;
- the same Channel identity generation;
- a positive Product source watermark not newer than the current Product revision.

A missing or stale witness therefore cannot publish a live Product mutation. Product locale absence
uses the same gate and returns no absence watermark when freshness is stale.

Freshness-only changes do not advance `relation_epoch` or `projection_epoch` when the resolved UUID
membership and complete Product record are unchanged. The witness can advance independently.

Detailed contracts:

- [Product graph projection ledger](../../rustok-product/docs/index-graph-projection-ledger.md)
- [Product-SalesChannel relation ledger](../../rustok-product/docs/index-sales-channel-relation-ledger.md)
- [Product-SalesChannel freshness witness](../../rustok-product/docs/index-sales-channel-relation-freshness.md)
- [Cross-owner resolver](./m7-product-sales-channel-resolver.md)

## Ownership

`rustok-product` owns Product/Variant persistence, tombstones, relation membership, freshness witness
storage, and graph projection epochs. It still has no `rustok-index` or `rustok-channel` dependency.

`rustok-channel` owns the tenant identity generation. `rustok-distribution` owns cross-owner
observation/resolution and Index conversion.

## Still open

The source-level freshness gap is closed, but production admission still requires:

- retained PostgreSQL replay/freshness/concurrency/restart/delete-recreate evidence;
- automatic convergence scheduling/triggering if the desired latency requires it;
- persisted tenant schema readiness evidence;
- Product typed event family/routes after event-contract digest admission;
- tombstone retention admission, query equivalence, and Storefront cutover evidence.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL execution, migrations, workflows, and CI
are maintainer-run. The implementation agent did not execute them.
