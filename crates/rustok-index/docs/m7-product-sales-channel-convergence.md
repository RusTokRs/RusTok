# M7 Product-to-SalesChannel automatic convergence

Status: `source_and_query_fence_complete_runtime_evidence_pending`.

## Selected boundary

The canonical Product Index relation has separate durable facts for membership, complete Product
mutation ordering, freshness, automatic owner-change convergence, and materialized query admission.
This separation prevents a freshness-only observation from fabricating relation membership or a new
Product schema.

Product owns:

- append-only visibility-change requests;
- tenant visibility cursor;
- Channel-generation checkpoint as an opaque numeric watermark;
- in-progress tenant sweep generation and Product keyset cursor;
- lease, retry availability, attempt count, and bounded last-error marker.

Channel continues to own `channel_index_identity_generations`. Distribution is the only layer that reads
both owners and invokes the existing exact resolver.

## Generic ModuleWork composition

The selected distribution registers `product_sales_channel_relation_convergence` through the generic
`ModuleWorkRegistrations` contract only when both Product and Channel runtime markers are present.
No Product or Channel module owns a background task and no host-side switch understands their storage.

One claim cycle is bounded:

- discover at most one due tenant;
- claim exactly one tenant lease under Product-owned `FOR UPDATE` state;
- process either one exact Product visibility request or one 64-Product convergence page;
- call the existing exact resolver for each Product in that bounded page;
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

Rejected Product owner data is isolated rather than allowed to poison the ordered queue. Invalid
visibility, an oversized allowlist, or too many resolved Channel targets complete the exact request
without a freshness witness. That Product remains fail-closed in canonical source/query admission. A
later Product correction appends another exact request; a Channel-side correction advances Channel
identity generation and starts another tenant sweep.

## Channel identity changes

The work source compares current Channel generation with the Product-owned completed checkpoint. A
missing checkpoint forces a baseline sweep. A newer Channel generation starts a bounded tenant sweep
with a fixed generation and Product UUID keyset cursor.

A rejected Product does not head-of-line block later valid Products in the same tenant. The convergence
page leaves that Product stale, continues the remaining exact resolver calls, and checkpoints the
bounded page. Retryable concurrency/storage/relation/freshness errors still stop the page and preserve
its durable cursor for retry.

A Channel change during the pass cannot be lost. The old pass may complete, but it checkpoints only the
generation it started with; the newer current generation remains ahead and schedules another pass.

## Multi-host and restart behavior

Due discovery is intentionally read-only and may race across hosts. The Product-owned state row is the
authority: `FOR UPDATE` claim plus lease token allows only one durable claimant. An expired lease is
reclaimable; exact request cursor and tenant sweep cursor survive restart.

The DDL state guard mirrors the service state machine and prevents direct SQL from skipping requests,
forging a completed Channel checkpoint, changing an in-progress sweep generation, resetting a partial
sweep cursor, or deleting state. Partial cursors advance strictly by Product UUID; terminal completion
clears the cursor only while checkpointing the exact sweep generation.

## Admission boundary

Automatic relation convergence is now source complete. It closes the previous requirement that an
external caller manually invoke Product relation reconciliation after Product visibility or Channel
identity changes, while isolating rejected Product owner data from valid tenant progress.

The separate materialized/query freshness fence is also source complete. Product root queries compare
materialized `projection_epoch` with current Product projection/revision, live locale identity, relation
freshness, current Channel generation, and the visibility-request watermark before user
filter/order/cursor/limit/exact-count semantics. Therefore an already-produced stale mutation can land
in Index storage without becoming query-authoritative.

Source completeness still does not authorize Storefront cutover. PostgreSQL execution evidence for the
in-flight stale-mutation window and for this convergence state machine remains pending.

## Retained PostgreSQL convergence packet

`crates/rustok-distribution/tests/product_channel_convergence_postgres.rs` is now a source-ready,
execution-pending packet for this state machine. It uses two independent generic `ModuleWorkScheduler`
hosts and the production Product/Channel/Index storage/runtime path.

The packet retains assertions for:

- live-lease exclusion and reclaim after lease expiry;
- restart continuation without resetting the Product visibility cursor;
- malformed Product isolation while later valid Products still receive current freshness;
- Product visibility `alpha -> beta` after source read, including physical stale Index materialization,
  relation/projection advancement, query exclusion, and corrective current mutation;
- Channel generation change with unchanged unrestricted UUID membership, where only freshness advances
  and the same materialized Product becomes admissible again;
- Channel generation change with changed restricted membership, where relation/projection advance and
  the old Index row stays inadmissible until the current Product mutation is applied.

Detailed packet contract:
[M7 Product visibility / Channel identity convergence PostgreSQL harness](./m7-product-channel-convergence-postgres-harness.md).

This is retained source only. It has not been executed or admitted.

## Remaining M7 admission

- execute/admit the source-ready Product delayed-mutation/locale-deletion PostgreSQL query-freshness
  packet;
- execute/admit the source-ready Product visibility + Channel-generation convergence packet;
- retain any still-missing Channel create/delete/tenant-move and delete-recreate evidence not covered by
  the slug-generation packet;
- admit canonical Product typed events/routes only after event-contract digest verification;
- prove complete Product/Variant/Channel query parity, including linked-target availability;
- move Storefront traffic only after readiness, equivalence, convergence, and materialized-freshness
  evidence pass.

## Maintainer verification

```bash
cargo test -p rustok-distribution --features mod-product --test product_materialized_query_freshness_postgres -- --nocapture
cargo test -p rustok-distribution --features mod-product --test product_channel_convergence_postgres -- --nocapture
node scripts/verify/verify-index-product-materialized-query-freshness-postgres-harness.mjs
node scripts/verify/verify-index-product-channel-convergence-postgres-harness.mjs
node scripts/verify/verify-index-product-materialized-query-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-convergence.mjs
node scripts/verify/verify-index-product-channel-relation-freshness.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --features mod-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, CI, or
`git diff --check` were executed by the implementation agent.
