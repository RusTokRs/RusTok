# Product-SalesChannel Index relation owner ledger

Status: `canonical_graph_source_complete_freshness_and_runtime_evidence_pending`.

## Purpose

Product visibility metadata is expressed with Channel slugs, while the canonical Product Index graph
links to stable SalesChannel UUID identities. `product_sales_channel_index_relation_snapshots` is the
Product-owned durable boundary for that resolved UUID membership.

It deliberately does not resolve slugs, read Channel tables, construct Index mutations, or publish a
broker event.

## Storage contract

Each append-only snapshot contains:

- positive tenant-local `sequence_no`;
- exact non-nil `tenant_id` and `product_id`;
- positive contiguous `relation_epoch` beginning at `1`;
- complete resolved Channel UUID membership as canonical JSON.

Membership is bounded to 1024 non-nil, strictly sorted, unique UUIDs. Empty membership is valid and
means no currently resolved Channel targets.

An empty resolved UUID set is not the same as an empty Product `allowed_channel_slugs` array: Product
visibility treats an empty allowlist as unrestricted, and the distribution resolver expands it to the
current tenant Channel identity universe.

Equal membership is idempotent and does not advance the epoch. Retained snapshots cannot be updated or
deleted.

## Product-owned write API

`ProductSalesChannelIndexRelationStore::replace` validates identity/membership, locks the exact live
Product under `FOR KEY SHARE`, serializes the relation identity with a PostgreSQL advisory transaction
lock, and atomically appends the next relation epoch only when membership changes.

The lock order fences stale resolution against Product deletion. Product hard delete takes the Product
row first and then the same relation lock, retaining final empty membership when needed.

The same owner boundary exposes bounded change pages, current scans, and targeted current loads.

## Cross-owner composition

`rustok-distribution::product_index::channel_relation_resolver` reads Product visibility and current
tenant Channel identities, then submits only the complete resolved UUID set to this store. The Product
crate remains unaware of Channel storage and types.

The resolver uses bounded observe/write/re-observe stabilization. Durable owner-change triggering or a
freshness watermark remains open.

## Canonical Product graph

The relation epoch is authoritative for relation membership changes. The complete Product Index record
also contains Product scalar/translation/ProductVariant state, so it uses the independent
`product_index_graph_projection_snapshots.projection_epoch` as its full-record `source_version`.

The projection ledger retains both Product and relation watermarks. The canonical Product source reads
the exact relation epoch referenced by the latest projection and materializes `sales_channel_ids` plus
the many `sales_channels` link.

Detailed projection contract:
`crates/rustok-product/docs/index-graph-projection-ledger.md`.

## Module boundary

`rustok-product` has no `rustok-index` or `rustok-channel` dependency. Relation storage accepts only
UUID membership. Cross-module identity resolution and Index conversion stay in `rustok-distribution`.

## Still open

- durable Product/Channel convergence triggers or an admitted freshness checkpoint;
- typed Product relation/event delivery after event-contract digest admission;
- retained PostgreSQL concurrency, restart, delete/recreate, retry, out-of-order, locale fan-out, and
  freshness evidence;
- Storefront or production Index cutover.

## Maintainer verification

```bash
cargo test -p rustok-product index_channel_relation --lib -- --nocapture
cargo test -p rustok-distribution product_sales_channel -- --nocapture
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-graph-projection-ledger.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
