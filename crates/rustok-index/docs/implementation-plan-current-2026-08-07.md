# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked against `main@9c890c86931d56c6c94e86dadf66032d02fa27ef` and continued on
`agent/index-m7-product-v3-sales-channel-20260807`. The intervening Pages #3136 request-contract work
does not overlap this Product/Index source slice.

This file supersedes the dated 2026-08-03 status overlay. `implementation-plan.md` remains the
historical milestone/architecture plan, but several M5/M6/M7 checklist entries are stale relative to
merged work from August 3-7.

## Current primary cursor

`M6 - execute and admit concrete repair PostgreSQL evidence`

The concrete repair implementation, recovery policy, real-migration PostgreSQL harness, and retained
evidence admission tooling are source complete. The locked evidence packet still requires maintainer
execution against PostgreSQL and must not be claimed by source inspection alone.

Independent source work may continue while that owner-execution gate is pending.

## Recheck against current main

### Confirmed source-complete since the older canonical checklist

- mutation-source event registry and commit-before-ack worker contract;
- Social Graph exact production mutation route and concrete consumer policy;
- exact one-key source-refresh worker with minimum owner source-version fencing;
- bounded replay dry-run, cooperative page interruption, retry/dead-letter transitions, authorized
  requeue, reconciliation retry/dead-letter handling, and generic host scheduling;
- bounded drift candidate reading, confirmation, finding persistence/inspection/lifecycle, targeted
  repair reservations/receipts, concrete missing-entity and orphan-link repair, and prepared-command
  recovery;
- Product locale append-only Index refresh ledger and bounded owner source;
- ProductVariant parent-aware tombstones plus append-only refresh ledger/source;
- Product-owned exact refresh canonical writer;
- durable Product locale/ProductVariant relay cursor and one-step canonical outbox publication;
- bounded fail-closed per-tenant persisted schema readiness gate;
- Product-owned append-only Product-to-SalesChannel relation snapshot ledger with independent
  monotonic epoch, bounded owner reads, live-Product fencing, and retained empty membership;
- bounded cross-owner Product visibility to Channel UUID resolver in `rustok-distribution` with
  explicit unrestricted semantics and observe/write/re-observe stabilization;
- Product-owned graph-v3 projection epoch ledger that arbitrates Product revision and relation epoch
  without using either independent counter directly as the future full-record source version.

### Owner-execution gates that remain open

- concrete repair PostgreSQL retained packet;
- persisted tenant schema readiness evidence;
- Product-locale absence/live drift evidence;
- Product-to-SalesChannel relation/resolver/projection PostgreSQL concurrency and restart evidence;
- live PostgreSQL/reference query equivalence;
- retained real PostgreSQL partition packet;
- multi-host/restart/graceful-shutdown evidence;
- freshness/outage/backlog/delete-recreate and tombstone-retention evidence.

## Event-contract admission status

The Product Index typed refresh family remains intentionally blocked.

The committed digest artifact changed after #3130, so the older statement that the exact current file
was definitely still the pre-Reactions stale artifact is no longer current. However the canonical
generator/verify admission remains maintainer-execution work and this pass did not run it or inspect a
retained successful verify packet. Source inspection alone cannot prove that the current digest hashes
are canonical and admitted.

Required gate before Product Index typed events:

1. maintainer establishes canonical digest status for current reviewed `main` with the existing
   admission workflow or exact locked generator;
2. if drift exists, commit only canonical generator output through normal event-contract review;
3. retain a successful `verify` packet on the admitted commit;
4. then add the Product Index typed family and regenerate/admit the digest artifact in that reviewed
   wire-contract change;
5. only after the wire contract is admitted, register Product/ProductVariant/relation routes and
   concrete consumers.

The Product owner ledgers, relay, relation resolver, and projection epoch do not bypass this gate.

## M5 incremental ingestion

- [x] Source replay registry with bounded failure classification.
- [x] Inbox deduplication and monotonic source versions.
- [x] Database-neutral mutation-event registry and commit-before-ack orchestration.
- [x] Exact source-refresh worker with owner revision fence.
- [x] Social Graph production route and concrete consumer policy.
- [x] Product locale refresh ledger/source.
- [x] ProductVariant parent-aware refresh ledger/source.
- [x] Product exact canonical refresh writer.
- [x] Durable Product locale/ProductVariant relay cursor and bounded one-step relay.
- [ ] Retain canonical event-contract digest admission/verify for current main.
- [ ] Add reviewed Product Index typed event family.
- [ ] Register Product/ProductVariant/relation routes and concrete consumers.
- [ ] Retain crash-between-commit-and-ack/redelivery evidence for the Product route.

## M6 replay, reconciliation, diagnosis, and repair

- [x] Bounded scan/load and stable replay identities.
- [x] Durable jobs, leases, checkpoints, multi-page replay, cancellation, and reconciliation.
- [x] Source timeout, no-write dry-run, cooperative page interruption.
- [x] Replay/reconciliation retry, dead-letter, authorized recovery, and generic host scheduling.
- [x] Exact drift snapshot/digest, missing discovery, stale/orphan candidates, confirmation, finding
  persistence, inspection, lifecycle, targeted repair, and prepared-command recovery.
- [x] Concrete missing-entity and orphan-link owners.
- [x] Real-migration PostgreSQL repair harness and clean-commit retained-evidence admission tooling.
- [ ] Execute and admit the concrete repair PostgreSQL packet.
- [ ] Bind interruption to already-pending owner futures and active lease state.
- [ ] Retain multi-host/restart/graceful-shutdown/command-transport evidence.
- [ ] Add locale/partition replay checkpoint dimensions.
- [ ] Add explicit targeted/full/shadow rebuild modes.
- [ ] Add time-derived repair lease expiry only with retained liveness/crash evidence.

## M7 Product/ProductVariant/SalesChannel production graph

- [x] Product, ProductVariant, and SalesChannel schemas and bounded sources.
- [x] Stable replay identities and retained delete semantics.
- [x] Product-to-ProductVariant v2 graph materialization.
- [x] Product locale and ProductVariant owner refresh publication.
- [x] Generic source-refresh worker.
- [x] Product canonical refresh writer and durable relay step.
- [x] Bounded fail-closed per-tenant persisted schema readiness gate for an explicit exact schema set.
- [x] Product-owned append-only Product-to-SalesChannel relation snapshot ledger with a dedicated
      monotonic epoch, live-Product delete fencing, bounded owner readers, idempotent replacement, and
      retained empty membership on Product hard delete.
- [x] Bounded cross-owner Product visibility to Channel UUID resolver with explicit unrestricted
      mapping, current Channel identity resolution, 64-Product pages, and three-attempt stabilization.
- [x] Product-owned graph-v3 projection epoch that advances when either Product graph revision or
      resolved relation epoch advances and retains both input watermarks.
- [ ] Run owner verification/evidence for persisted M7 tenant schema readiness, relation storage,
      resolver convergence, and projection-epoch concurrency/delete ordering.
- [ ] Add Product v3 plus Product-to-SalesChannel `IndexLink` using the dedicated projection epoch as
      the full-record source version; do not mutate Product v2 in place.
- [ ] Add Product v3 absence semantics and exact replay checks that fail closed when projection inputs
      are not current.
- [ ] Add durable Product-visibility and Channel-identity convergence triggering or an admitted
      relation freshness watermark/checkpoint; the bounded resolver alone is not continuous.
- [ ] Admit the Product Index typed wire family, routes, and concrete consumers after digest
      verification/admission.
- [ ] Retain Channel create/delete/slug/identity-change, Product visibility change, retry, restart,
      delete/recreate, out-of-order, and locale fan-out evidence for the relation.
- [ ] Prove complete Product/Variant/Channel query parity and no source-module filtering fan-out.
- [ ] Move one Storefront query to Index only after all readiness/equivalence/freshness gates pass.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## New source slice — Product v3 projection epoch prerequisite

The previous overlay said the next Product v3 source should use the Product-to-SalesChannel
`relation_epoch` as its relation source version. Rechecking the actual Index mutation store exposed a
necessary correction: Index stores one source version for the **whole** entity record and ignores an
incoming mutation when its version is not greater than the current materialized version.

A Product v3 record will combine two independently versioned input families:

- Product scalars, translations, and ProductVariant membership under `products.index_revision`;
- resolved SalesChannel membership under `relation_epoch`.

Using either counter directly would permit a change from the other family to be stale-ignored. Using
`max`, hashes, timestamps, or pair encodings is also unsafe and was already rejected by the relation
admission contract.

`product_index_graph_v3_projection_snapshots` therefore owns a third counter:
`projection_epoch`.

For one exact tenant/Product identity:

- epoch 1 is created only when both a Product/tombstone source version and relation epoch exist;
- later projection epochs advance exactly by one;
- neither retained input watermark may regress;
- an unchanged input pair is idempotent and does not append;
- the canonical reconciliation function merges concurrent retained component watermarks and advances
  the independent projection epoch when either input moved;
- direct inserts are guarded under the same advisory lock and contiguous-epoch contract;
- snapshots are append-only;
- Product insert, Product `index_revision` changes, relation snapshot inserts, and Product delete all
  invoke reconciliation;
- Product hard-delete trigger naming deliberately runs projection reconciliation after the existing
  retained-empty-relation trigger, so committed projection state uses the final empty membership;
- migration backfill creates epoch 1 for existing live/retained Product identities that already have
  relation state.

`GREATEST(product_source_version, previous_product_source_version)` and the analogous relation call are
only retained-watermark merges. They are explicitly **not** used as the Index source-version
encoding. The future Product v3 source version is the independent `projection_epoch`.

Detailed contract:
`../../rustok-product/docs/index-graph-v3-projection-ledger.md`.

## Remaining freshness boundary

Projection monotonicity is necessary but not sufficient for production correctness. A Product update
can advance `products.index_revision` before the cross-owner resolver has recomputed relation
membership for changed Product visibility metadata. The projection epoch can therefore prove mutation
ordering without proving that relation membership is already fresh.

The future Product v3 source may be implemented against this ledger, but authoritative replay/cutover
must remain blocked until durable Product-visibility/Channel-identity convergence triggering or an
admitted freshness watermark exists and retained PostgreSQL evidence proves the behavior.

## Next implementation step

Primary owner step: execute and admit the locked concrete-repair PostgreSQL packet.

Next unblocked source step after this prerequisite: publish Product v3 on the existing stable
`product-postgres-primary` source identity, use `projection_epoch` as its full-record source version,
materialize both the existing ProductVariant link and the new SalesChannel link, and add projection-
aware Product v3 absence semantics. Product v1/v2 must remain unchanged.

In parallel, durable relation convergence/freshness triggering remains required before Product v3 can
be treated as authoritative, and event-contract digest admission remains required before any new typed
Product event route is claimed.

## Maintainer verification for this slice

```bash
node scripts/verify/verify-index-product-v3-projection-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
