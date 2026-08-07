# M7 Product-to-SalesChannel cross-owner resolver

Status: `source_complete_event_wiring_and_atomic_snapshot_evidence_pending`.

## Purpose

The Product owner now persists a dedicated monotonic Product-to-SalesChannel relation epoch, but the
owner intentionally does not read Channel storage. `rustok-distribution` is the selected-module
composition layer that already sees both owners, so it is the correct boundary for resolving Product
visibility metadata into the current tenant SalesChannel UUID set.

`ProductSalesChannelRelationResolver` reads current Product visibility and current Channel identities,
then submits only the complete resolved UUID membership to
`ProductSalesChannelIndexRelationStore::replace`. It does not write Index rows, publish events, mutate
Product metadata, or own a background loop.

## Visibility policy

The resolver preserves the already published Product/Storefront semantics:

- missing `metadata.channel_visibility` is unrestricted visibility;
- an empty canonical `allowed_channel_slugs` array is unrestricted visibility;
- unrestricted visibility resolves to every current Channel identity for the tenant;
- a non-empty allowlist resolves to current Channel UUIDs whose `lower(btrim(slug))` matches one of
  the canonical Product slugs;
- malformed, non-canonical, duplicate, or non-string Product visibility fails closed;
- a deleted Channel is absent from the next observation and therefore leaves the resolved set;
- an unresolved restricted slug simply contributes no target until a matching Channel exists.

The resolver deliberately does **not** filter `channels.is_active`. Relation membership represents
identity resolution, while Channel runtime availability remains owned by the trusted Channel authority.
Toggling active state alone therefore does not create a new relation epoch. Channel create/delete,
slug movement, or identity movement can change membership and must converge through this resolver.

This distinction also closes the semantic trap from the owner-ledger slice: an empty Product slug
allowlist does not become an empty relation snapshot. Unrestricted Product visibility resolves against
the current tenant Channel universe.

## Bounded contract

The source contract is bounded at every externally variable collection:

- at most 1024 canonical Product visibility slugs;
- at most 1024 resolved Channel UUID targets, matching the Product-owned relation limit;
- at most 64 Products in one tenant convergence page;
- at most three exact Product stabilization attempts per reconciliation call.

Tenant sweep enumeration uses stable Product UUID keyset ordering with one-row lookahead. Exact
Product reconciliation can be used independently for Product-owned visibility changes.

## Cross-owner consistency boundary

There is no single database transaction owned by both Product and Channel domains in this slice.
Pretending otherwise would overstate the consistency contract.

For one exact Product the resolver instead performs bounded optimistic stabilization:

1. open one PostgreSQL `REPEATABLE READ`, `READ ONLY` transaction;
2. observe the exact Product visibility and its resolved current tenant Channel UUID set in that one
   snapshot;
3. commit the read-only observation;
4. call the Product-owned relation writer, which atomically advances membership plus relation epoch;
5. open a fresh `REPEATABLE READ`, `READ ONLY` observation;
6. if the resolved UUID set is unchanged, return the owner relation epoch;
7. otherwise repeat the sequence, at most three times, then fail with `ConcurrentChange`.

The Product-owned writer still fences Product hard deletion with its live-row `FOR KEY SHARE` lock.
If the Product disappears between enumeration, observation, or owner write, exact reconciliation
returns `ProductNotFound` and a tenant page records the row as gone rather than recreating relation
state.

This is a convergence primitive, not an atomic cross-owner snapshot, durable watermark, checkpoint,
or event acknowledgement. A continuously changing owner set can exhaust the bounded attempts and
must be retried by a future caller.

## Tenant convergence page

`reconcile_tenant_page` provides a bounded initial-backfill and Channel-change convergence primitive.
It enumerates at most 64 current tenant Products in UUID order and reconciles each exact Product
independently.

A page is intentionally not all-or-nothing. If a later Product fails after earlier owner epochs have
committed, rerunning from the same input cursor is safe: already converged Products return the owner
store's idempotent `Unchanged` result. Products created behind an already passed cursor are not
claimed as covered forever; durable Product events or a later tenant sweep remain required.

## Module boundary

Channel SQL exists only in `rustok-distribution`, alongside the already existing SalesChannel Index
source. `rustok-product` remains independent from `rustok-channel` and `rustok-index` and still accepts
only resolved UUID membership.

The resolver does not import Product Index schema types and does not add a Product-to-SalesChannel
`IndexLink`. Product v2 remains immutable.

## Still open

This slice does not yet provide:

- durable Product-visibility and Channel-identity event triggers or a relation watermark/checkpoint;
- a host worker, retry schedule, lease, broker cursor, or acknowledgement;
- one atomic Product+Channel cross-owner database snapshot;
- a new Product Index schema version;
- a relation replay source or locale fan-out adapter;
- Product-to-SalesChannel `IndexLink` materialization;
- retained PostgreSQL concurrency/restart/delete-recreate/out-of-order evidence;
- Storefront or production Index cutover.

The next unblocked source slice is a new Product Index schema version plus a relation replay adapter
that consumes the Product-owned relation epoch and fans it out to exact current Product locales.
Incremental typed event wiring remains separately gated on reviewed event-contract digest admission.

## Maintainer verification

```bash
cargo test -p rustok-distribution product_sales_channel -- --nocapture
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
