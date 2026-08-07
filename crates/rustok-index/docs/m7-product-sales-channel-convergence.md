# M7 Product-to-SalesChannel automatic convergence

Status: `source_complete_runtime_evidence_pending`.

## Selected boundary

The canonical Product Index relation already has three separate durable facts:

1. membership in Product-owned `relation_epoch` snapshots;
2. complete Product mutation ordering in Product-owned `projection_epoch` snapshots;
3. freshness witnesses proving the retained membership was resolved from current Product visibility
   and tenant Channel identity generation.

This slice adds the missing automatic convergence path without creating another Product schema or event
family.

Product owns:

- append-only visibility-change requests;
- tenant visibility cursor;
- Channel-generation checkpoint as an opaque numeric watermark;
- in-progress tenant sweep generation and Product keyset cursor;
- lease, retry availability, attempt count, and bounded last-error marker.

Channel continues to own `channel_index_identity_generations`. Distribution is the only layer that reads
both owners and invokes the existing resolver.

## Generic ModuleWork composition

The selected distribution registers `product_sales_channel_relation_convergence` through the generic
`ModuleWorkRegistrations` contract only when both Product and Channel runtime markers are present.
No Product or Channel module owns a background task and no host-side switch understands their storage.

One claim cycle is bounded:

- discover at most one due tenant;
- claim exactly one tenant lease under Product-owned `FOR UPDATE` state;
- process either one exact Product visibility request or one tenant page;
- tenant pages resolve at most 64 Products;
- commit one durable request/sweep checkpoint or retain the claim for retry.

There is no `tokio::spawn`, worker-local infinite loop, outbox family, broker topic, or new Product typed
Index event in this path. The deployment-owned `ModuleWorkScheduler` provides the existing lifecycle.

## Visibility changes

A Product INSERT or canonical `channel_visibility` change appends a Product-owned convergence request
after Product BEFORE triggers have produced the final metadata and `index_revision`.

Unrelated Product metadata changes do not enqueue relation work. Requests are consumed in tenant
sequence order. The worker resolves current Product owner state, so an older queued request is safe and
a later queued revision remains a durable idempotent follow-up.

Product deletion before request execution is terminal for that exact request: the Product no longer has
a live relation to publish, and hard-delete replay is already independent of freshness witnesses.

## Channel identity changes

The work source compares current Channel generation with the Product-owned completed checkpoint. A
missing checkpoint forces a baseline sweep. A newer Channel generation starts a bounded tenant sweep
with a fixed generation and Product UUID keyset cursor.

A Channel change during the pass cannot be lost. The old pass may complete, but it checkpoints only the
generation it started with; the newer current generation remains ahead and schedules another pass.

## Multi-host and restart behavior

Due discovery is intentionally read-only and may race across hosts. The Product-owned state row is the
authority: `FOR UPDATE` claim plus lease token allows only one durable claimant. An expired lease is
reclaimable; exact request cursor and tenant sweep cursor survive restart.

The DDL state guard mirrors the service state machine and prevents direct SQL from skipping requests,
forging a completed Channel checkpoint, changing an in-progress sweep generation, or deleting state.

## Admission boundary

Automatic relation convergence is now source complete. It closes the previous requirement that an
external caller manually invoke Product relation reconciliation after Product visibility or Channel
identity changes.

It does **not** close the materialized/query freshness window. Source observation and Index mutation
application are still separate transactions. A Channel identity change can commit after a Product
source page was read and before that page's mutation reaches `index_entities/index_links`. The next
source observation fails closed and this worker repairs the owner relation, but authoritative query
admission still needs a materialized/query freshness fence or equivalent retained evidence.

Therefore this slice does not authorize Storefront cutover.

## Remaining M7 admission

- execute PostgreSQL evidence for relation/freshness/convergence storage and multi-host lease recovery;
- retain Channel create/delete/slug/tenant, Product visibility, retry/restart/delete-recreate evidence;
- close the source-read -> mutation-apply materialized/query freshness window;
- admit canonical Product typed events/routes only after event-contract digest verification;
- prove complete Product/Variant/Channel query parity;
- move Storefront traffic only after readiness, equivalence, and materialized-freshness evidence pass.

## Maintainer verification

```bash
node scripts/verify/verify-index-product-channel-relation-convergence.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
