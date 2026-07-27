# Implementation plan for `rustok-events`

## Source of truth

This file is the live plan for shared event contracts and guarantees remote consumers
may rely on. Transport execution remains owned by Outbox, Iggy, the server runtime,
or the consuming owner module.

Last reconciled with `main`: 2026-07-27.

## Current state

`rustok-events` is the canonical source of `DomainEvent`, `EventEnvelope`, sealed typed
families, schema metadata, semantic validation, and versioning policy.
`rustok-core::events` is a compatibility re-export only.

The committed `contracts/event-contract-digests.json` gates the root registry and
root/typed wire schemas. Digest generation is deterministic and never occurs as an
implicit build/test side effect. The current release train intentionally allows only
version-1 schemas until remote-consumer migration ownership is retained.

Root and typed envelopes validate at publication, outbox, relay, and JSON/MessagePack
decode boundaries. `EventRuntime` is published before dispatcher startup and owns the
shared Iggy connector used by outbound relay and approved inbound consumers.

The typed-family implementation includes sealed
`social_graph.relation.state_changed` v1. Its payload contains relation id,
source/target user ids, canonical relation kind, active state, and revision only.
Tenant and actor remain envelope metadata. The Social Graph owner publishes the fact
transactionally and provides bounded service/system-only replay over authoritative
relation state.

## Delivered Social Graph → Index consumer contract

- Index is the first approved consumer of the sealed relation family.
- Active relations map to generic non-localized upserts, inactive relations to
  tombstones, relation id to entity identity, and revision to monotonic
  `source_version`.
- `SocialGraphIndexProjector` persists or exactly recognizes the tenant schema through
  Index-owned `PostgresSchemaRegistrationStore` before `PostgresMutationStore` apply.
- `Applied`, `Duplicate`, and `StaleIgnored` are durable terminal outcomes.
- Persistent group `rustok-social-graph-index` consumes the shared `domain` topic;
  unrelated sealed families are acknowledged without projection.
- Staged receive/project/ack retains one outstanding delivery across bounded retries.
- Runtime execution is default-off and requires explicit enablement, a worker host,
  and effective `outbox_iggy` delivery.
- Relay and consumer reuse the exact `Arc<IggyTransport>` created by `EventRuntime`.
- Shared `StopHandle` controls graceful shutdown and the worker handle participates in
  readiness only while explicitly enabled.
- Projection failures use bounded retry. Permanent/exhausted failures may publish exact
  original bytes to DLQ only before a durable Index result.
- DLQ publication and source acknowledgement are staged. Once Index or DLQ has a
  terminal result, the worker retries acknowledgement only.
- Successfully decoded deliveries retain exact raw bytes. Undecodable bytes remain
  unacknowledged pending a connector-level poison-message contract.
- Missing/stopped/invalid enabled worker state reaches `runtime_guardrails`,
  `/health/ready`, and aggregate guardrail metrics. Disabled execution is healthy.
- Shared bounded Prometheus delivery metrics cover received/terminal outcomes, retries,
  stage/error failures, DLQ publication, receive-to-ack duration, worker lifecycle,
  in-flight state/timestamp, and last success.
- A read-only Iggy observer connects to the already-running configured endpoint and
  reads every `domain` partition plus the persistent group checkpoint. It does not
  consume events, store offsets, publish, acknowledge, or manage a broker process.
- Position metrics expose snapshot timestamp, partition count, completeness, and exact
  total/max broker offset lag only when every partition has a coherent result. Empty
  partitions contribute zero; missing/inconsistent checkpoints make the snapshot
  incomplete and clear lag gauges.
- Metric labels are bounded and exclude tenants, event/relation identifiers, partition,
  offset, payloads, ack tokens, credentials, and raw error messages.
- Observer failures are retried independently and do not stop projection or enter the
  projection worker readiness contract.
- Bounded Social Graph replay uses the same schema/inbox/source-version path for repair.
- Profiles privacy remains on synchronous authoritative Social Graph ports.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core -> transport -> ui/leptos`, with a sibling module-owned Next
  package.
- `rustok-events-module` remains the cycle-free runtime/manifest adapter and owns its
  admin delivery-profile surface.
- The host composes routes and delivery control; it does not own event schemas or
  module UI.

## Completed source results

- [x] Keep one canonical event/envelope/schema definition in `rustok-events`.
- [x] Validate root and typed payloads at publication, relay, and decode boundaries.
- [x] Keep registry, wire schemas, and committed digest artifact synchronized.
- [x] Own and guard outbound relay lifecycle.
- [x] Add sealed `social_graph.relation.state_changed` v1 and transactional publication.
- [x] Add bounded authoritative Social Graph replay.
- [x] Add generic Index conversion and Index-owned tenant schema registration.
- [x] Add persistent result-first Index consumption with duplicate/stale recognition.
- [x] Add default-off host startup, strict delivery gating, one shared Iggy connector,
  shutdown, bounded retry, staged exact-byte DLQ-before-ack, and acknowledgement-only
  recovery.
- [x] Add enabled-worker readiness and aggregate guardrail metrics.
- [x] Add bounded dedicated remote-consumer delivery telemetry.
- [x] Add read-only every-partition committed/high-watermark observation and
  completeness-gated total/max lag.
- [x] Add source guards for ordering, connector ownership, readiness, telemetry labels,
  broker-backed lag origin, incomplete-snapshot clearing, and foreign-table isolation.

## Open results

1. **Keep reviewed event-contract digests synchronized.** Regenerate only through the
   deterministic example when a reviewed contract shape changes.
2. **Keep event types, registry, release artifacts, and consumer imports synchronized.**
   New families require direct `rustok-events` imports, semantic coverage, and owner
   recovery guidance.
3. **Prove remote cursor and position recovery.** Execute real-Iggy restart, redelivery,
   ack failure, DLQ failure, connector loss, observer reconnect, shutdown, TLS/auth,
   rebalance, concurrent snapshot movement, and multi-replica ownership scenarios.
4. **Prove Index repair and concurrency.** Execute PostgreSQL concurrent schema
   registration/mutation, create drift, and repair through bounded owner replay/rescan
   while privacy remains on owner ports.
5. **Close the DLQ acknowledgement window.** Decide whether publish-success/source-ack
   failure needs a durable owner receipt or another idempotent DLQ identity. Define a
   connector poison shape for undecodable deliveries.
6. **Synchronize recovery guidance.** Outbox, replay, reindex, lag, and DLQ runbooks must
   name exact schemas and avoid transport-owned payload copies.

## Verification

- `cargo xtask module validate events`
- `cargo xtask module test events`
- `cargo test -p rustok-events --test social_graph_contracts -- --nocapture`
- `cargo run -p rustok-events --example event_contract_digests`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets`
- `cargo test -p rustok-telemetry`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-index --all-targets`
- `cargo test -p rustok-index schema_registration --lib -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index-consumer --all-targets`
- `cargo test -p rustok-social-graph --features index-consumer index_consumer::tests -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets`
- `cargo test -p rustok-server social_graph_index_worker --lib -- --nocapture`
- `cargo test -p rustok-server runtime_guardrails --lib -- --nocapture`
- `node scripts/verify/verify-index-schema-registration.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-relation-event-replay.mjs`
- `node scripts/verify/verify-social-graph-index-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-runtime-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `node scripts/verify/verify-runtime-consumer-metrics.mjs`
- Real-broker multi-replica restart/recovery/position and PostgreSQL evidence.

These commands and scenarios remain maintainer-run and were not executed manually in
this slice.

## Change rules

1. Keep canonical event payloads and schemas in this module.
2. Keep transport execution in its runtime owner; do not copy payload definitions.
3. Update digest artifacts only through intentional contract review.
4. Consumers persist or recognize tenant schema and owner result before ack.
5. Source consumers never write another owner's schema/projection tables directly.
6. Reuse the host-owned connector; do not create another bundled transport in a worker.
7. Permitted DLQ publication precedes source ack; terminal-result ack failure is
   acknowledgement-only recovery.
8. Enabled durable workers participate in readiness and bounded telemetry; disabled
   optional workers do not degrade the host.
9. Publish lag only from every-partition broker checkpoints/high-watermarks with an
   explicit completeness signal; never infer it from event age or one offset.
10. Position observation is read-only and cannot become event execution or owner policy.
11. Keep producer storage authoritative for bounded repair.
12. Update module docs, event flow, and recovery guidance with every contract change.
13. Keep the central plan registry limited to status and nearest priority.
