# M7 Product source adapter

Status: `source_complete_owner_execution_pending`

## Scope

When `mod-product` is selected, `rustok-distribution` publishes Product schema versions
`rustok-product::product@1` and `@2` through one stable PostgreSQL source,
`product-postgres-primary`.

Product v1 retains the original locale-required scalar contract. Product v2 adds the stable Product
identity field, bounded channel-visibility fields, variant identities, and the schema-declared
Product-to-ProductVariant link described in the graph contract.

Index core and the server remain Product-domain agnostic.

## Ownership and composition

`rustok-product` owns:

- `products` and `product_translations`;
- positive monotonic `products.index_revision`;
- Product and locale tombstone storage;
- triggers that advance the revision for Product and translation changes;
- identity-reuse fencing against retained tombstones.

The Product crate does not depend on `rustok-index`. It publishes only
`ProductRuntimeSelected`; `rustok-distribution` registers schemas and database-aware factories.

Host factory materialization constructs adapters without SQL or task startup and commits the staged
source/absence catalogs atomically only after every selected factory succeeds.

## Source contract

The Product PostgreSQL source supports:

- cursor scans ordered by stable `(product_id, locale)` identity;
- one-row lookahead over the caller's bounded page limit;
- targeted loads over exact `(product_id, locale)` pairs;
- exact tenant and schema scope for Product v1 and v2;
- canonical locale and cursor validation;
- stable replay event UUIDs derived from tenant, Product, locale, event domain, and source revision;
- retryable storage failures and permanent contract/backend failures without raw database details.

The source reads one union of live Product/translation rows and retained
`product_index_tombstones`. It emits generic `IndexMutation::Upsert` or `IndexMutation::Delete`
values and never writes Index storage directly.

Live/tombstone coexistence for one exact tenant/Product/locale identity fails closed through the
row identity-count contract. Stable enumeration excludes mutable revision values from the cursor.

## Monotonic source version and retained deletes

`products.index_revision` is a positive `BIGINT`. Product updates and ProductVariant membership
changes advance it. Translation INSERT, UPDATE, DELETE, locale move, tenant move, or Product move
also advances the affected Product revisions.

Translation deletion or identity movement stores an exact locale tombstone at the new revision.
Product hard delete stores tombstones for every retained translation locale. Product identity reuse
seeds the live revision above retained tombstones and clears only tombstones strictly superseded by
the new live revision.

The revision is storage-internal and is used as generic mutation `source_version` and replay event
identity, not as the scan cursor.

## Explicit locale-absence high-watermark

The selected distribution also publishes `product-locale-absence-postgres` for Product v1 and v2.
For a live Product with no requested translation and no exact retained tombstone, the provider
returns positive `products.index_revision` as proof that the exact locale is absent at that owner
version.

This proof is separate from ordinary targeted load and is consumed only by bounded drift snapshot
capture. Tombstoned locales remain ordinary `Delete` mutations; unknown Product identities remain
non-authoritative.

## Persisted tenant readiness

The generic [`M7 tenant schema readiness gate`](./m7-schema-readiness.md) is source complete. It can
require the exact selected Product/ProductVariant/SalesChannel schema set for one tenant against the
immutable runtime registry and persisted `index_schemas` rows. Missing, inactive, fingerprint-drifted,
or schema-JSON-drifted contracts fail the complete readiness request closed.

The readiness gate does not apply missing schemas automatically and does not authorize cutover by
itself. Owner verification and the remaining replay/equivalence/freshness admission still apply.

## Explicitly open

- production Product/ProductVariant mutation-event routes and concrete broker consumer wiring after
  canonical event-contract digest admission;
- owner execution/evidence for the persisted tenant schema readiness gate;
- complete durable Product-to-SalesChannel relation semantics;
- tombstone purge admission and retained retention evidence;
- real PostgreSQL replay, restart, absence-diagnosis, freshness, and equivalence evidence;
- Storefront/admin/search authoritative consumer cutover.

No repeatable-read full-tenant owner snapshot is claimed by the replay source. Reconciliation remains
required for concurrent inserts that sort behind an active cursor.

## Owner verification

The implementation agent did not run commands. The repository owner should run:

```bash
node scripts/verify/verify-index-product-source.mjs
node scripts/verify/verify-index-product-graph-source.mjs
node scripts/verify/verify-index-schema-readiness.mjs
node scripts/verify/verify-index-source-absence-watermark.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-distribution --all-targets --features mod-product
cargo check -p rustok-server --all-targets --features mod-product
cargo test -p rustok-distribution product_index --features mod-product -- --nocapture
```

Validation and live PostgreSQL execution are `maintainer-run`.
