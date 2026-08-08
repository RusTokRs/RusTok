# M7 linked-target replay/redelivery PostgreSQL harness

Status: `source_ready_execution_pending`.

## Purpose

`product_linked_target_replay_redelivery_postgres.rs` composes the existing generic replay contract with
the canonical ProductVariant source, real PostgreSQL mutation storage, Product linked-target availability,
and query-runtime recomposition.

No new replay mechanism, owner clock, Product schema, relation ledger, or compatibility version is added.
The packet exists because generic M5 unit evidence already proves the replay ordering contract in
isolation, while Product M7 still needed retained composition evidence that duplicate/out-of-order target
delivery cannot regress linked graph authority.

## Existing generic replay contract reused

`IndexReplayWorker` already:

- scans one bounded current source page;
- validates event UUIDs before persistence;
- applies every mutation before checkpoint commit;
- leaves the checkpoint unchanged if checkpoint commit fails after mutation durability;
- repeats the same source page/event after that failure;
- reports `Applied`, `Duplicate`, and `StaleIgnored` independently.

`PostgresMutationStore` already:

- derives replay delivery identity from the stable mutation event UUID;
- stores inbox and entity/link mutation atomically;
- reports exact stable-event redelivery as `Duplicate`;
- serializes one exact entity key before comparing source versions;
- ignores a never-before-delivered mutation whose source version is not newer than the current durable
  entity version;
- never rewrites current payload/links for `Duplicate` or `StaleIgnored` outcomes.

This packet does not replace those generic contracts. It proves their Product graph composition.

## Fixture

One tenant owns:

- Product with one English translation;
- one ProductVariant;
- one SalesChannel `alpha`;
- Product visibility restricted to `alpha` through canonical owner metadata.

Real Channel, Product, and Index migrations are applied. The selected Index + Channel + Product
distribution runtime provides:

- canonical Product and ProductVariant schemas/sources;
- persisted tenant schema readiness;
- generic Product/Channel convergence work for initial relation/freshness state;
- generic `PostgresMutationStore`;
- canonical shared Index query runtime.

Only `variants` is traversed by the graph query, so the query-path-scoped availability policy does not
require SalesChannel target materialization.

## Initial state

The Product mutation and Variant v1 mutation are loaded from the canonical source registry and applied
to PostgreSQL. The Product graph query must expose Variant SKU `variant-v1` with exact count 1.

Then the owner Variant is updated twice without changing Variant identity/membership:

1. v2 SKU `variant-v2-never-applied` is loaded from the canonical source adapter and retained in memory,
   but deliberately never delivered;
2. owner advances again to v3 SKU `variant-v3-current`, also loaded from the canonical source adapter.

The Product owner/projection stays current because SKU-only updates do not trigger Product membership
revision. The materialized Variant remains v1, so scalar Product query stays authoritative while the
linked `variants` graph query fails closed with exact count 0.

## Crash after mutation durability, before checkpoint

The packet constructs a real `IndexReplayWorker` over:

- the canonical `SharedIndexSourceRegistry`;
- the canonical shared schema registry;
- real PostgreSQL `PostgresMutationStore`;
- a bounded fail-once checkpoint adapter used only to inject the exact crash boundary.

The checkpoint adapter is intentionally not a replacement persistence implementation. Generic M5
already owns durable PostgreSQL checkpoint/lease semantics. Here it deterministically fails only the
first checkpoint commit so the packet can prove Product graph behavior at the mutation/checkpoint
boundary without adding test-only hooks to production storage.

On the first replay page:

1. canonical ProductVariant scan returns current v3 only;
2. real PostgreSQL mutation persistence applies v3;
3. checkpoint commit is injected to fail;
4. worker returns `CheckpointCommitFailed` and checkpoint remains absent.

Even though replay cursor durability failed, the durable target is already current. Both the existing
query runtime and a separately composed query runtime on a fresh PostgreSQL session must expose
`variant-v3-current` with exact count 1. Query authority therefore does not depend on process-local replay
checkpoint state.

## Worker restart and exact redelivery

A new `IndexReplayWorker` instance is created with the same canonical source registry, PostgreSQL
mutation store, and still-empty checkpoint state.

The retry scans current v3 again and derives the same stable event UUID. The PostgreSQL inbox must
classify the mutation as `Duplicate`; the page must report:

- `mutation_count = 1`;
- `applied_count = 0`;
- `duplicate_count = 1`;
- `stale_count = 0`;
- `Complete` checkpoint with source version v3 and the exact current event UUID.

Graph payload remains v3 throughout.

## Late never-delivered historical mutation

The retained canonical v2 mutation has a stable event UUID but was never previously written to the
inbox. After v3 is durable, it is delivered through the same `IndexReplayMutationSink` implementation on
`PostgresMutationStore`.

The sink must report `StaleIgnored`. Durable target source version and payload remain v3 in both the
original and freshly composed query runtimes.

A final exact v3 redelivery must report `Duplicate` and likewise leave graph authority unchanged.

This distinguishes two important late-delivery classes:

- exact current replay after checkpoint loss -> inbox `Duplicate`;
- previously unseen lower source version -> monotonic `StaleIgnored`.

Neither can regress Product linked-target authority.

## Evidence boundary

The fail-once checkpoint adapter injects the failure boundary but does not claim to execute the durable
PostgreSQL replay-job/checkpoint lease store in this packet. That owner machinery already has separate
M6 source/evidence contracts. If maintainer execution reveals a checkpoint-store-specific integration
gap, retain that separately rather than adding Product-specific replay state.

This packet is source-ready and unexecuted.

## Maintainer verification

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-distribution --features mod-product \
  --test product_linked_target_replay_redelivery_postgres -- --nocapture
node scripts/verify/verify-index-linked-target-replay-redelivery-postgres-harness.mjs
node scripts/verify/verify-index-source-replay-contract.mjs
node scripts/verify/verify-index-link-target-availability.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
