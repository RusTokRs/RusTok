# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked against current `main@c1edda46e5c4bf5ad3e6dbb01581d6026da28a95` and continued on
`agent/index-m7-product-channel-relation-ledger-20260807`. The intervening #3126 Page Builder change
does not overlap the Index/Product relation slice.

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
- bounded fail-closed per-tenant persisted schema readiness gate.

### Owner-execution gates that remain open

- concrete repair PostgreSQL retained packet;
- persisted tenant schema readiness evidence;
- Product-locale absence/live drift evidence;
- live PostgreSQL/reference query equivalence;
- retained real PostgreSQL partition packet;
- multi-host/restart/graceful-shutdown evidence;
- freshness/outage/backlog/delete-recreate and tombstone-retention evidence.

## Event-contract blocker

The Product Index typed refresh family is intentionally blocked.

`crates/rustok-events/contracts/event-contract-digests.json` is stale relative to the already merged
Reactions family. PR #3122 added a maintainer-dispatched canonical digest admission workflow, but it
did not regenerate or admit the artifact. Product Index events must not be layered on top of stale
release digests.

Required order:

1. maintainer runs **Event contract digest admission** in `generate_patch` mode;
2. canonical generated digest changes are reviewed and committed in a separate PR;
3. `verify` mode is retained against the admitted commit;
4. only then add the Product Index typed family and regenerate the digests in that wire-contract PR;
5. register Product/ProductVariant production routes and concrete consumers after the wire contract
   is admitted.

The existing owner ledgers, canonical writer, and durable relay step do not bypass this gate.

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
- [ ] Admit the canonical event-contract digest baseline.
- [ ] Add reviewed Product Index typed event family.
- [ ] Register Product/ProductVariant routes and concrete consumers.
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
- [ ] Run owner verification/evidence for persisted M7 tenant schema readiness and relation storage.
- [ ] Compose the cross-owner Product visibility to Channel UUID resolver without adding a Channel
      dependency or Channel SQL to `rustok-product`; explicitly preserve unrestricted Product
      visibility rather than interpreting an empty slug allowlist as an empty UUID membership.
- [ ] Add a new Product schema version plus relation replay source and Product-to-SalesChannel
      `IndexLink`; do not mutate Product v2 in place.
- [ ] Admit the Product Index typed wire family, routes, and concrete consumers after digest repair.
- [ ] Retain Channel create/delete/slug-change, Product visibility change, retry, restart,
      delete/recreate, out-of-order, and locale fan-out evidence for the relation.
- [ ] Prove complete Product/Variant/Channel query parity and no source-module filtering fan-out.
- [ ] Move one Storefront query to Index only after all readiness/equivalence/freshness gates pass.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## New source slice — Product-owned SalesChannel relation ledger

The existing database-neutral relation admission contract already proved why Product and Channel
owner revisions cannot be combined into a safe relation source version. This slice gives that
relation its first durable owner state without violating module boundaries.

`product_sales_channel_index_relation_snapshots` stores one immutable complete resolved Channel UUID
set per relation epoch. For one exact tenant/Product identity:

- the first epoch is exactly `1`;
- later membership changes advance exactly by one under an advisory transaction lock;
- identical membership is an idempotent retry and does not append another epoch;
- membership is canonical, strictly sorted, unique, non-nil, and bounded to 1024 Channel UUIDs;
- writes first require the live Product row under `FOR KEY SHARE`, preventing stale post-delete
  resolver writes;
- retained snapshots cannot be updated or deleted;
- Product hard delete appends an empty epoch when the current membership is non-empty;
- `list_changes` exposes bounded tenant-scoped sequence pages;
- `scan_current` exposes bounded current Product-order pages;
- `load_current` exposes bounded exact Product loads.

The owner store intentionally knows no Channel schema, Channel table, Index mutation, broker, or
runtime worker. It receives only an already resolved UUID membership. This keeps `rustok-product`
installable without `rustok-channel` and makes cross-owner resolution a separate integration concern.

The resolver has one semantic trap that must remain explicit: Product currently treats an empty
`allowed_channel_slugs` list as unrestricted visibility, whereas an empty resolved UUID membership
means no relation targets. The next slice must define the reviewed unrestricted resolution policy and
must not copy the Product slug list mechanically.

## Next implementation step

Primary owner step: execute and admit the locked concrete-repair PostgreSQL packet.

Next unblocked source step after this ledger slice: compose a bounded cross-owner resolver in
`rustok-distribution` (or an equivalent selected-module integration layer). It should read current
Product visibility and current tenant Channel identities, preserve the unrestricted visibility
semantics, submit the complete resolved UUID set to the Product-owned relation store, and react to
both Product visibility and Channel identity changes.

After that resolver is source complete, add a new Product Index schema version and relation replay
adapter; Product v2 must remain immutable.

## Maintainer verification for this slice

```bash
cargo test -p rustok-product index_channel_relation --lib -- --nocapture
cargo test -p rustok-distribution product_sales_channel_relation -- --nocapture
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
