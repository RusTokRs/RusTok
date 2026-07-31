# M7 Product and ProductVariant tombstone replay

Status: `source_complete_owner_execution_pending`

## Scope

This slice extends the existing stable Product replay sources with durable hard-delete state for all
published Product schema versions:

- locale-aware `rustok-product::product@1` and `rustok-product::product@2`;
- non-localized `rustok-product::product_variant@1` and
  `rustok-product::product_variant@2`.

No schema field, link, fingerprint, source name, cursor shape, or replay event domain changes. The
existing source identities remain:

- `product-postgres-primary` for Product v1 and v2;
- `product-variant-postgres-primary` for ProductVariant v1 and v2.

The same source now emits `IndexMutation::Upsert` for live owner rows and
`IndexMutation::Delete` for retained owner tombstones.

## Owner storage

`rustok-product` owns two PostgreSQL tables:

- `product_index_tombstones`, keyed by `(tenant_id, product_id, locale)`;
- `product_variant_index_tombstones`, keyed by `(tenant_id, variant_id)`.

Each row stores a positive `source_version` and deletion timestamp. These are Product-owned replay
facts, not Index storage and not public Product API entities.

Product deletion captures every currently stored translation locale before cascade removal. Direct
translation deletion captures only the removed locale. Translation identity moves tombstone the old
`(tenant_id, product_id, locale)` key and advance the new parent revision for the replacement live
key.

ProductVariant deletion captures the exact non-localized Variant key. Variant ID or tenant moves
capture the old key while the updated row publishes the new key.

## Monotonic delete versions

A hard-delete tombstone uses a source version strictly greater than the last live row for that exact
identity:

- Product and ProductVariant row deletion retain `OLD.index_revision + 1`;
- direct translation deletion first advances the parent Product revision and retains that returned
  revision;
- identity moves use the revision advanced by the owner write.

Revision exhaustion fails the owner write instead of wrapping.

Retained tombstones also protect UUID and locale reuse. A recreated Product or ProductVariant is
seeded above the highest retained delete revision. A new live translation or Variant clears its
matching tombstone only when its live revision is strictly greater. Equal or lower live revisions
fail closed.

## Replay contract

Each PostgreSQL source scans one tenant-scoped union of live rows and retained tombstones:

- Product keeps stable `(product_id, locale)` ordering;
- ProductVariant keeps stable `variant_id` ordering;
- one-row lookahead remains unchanged;
- targeted loads return either the current live mutation or the retained delete for an exact key.

The source computes `identity_count` over each exact key. Live/tombstone coexistence is treated as
storage drift and produces a permanent source-record failure. The adapter never chooses one side
nondeterministically and never returns duplicate mutation keys.

Delete event IDs continue to use the existing deterministic source-event helper with the exact v1
or v2 event domain, tenant, entity, locale, and retained source version. Therefore duplicate replay
of the same tombstone is idempotent, while later recreation can supersede it with a higher version.

## Ownership boundary

`rustok-product` owns tombstone persistence, revision advancement, identity-reuse protection, and
migration rollback. It still has no dependency on `rustok-index`.

`rustok-distribution` owns generic conversion from owner rows into `IndexMutation::Upsert` or
`IndexMutation::Delete`. It does not write Index tables, start tasks, or own retries.

Index core, server composition, Product DTOs, and SeaORM write models remain unchanged.

## Explicitly open

- incremental Product/translation/ProductVariant event ingestion and broker acknowledgement;
- tombstone retention and purge policy after every admitted consumer checkpoint is proven newer;
- repeatable-read full-tenant replay snapshots and reconciliation for concurrent identities that
  sort behind an active cursor;
- persisted per-tenant v2 schema application and authoritative Storefront cutover;
- durable Product/ProductVariant-to-SalesChannel UUID relations and cross-owner revision semantics;
- retained PostgreSQL hard-delete/recreate, restart, drift, and equivalence evidence;
- retry/backoff/dead-letter scheduling and graceful host task ownership.

Soft-delete fields are not reinterpreted by this slice. Only physical Product, Product translation,
and ProductVariant identity removal produces retained tombstones.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, PostgreSQL trigger/source execution, and CI are
maintainer-run. The implementation agent did not execute them.
