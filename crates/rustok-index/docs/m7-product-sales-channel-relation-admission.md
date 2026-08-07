# Product to SalesChannel relation admission

Status: `resolver_and_projection_epoch_source_complete_v3_wiring_and_runtime_evidence_pending`

Rechecked on 2026-08-07 against the Product-owned durable relation ledger, the selected-module
Product/Channel resolver, and the Index mutation-store monotonicity contract. The cross-owner resolver
source is complete and Product now has a dedicated graph-v3 projection epoch; Product v3 replay,
durable convergence triggering, and retained PostgreSQL evidence remain open.

## Problem

The Product Index source owns Product scalar state, ProductVariant links, and Product-owned channel
visibility metadata. A real `IndexLink` from Product to the SalesChannel schema is different: Channel
create, delete, slug movement, or identity changes can alter the resolved target set without changing
`products.index_revision`.

A relation version must therefore not be derived with `max(product_revision, channel_revision)`, a
hash, truncated pairing function, timestamp, row count, or the current Product revision. Independent
owner counters do not guarantee that any of those encodings advances for every relation-only change.

A second constraint matters for Product v3: Index accepts only full `Upsert`/`Delete` mutations and
stale-ignores an incoming mutation when its `source_version` is less than or equal to the already
materialized version. A full Product v3 record therefore cannot safely use either
`products.index_revision` or the relation `relation_epoch` directly; either choice can hide a change
from the other input family.

## Compiled relation admission contract

`rustok-distribution::product_index::relation_admission` remains the database-neutral relation gate:

- `ProductSalesChannelRelationEpoch` is a positive `u64` relation version;
- `ProductSalesChannelRelationSnapshot` is scoped by exact non-nil tenant, non-nil Product, and
  canonical locale;
- channel UUIDs must be non-nil and unique, and are sorted canonically;
- an empty set is valid and represents removal of all Product-to-Channel links;
- event identity is derived with the versioned
  `rustok-distribution.product-sales-channel-relation-v1` domain and the exact relation epoch;
- an identical epoch is accepted only for an identical retry;
- changed membership under the same epoch fails closed;
- epoch regression and tenant/Product/locale scope changes fail closed;
- a strictly greater epoch is admitted as an advanced relation snapshot.

The locale dimension remains in the Index-side relation snapshot because Product Index records are
locale-required. One Product-owned relation epoch may fan out to every current Product locale, with a
separate deterministic relation identity for each locale.

## Product-owned durable relation storage

`rustok-product` owns `product_sales_channel_index_relation_snapshots` and
`ProductSalesChannelIndexRelationStore`.

The owner storage is append-only and independent from both owner revision columns. For each exact
`(tenant_id, product_id)` identity it persists one positive sequence number, one positive contiguous
relation epoch beginning at `1`, and the complete resolved Channel UUID set as canonical bounded JSON.

The writer first requires the exact live Product row under `FOR KEY SHARE`, then serializes one
relation identity with a PostgreSQL advisory transaction lock. Equal membership is returned as an
idempotent `Unchanged` result without advancing the epoch. Changed membership appends exactly one
next epoch in the same transaction.

That lock order fences a stale resolver against concurrent Product deletion. Product deletion owns
the Product row first and its `AFTER DELETE` trigger then takes the same relation advisory lock, so a
post-delete non-empty relation cannot be appended after the retained empty epoch.

The owner API also provides bounded append-only change pages, current-state scans in Product UUID
order, and exact current targeted loads. It does not read Channel tables or import Channel types.

Detailed owner contract: `crates/rustok-product/docs/index-sales-channel-relation-ledger.md`.

## Cross-owner resolver

`rustok-distribution::product_index::channel_relation_resolver` owns the source-level resolution
boundary. It reads Product metadata and current tenant Channel identities, then calls only
`ProductSalesChannelIndexRelationStore::replace` with the complete resolved UUID set.

The reviewed policy is explicit:

- missing Product `channel_visibility` or an empty canonical `allowed_channel_slugs` array means
  unrestricted visibility;
- unrestricted visibility resolves to every current Channel identity for the tenant;
- a non-empty allowlist resolves Channel UUIDs by canonical `lower(btrim(slug))` membership;
- malformed or non-canonical Product visibility fails closed;
- Channel `is_active` does not alter relation membership; runtime availability remains Channel-owned;
- Channel create/delete/slug/identity changes can alter the relation and require convergence.

The resolver is bounded to 1024 visibility slugs, 1024 resolved Channel UUIDs, 64 Products per tenant
page, and three exact Product stabilization attempts.

For each Product it performs bounded observe/write/re-observe stabilization using read-only
`REPEATABLE READ` observations. This closes ordinary source-level races without inventing one
cross-owner transaction. It remains a convergence primitive, not a durable watermark, broker
checkpoint, event acknowledgement, or proof that every future owner mutation triggers resolution.

Detailed resolver contract: `m7-product-sales-channel-resolver.md`.

## Product v3 projection epoch

The relation epoch is suitable for versioning relation membership, but not for versioning one full
Product v3 record that also carries Product scalar and ProductVariant state. Product therefore now
owns `product_index_graph_v3_projection_snapshots`.

For one exact tenant/Product identity it retains:

- a positive contiguous `projection_epoch` beginning at `1`;
- the latest retained Product `index_revision`/tombstone source-version watermark;
- the latest Product-to-SalesChannel `relation_epoch` watermark.

The two input watermarks may only advance. Whenever either advances, the canonical Product function
appends exactly one next `projection_epoch`; an identical input pair is an idempotent no-op. Direct
snapshot inserts are guarded by the same per-Product advisory lock and must obey the same contiguous,
non-regressing contract. Retained rows are append-only.

The migration reconciles projection state after Product insert, Product `index_revision` change,
Product hard delete, and relation-snapshot insert. Existing live/retained Product identities with an
existing relation snapshot receive an epoch-1 backfill.

Product hard delete is ordered deliberately: the v3 Product delete trigger sorts after
`trg_products_retain_empty_channel_relation`, so the final empty relation epoch is retained first and
its relation trigger advances projection state before the trailing Product delete reconciliation.

The `GREATEST` operations inside projection reconciliation do **not** derive the Index source version.
They merge already retained component watermarks so a concurrent observer cannot regress one input.
The separate `projection_epoch` is the future Product v3 full-record source version.

Detailed owner contract:
`crates/rustok-product/docs/index-graph-v3-projection-ledger.md`.

## Production admission status

1. **Durable relation epoch storage:** source complete.
2. **Resolved membership discovery:** source complete in the bounded cross-owner resolver.
3. **Atomic membership + relation epoch commit:** source complete in the Product owner.
4. **Bounded relation current/change reads:** source complete.
5. **Retained empty membership on Product hard delete:** source complete.
6. **Independent full-record Product v3 source-version arbitration:** source complete via
   `projection_epoch`.
7. **Product v3 schema/source, SalesChannel `IndexLink`, and v3 absence semantics:** pending.
8. **Durable Product-visibility/Channel-identity convergence triggering or admitted freshness
   watermark:** pending.
9. **Typed event route, broker adapter, commit-before-ack worker:** pending.
10. **PostgreSQL concurrency/restart/retry/delete-recreate/out-of-order/locale evidence:** pending.

## Why Product v3 is still not published in this slice

Product v2 cannot be modified in place because its schema fingerprint is already published. Product
v3 will need to preserve the existing stable `product-postgres-primary` source identity while adding a
many-cardinality SalesChannel link.

This pass intentionally stops before that schema/source change. The source-version recheck found that
the previous plan wording — using relation epoch directly as the Product v3 source version — was
unsafe under Index stale-ignore semantics. The dedicated projection epoch is therefore admitted first
as a separate owner prerequisite rather than hiding that correction inside a large replay change.

There is also a distinct freshness boundary: projection epoch monotonicity does not prove that the
latest relation membership has already converged after the newest Product visibility or Channel
identity change. The current resolver can converge it, but durable triggering/watermark evidence is
still open. Product v3 replay may be implemented next, but authoritative use remains forbidden until
that freshness gate is closed and evidenced.

Incremental typed event wiring remains separately gated on canonical event-contract digest admission.
The committed digest artifact changed on `main` after #3130, but this pass did not execute the
canonical generator or retain a successful verify packet. Product typed events therefore remain
blocked.

## Explicit non-claims

This slice does not yet add:

- Product v3 or a Product-to-SalesChannel `IndexLink`;
- a Product v3 replay or absence source;
- durable Product/Channel event triggers or a relation freshness watermark/checkpoint;
- a host-owned resolver loop, lease, retry scheduler, broker cursor, or acknowledgement;
- one atomic cross-owner Product+Channel snapshot;
- retained PostgreSQL runtime evidence;
- Storefront or production partition cutover.

`allowed_channel_slugs` remains Product-owned desired visibility metadata. The relation ledger owns
resolved membership, the resolver computes current membership, and the new projection ledger only
arbitrates a monotonic full-record version across Product and relation input counters. None of these
source-complete pieces alone prove production cutover readiness.

## Maintainer validation

```bash
node scripts/verify/verify-index-product-v3-projection-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
