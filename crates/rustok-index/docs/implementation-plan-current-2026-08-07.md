# Current `rustok-index` implementation plan — 2026-08-07

Status overlay rechecked against `main@64972ec50d3c77dd54aafa50b0fcfd2c79eabc96` and continued on
`agent/index-m7-schema-readiness-20260807`.

This file supersedes the dated 2026-08-03 status overlay. `implementation-plan.md` remains the
historical milestone/architecture plan, but several of its M5/M6/M7 checklist entries are stale
relative to merged work from August 3-6.

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
- durable Product locale/ProductVariant relay cursor and one-step canonical outbox publication.

### Owner-execution gates that remain open

- concrete repair PostgreSQL retained packet;
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
- [x] Add a bounded fail-closed per-tenant persisted schema readiness gate for an explicit exact
      schema set.
- [ ] Run owner verification/evidence for persisted M7 tenant schema readiness.
- [ ] Admit the Product Index typed wire family, routes, and concrete consumers after digest repair.
- [ ] Complete durable Product-to-SalesChannel owner relation epoch semantics and retained evidence.
- [ ] Prove complete Product/Variant/Channel query parity and no source-module filtering fan-out.
- [ ] Move one Storefront query to Index only after all readiness/equivalence/freshness gates pass.
- [ ] Keep authoritative consumer and production partition cutover forbidden until admission.

## New source slice — tenant schema readiness

`PostgresIndexSchemaReadinessStore` closes the source-level readiness gap without inventing a stale
secondary readiness flag.

For one explicit tenant and at most 64 exact schema references it:

- requires every reference to exist in the immutable runtime `SchemaRegistry` before storage access;
- reads the complete requested tenant set in one bounded statement;
- requires one persisted row per exact reference;
- requires `status = active`;
- requires the persisted fingerprint to equal the runtime fingerprint;
- requires persisted `schema_json` to equal the runtime contract;
- returns one deterministic receipt only when the complete set is ready;
- reports typed missing/inactive/fingerprint/contract failures and never returns partial success.

It is generic Index infrastructure. It performs no schema registration, Product-domain call, task
startup, retry, broker work, or cutover by itself.

## Next implementation step

Primary owner step: execute and admit the locked concrete-repair PostgreSQL packet.

Next unblocked source step after this readiness slice: define the durable Product-to-SalesChannel
owner relation epoch/storage semantics from the already admitted generic relation contract, unless the
maintainer first admits the event-contract digest baseline and unblocks Product Index typed events.

## Maintainer verification for this slice

```bash
cargo test -p rustok-index schema_readiness --lib -- --nocapture
node scripts/verify/verify-index-schema-readiness.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, Cargo checks, formatting, migrations, PostgreSQL scenarios, workflows, or CI
were executed by the implementation agent.
