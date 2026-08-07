# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked against branch base `main@375f7cdb80244570bb5e93405b771e0e8516b1f4` and continued on
`agent/index-m7-product-channel-resolver-20260807`. Main advanced substantially after #3130 in Events,
Pages, Reactions, Commerce, module-control-plane, and adjacent surfaces; those changes do not replace
the Product-to-SalesChannel relation resolver work described here.

This file supersedes the dated 2026-08-03 status overlay. `implementation-plan.md` remains the
historical milestone/architecture plan, but several of its M5/M6/M7 checklist entries are stale
relative to merged work from August 3-7.

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
  explicit unrestricted semantics and observe/write/re-observe stabilization.

### Owner-execution gates that remain open

- concrete repair PostgreSQL retained packet;
- persisted tenant schema readiness evidence;
- Product-locale absence/live drift evidence;
- Product-to-SalesChannel owner/resolver PostgreSQL concurrency and restart evidence;
- live PostgreSQL/reference query equivalence;
- retained real PostgreSQL partition packet;
- multi-host/restart/graceful-shutdown evidence;
- freshness/outage/backlog/delete-recreate and tombstone-retention evidence.

## Event-contract admission status

The Product Index typed refresh family remains intentionally blocked, but the reason must now be
stated more precisely than in the previous overlay.

The current digest artifact changed after #3130. Therefore the older source statement that the exact
committed digest file was definitely still the pre-Reactions stale artifact is no longer current.
However `crates/rustok-events/docs/event-contract-digest-admission.md` still marks canonical
maintainer execution pending, and this implementation pass did not run the generator or inspect a
retained `verify` admission packet. Source inspection alone cannot prove that the current digest hashes
are canonical and admitted.

Required gate before Product Index typed events:

1. maintainer establishes the canonical digest status for the current reviewed `main` using the
   existing Event contract digest admission workflow or the exact locked generator;
2. if drift exists, review and commit only canonical generator output through the normal event-contract
   review path;
3. retain a successful `verify` packet on the admitted commit;
4. then add the Product Index typed family and regenerate/admit the digest artifact in that reviewed
   wire-contract change;
5. only after the wire contract is admitted, register Product/ProductVariant/relation routes and
   concrete consumers.

The Product owner ledgers, canonical writer, durable relay, relation owner storage, and cross-owner
resolver do not bypass this gate.

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
- [ ] Run owner verification/evidence for persisted M7 tenant schema readiness, relation storage, and
      resolver concurrency/restart behavior.
- [ ] Add Product v3 plus relation replay source and Product-to-SalesChannel `IndexLink`; do not
      mutate Product v2 in place.
- [ ] Add durable Product-visibility and Channel-identity convergence triggering or an admitted
      relation watermark/checkpoint; the source resolver alone is not a continuous consumer.
- [ ] Admit the Product Index typed wire family, routes, and concrete consumers after digest
      verification/admission.
- [ ] Retain Channel create/delete/slug/identity-change, Product visibility change, retry, restart,
      delete/recreate, out-of-order, and locale fan-out evidence for the relation.
- [ ] Prove complete Product/Variant/Channel query parity and no source-module filtering fan-out.
- [ ] Move one Storefront query to Index only after all readiness/equivalence/freshness gates pass.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## New source slice — cross-owner Product/SalesChannel resolver

`rustok-distribution::product_index::channel_relation_resolver` now composes the two owner views without
moving Channel knowledge into Product.

For one exact Product it:

- reads `products.metadata.channel_visibility.allowed_channel_slugs` under one PostgreSQL
  `REPEATABLE READ`, `READ ONLY` observation;
- treats missing visibility or an empty allowlist as unrestricted;
- resolves unrestricted visibility to all current tenant Channel identities;
- resolves restricted visibility by canonical `lower(btrim(channels.slug))` membership;
- does not filter `channels.is_active`, because runtime availability remains Channel-authority state
  rather than Product-to-Channel identity membership;
- bounds visibility input and resolved UUID membership at 1024 entries;
- calls the Product-owned relation writer with only the complete UUID set;
- re-observes the cross-owner inputs and succeeds only if the resolved membership is stable;
- retries at most three times before returning `ConcurrentChange`.

For initial backfill and Channel-side changes, `reconcile_tenant_page` enumerates at most 64 Product
UUIDs with keyset ordering and reconciles each independently. A partial page can be retried from the
same input cursor because owner replacement is idempotent. The page does not claim a global snapshot;
Products created behind an already consumed cursor still require Product events or a later sweep.

This source slice deliberately does not register a host loop, event route, broker consumer, relation
watermark, or Index schema. It is a bounded convergence primitive whose durable triggering remains
open.

Detailed contract: `m7-product-sales-channel-resolver.md`.

## Next implementation step

Primary owner step: execute and admit the locked concrete-repair PostgreSQL packet.

Next unblocked source step after this resolver slice: add Product v3 and a relation replay adapter that
uses `ProductSalesChannelIndexRelationStore` epochs as the relation source version, fans each current
relation snapshot out to exact current Product locales, and materializes the SalesChannel UUID link.
Product v2 must remain immutable.

In parallel, the maintainer must establish current event-contract digest admission before any new
Product typed event family or durable incremental route is claimed.

## Maintainer verification for this slice

```bash
cargo test -p rustok-product index_channel_relation --lib -- --nocapture
cargo test -p rustok-distribution product_sales_channel -- --nocapture
node scripts/verify/verify-index-product-channel-relation-resolver.mjs
node scripts/verify/verify-index-product-channel-relation-ledger.mjs
node scripts/verify/verify-index-product-channel-relation-admission.mjs
node scripts/verify/verify-index-schema-readiness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-product --all-targets
cargo check -p rustok-distribution --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
