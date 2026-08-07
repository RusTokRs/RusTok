# Product to SalesChannel relation admission

Status: `resolver_source_complete_index_wiring_and_runtime_evidence_pending`

Rechecked on 2026-08-07 against the Product-owned durable relation ledger and the selected-module
Product/Channel composition boundary. The cross-owner resolver source is complete; a new Product Index
schema version, replay/materialization wiring, durable incremental triggers, and retained PostgreSQL
evidence remain open.

## Problem

The Product Index source owns Product scalar state, ProductVariant links, and Product-owned channel
visibility metadata. A real `IndexLink` from Product to the SalesChannel schema is different: Channel
create, delete, slug movement, or identity changes can alter the resolved target set without changing
`products.index_revision`.

A relation version must therefore not be derived with `max(product_revision, channel_revision)`, a
hash, truncated pairing function, timestamp, row count, or the current Product revision. Independent
owner counters do not guarantee that any of those encodings advances for every relation-only change.

## Compiled admission contract

`rustok-distribution::product_index::relation_admission` remains the database-neutral semantic gate:

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

The locale dimension remains in the Index-side snapshot because Product Index records are
locale-required. One Product-owned relation epoch may fan out to every current Product locale, with a
separate deterministic Index event identity for each locale.

## Product-owned durable owner storage

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

`rustok-distribution::product_index::channel_relation_resolver` now owns the source-level resolution
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

## Consistency boundary

The resolver deliberately does not claim one atomic Product+Channel transaction. For each exact
Product it performs bounded optimistic stabilization:

1. observe Product visibility plus Channel membership in one PostgreSQL `REPEATABLE READ`, `READ ONLY`
   snapshot;
2. call the Product-owned relation writer;
3. observe the same relation inputs again in a fresh read-only repeatable-read snapshot;
4. return only when the resolved UUID set is stable;
5. retry at most three times, then fail with `ConcurrentChange`.

This closes ordinary source-level races without inventing an unowned cross-owner lock. It remains a
convergence primitive, not a durable watermark, broker checkpoint, event acknowledgement, or proof
that every future owner mutation will trigger reconciliation.

The tenant page is likewise bounded and idempotent rather than atomic. If earlier Products commit and
a later Product fails, the page can be retried from the same input cursor because already converged
memberships return `Unchanged`.

Detailed resolver contract: `m7-product-sales-channel-resolver.md`.

## Production admission status

1. **Durable epoch storage for exact tenant/Product identity:** source complete.
2. **Strict increment when resolved membership changes:** source complete in the Product owner;
   Product/Channel membership discovery is source complete in the cross-owner resolver.
3. **Atomic membership + epoch commit:** source complete in the Product owner.
4. **Bounded current-state scan and targeted load:** source complete.
5. **Retained empty-membership state:** source complete for explicit replacement and Product hard
   delete; Channel-driven removal converges through resolver source, but durable triggering is pending.
6. **Stable event/source-version reuse:** relation epoch semantics are source complete; locale-specific
   Index conversion remains pending.
7. **Relation event descriptor, owner delivery route, broker adapter, commit-before-ack worker:**
   pending.
8. **PostgreSQL concurrency/restart/retry/delete-recreate/out-of-order/locale evidence:** pending.

## Index schema and wiring requirement

Product v2 cannot be modified in place because its schema fingerprint is already published. The real
Product-to-SalesChannel `IndexLink` therefore requires Product v3 (or the next reviewed Product schema
version) plus a relation replay adapter.

That future source must consume the Product-owned relation epoch, fan it out to exact current Product
locales, use the admitted relation event identity, and materialize SalesChannel UUID targets without
moving Channel SQL into `rustok-product`.

Incremental typed event wiring remains separately gated on canonical event-contract digest admission.
The committed digest artifact changed on `main` after #3130, so the older statement that the exact
artifact was known stale is no longer source-current. However the maintainer admission document still
marks canonical generator/verify execution pending; source inspection alone therefore does not prove
the current hashes are admitted. Product typed events remain blocked until that verification is
retained.

## Explicit non-claims

This slice does not yet add:

- durable Product/Channel event triggers or a relation watermark/checkpoint;
- a host-owned resolver loop, lease, retry scheduler, broker cursor, or acknowledgement;
- one atomic cross-owner Product+Channel snapshot;
- Product v3 or a Product-to-SalesChannel `IndexLink`;
- a relation replay source or locale fan-out materializer;
- retained PostgreSQL runtime evidence;
- Storefront or production partition cutover.

`allowed_channel_slugs` remains Product-owned desired visibility metadata. The relation ledger is the
resolved relation owner state, while the distribution resolver is only the current convergence
mechanism. None of these source-complete pieces alone prove production cutover readiness.

## Maintainer validation

```bash
cargo test -p rustok-product index_channel_relation --lib -- --nocapture
cargo test -p rustok-distribution product_sales_channel -- --nocapture
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
