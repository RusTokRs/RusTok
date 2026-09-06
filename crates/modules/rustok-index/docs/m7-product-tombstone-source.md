# M7 Product and ProductVariant tombstone replay

Status: `canonical_source_complete_owner_execution_pending`

## Scope

The current Product and ProductVariant replay sources retain physical-delete identities instead of
losing them when owner rows disappear.

The two current source identities are:

- `product-postgres-primary` for locale-aware Product graph replay;
- `product-variant-postgres-primary` for non-localized ProductVariant replay.

Both sources emit `IndexMutation::Upsert` for live owner state and `IndexMutation::Delete` for retained
owner tombstones. There are no parallel Product/ProductVariant compatibility source branches.

## Owner storage

`rustok-product` owns:

- `product_index_tombstones`, keyed by `(tenant_id, product_id, locale)`;
- `product_variant_index_tombstones`, keyed by `(tenant_id, variant_id)`.

Product deletion captures every current translation locale before cascade removal. Direct translation
deletion retains only the removed locale. Translation identity movement retains the old exact key.

ProductVariant deletion retains the exact non-localized Variant key. Variant identity movement retains
the old key while the updated row owns the new key.

These are Product-owned replay facts, not Index storage or public Product API entities.

## Monotonic owner delete watermarks

Product/ProductVariant tombstones retain positive owner source watermarks above the last live owner
revision for the deleted identity. Revision exhaustion fails closed.

Retained tombstones also fence identity reuse: recreated owner identities are seeded above retained
delete watermarks and matching tombstones are cleared only after a strictly newer live owner revision
exists.

For Product specifically, that owner tombstone watermark is an **input** to
`product_index_graph_projection_snapshots`. The canonical Product Index mutation itself uses
`projection_epoch`, because the complete Product record also includes resolved SalesChannel
membership under an independent relation epoch.

ProductVariant remains fully owner-versioned by its own `index_revision`.

## Replay contract

- Product scans stable `(product_id, locale)` identities and combines current rows with retained
  Product tombstones. The canonical Product row additionally requires graph projection state and uses
  its `projection_epoch` for both live and delete mutations.
- ProductVariant scans stable `variant_id` identities and combines current rows with retained Variant
  tombstones.
- both sources retain one-row lookahead and exact targeted-load semantics;
- `identity_count` rejects live/tombstone coexistence rather than choosing nondeterministically.

Canonical delete event identity uses the current Product or ProductVariant replay event domain plus the
exact mutation source version. Replaying the same retained identity is therefore deterministic.

## Ownership boundary

`rustok-product` owns tombstone persistence, owner revisions, identity-reuse protection, relation
snapshots, and Product graph projection epochs. It has no `rustok-index` dependency.

`rustok-distribution` owns conversion from retained owner state into generic Index mutations. It does
not write Index tables or start retry/background tasks.

## Still open

- typed incremental Product/ProductVariant event delivery after event-contract digest admission;
- tombstone purge admission after consumer checkpoints are proven newer;
- retained PostgreSQL hard-delete/recreate/restart/drift/equivalence evidence;
- durable Product/SalesChannel relation freshness triggering;
- authoritative Storefront cutover.

Soft-delete fields are not reinterpreted. Only physical Product, Product translation, and
ProductVariant identity removal produces retained tombstones.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL trigger/source execution, migrations,
workflows, and CI are maintainer-run. The implementation agent did not execute them.
