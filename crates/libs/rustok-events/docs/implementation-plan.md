# Implementation plan for `rustok-events`

## Source of truth

This file is the live plan for shared event contracts and guarantees remote consumers
may rely on. Transport execution remains owned by Outbox, Iggy, the server runtime,
or the consuming owner module.

Last reconciled with `main`: 2026-07-28.

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
shared Iggy transport used by outbound relay and approved inbound consumers.
The root envelope uses the nil tenant UUID only for the explicit allow-list of
platform-capable module events; every other root and typed contract envelope
rejects the sentinel before persistence or relay.

The dynamic artifact lifecycle publishes `module.artifact.activated` with the
new installation identity, optional direct-predecessor identity, and positive
owner revision. Actor, reason, idempotency fingerprint, and the operation
receipt remain in the lifecycle owner rather than the shared event payload.

The typed-family implementation includes sealed
`social_graph.relation.state_changed` v1. Its payload contains relation id,
source/target user ids, canonical relation kind, active state, and revision only.
Tenant and actor remain envelope metadata. The Social Graph owner publishes the fact
transactionally and provides bounded service/system-only replay over authoritative
relation state.

The typed-family implementation also includes
`TranslationWorkflowEvent` v1 for job creation/cancellation/completion,
assignment, explicit blocked-item retry, proposal, apply, and recovery
lifecycle evidence. Translation publishes these contracts transactionally with
its workflow state. Payloads exclude source/target copy, proposal values,
operator reasons, claims, roles, and owner receipt data.

Contract tests cover public event-contract use cases.

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
- Projection, DLQ publication, and source acknowledgement use bounded retry.
- Migration `m20260727_000004_create_index_dlq_receipts` binds a poison decision to
  tenant/consumer-group/event identity, exact source coordinates, exact broker bytes,
  stable error code, and projection attempts.
- Receipt states durably reserve and lease publication, then record `published` before
  source ack. The consumer checks them before projection.
- A `published`/`acknowledged` redelivery skips Index projection and DLQ publication and
  enters acknowledgement-only recovery. An unfinished receipt remains retryable DLQ
  work and cannot cross back into mutation apply.
- A versioned length-framed SHA-256 construction derives one RFC 9562 UUIDv8 from the
  immutable receipt identity and exact payload. Retry count, time, publisher identity,
  and random state are excluded.
- `publish_consumed_to_dlq` attaches that UUID separately from the source event ID,
  publishes exact bytes, and returns success only after the durable `published`
  transition. The worker records fresh and previously published outcomes separately.
- `IggyTransport` lazily opens an SDK publisher connection to the same configured
  endpoint and existing `dlq` topic, then maps the UUIDv8 to Iggy's `u128` message
  header. It creates no second transport or broker process.
- Ack failure after successful publication is replay-safe without DLQ republish.
  Receipt acknowledgement is best-effort bookkeeping after the source broker commit.
- Broker success followed by process/DB failure before the `published` transition is
  still an explicit confirmation ambiguity. Republication carries the same header ID,
  but physical suppression occurs only while deployment-owned Iggy deduplication is
  enabled and its per-partition cache/expiry contains that ID.
- A deterministic ID and bounded optional deduplication do not create an event-contract
  exactly-once guarantee. The durable owner receipt remains authoritative.
- Successfully decoded deliveries retain exact raw bytes. Undecodable bytes remain
  unacknowledged pending a connector-level poison-message contract.
- Missing/stopped/invalid enabled worker state reaches `runtime_guardrails`,
  `/health/ready`, and aggregate guardrail metrics. Disabled execution is healthy.
- Shared bounded Prometheus delivery metrics cover received/terminal outcomes,
  projection/DLQ/ack retries, stage/error failures, DLQ publish classifications,
  receive-to-ack duration, worker lifecycle, in-flight state/timestamp, and last
  success.
- A read-only Iggy observer connects to the already-running configured endpoint and
  reads every `domain` partition plus the persistent group checkpoint. It does not
  consume events, store offsets, publish, acknowledge, or manage a broker process.
- Position metrics expose snapshot timestamp, partition count, completeness, and exact
  total/max broker offset lag only when every partition has a coherent result. Empty
  partitions contribute zero; missing/inconsistent checkpoints make the snapshot
  incomplete and clear lag gauges.
- Metric labels are bounded and exclude tenants, event/relation identifiers, partition,
  offset, payloads, broker IDs, ack tokens, credentials, and raw error messages.
- Observer failures are retried independently and do not stop projection or enter the
  projection worker readiness contract.
- Bounded Social Graph replay uses the same schema/inbox/source-version path for repair.
- Profiles privacy remains on synchronous authoritative Social Graph ports and must not
  authorize from Index, DLQ receipts, broker IDs, deduplication state, or lag.

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
- [x] Restrict the root nil-tenant platform sentinel to an explicit module-event
  allow-list; all other root and typed envelopes reject it. The release artifact
  was reverified by `canonical_contracts` (14 passing tests) on 2026-08-06.
- [x] Own and guard outbound relay lifecycle.
- [x] Add sealed `social_graph.relation.state_changed` v1 and transactional publication.
- [x] Add bounded authoritative Social Graph replay.
- [x] Add generic Index conversion and Index-owned tenant schema registration.
- [x] Add persistent result-first Index consumption with duplicate/stale recognition.
- [x] Add default-off host startup, strict delivery gating, one shared Iggy transport,
  shutdown, bounded projection/DLQ/ack retry, durable exact-byte DLQ receipts,
  deterministic UUIDv8 broker headers, and acknowledgement-only recovery.
- [x] Add enabled-worker readiness and aggregate guardrail metrics.
- [x] Add bounded dedicated remote-consumer delivery telemetry.
- [x] Add read-only every-partition committed/high-watermark observation and
  completeness-gated total/max lag.
- [x] Add source guards for ordering, transport ownership, readiness, telemetry labels,
  broker-backed lag origin, incomplete-snapshot clearing, durable DLQ identity/state,
  deterministic header construction, explicit Iggy `u128` publication, and
  foreign-table isolation.

## Open results

1. **Keep reviewed event-contract digests synchronized.** Regenerate only through the
   deterministic example when a reviewed contract shape changes.
2. **Keep event types, registry, release artifacts, and consumer imports synchronized.**
   New families require direct `rustok-events` imports, semantic coverage, and owner
   recovery guidance.
3. **Prove remote cursor, receipt, header, and position recovery.** Execute real-Iggy
   restart, redelivery, ack failure, DLQ failure, publisher reconnect, connector loss,
   observer reconnect, shutdown, TLS/auth, rebalance, concurrent snapshot movement,
   and multi-replica ownership.
4. **Exercise the remaining DLQ confirmation ambiguity.** Fail after broker publication
   but before the durable `published` mark with deduplication disabled, enabled,
   capacity-evicted, and expired. Verify the configured window covers the maximum
   lease/restart/recovery horizon before relying on suppression.
5. **Choose the production confirmation mechanism.** Enforce and monitor an adequate
   Iggy deduplication contract, or adopt a broker transaction/DB-owned DLQ outbox relay
   before claiming stronger physical duplicate guarantees.
6. **Prove Index repair and concurrency.** Execute PostgreSQL concurrent schema
   registration/mutation/DLQ receipt claims, create drift, and repair through bounded
   owner replay/rescan while privacy remains on owner ports.
7. **Define undecodable poison handling.** Add a connector shape that preserves exact
   raw bytes and source coordinates before envelope construction without moving owner
   policy into the transport.
8. **Synchronize recovery guidance.** Outbox, replay, reindex, lag, dedup configuration,
   receipt retention, and DLQ runbooks must name exact schemas and avoid
   transport-owned payload copies.

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
- `cargo test -p rustok-social-graph --features index-consumer index_dlq_receipt::tests -- --nocapture`
- `cargo test -p rustok-social-graph --features index-consumer index_dlq_message_id::tests -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets`
- `cargo test -p rustok-server social_graph_index_worker --lib -- --nocapture`
- `cargo test -p rustok-server runtime_guardrails --lib -- --nocapture`
- `node scripts/verify/verify-index-schema-registration.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-relation-event-replay.mjs`
- `node scripts/verify/verify-social-graph-index-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-runtime-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `node scripts/verify/verify-social-graph-index-dlq-receipts.mjs`
- `node scripts/verify/verify-runtime-consumer-metrics.mjs`
- Real-broker deterministic-header/dedup disabled-enabled-expiry-capacity,
  multi-replica restart/recovery/position/receipt, and PostgreSQL evidence.

These commands and scenarios remain maintainer-run and were not executed manually in
this slice.

## Change rules

1. Keep canonical event payloads and schemas in this module.
2. Keep transport execution in its runtime owner; do not copy payload definitions.
3. Update digest artifacts only through intentional contract review.
4. Consumers persist or recognize tenant schema and owner result before ack.
5. Source consumers never write another owner's schema/projection tables directly.
6. Reuse the host-owned transport; additional SDK clients may connect only to its
   configured endpoint and must never create another bundled process.
7. Permitted DLQ publication and durable terminal receipt precede source ack;
   terminal-result ack failure is acknowledgement-only recovery.
8. A durable receipt is checked before projection and binds exact source coordinates and
   bytes. A deterministic broker ID must bind the same immutable identity and exclude
   attempt/time/random state.
9. Do not claim physical broker exactly-once from a receipt, message ID, or optional
   bounded deduplication cache without retained configuration/runtime evidence.
10. Enabled durable workers participate in readiness and bounded telemetry; disabled
    optional workers do not degrade the host.
11. Publish lag only from every-partition broker checkpoints/high-watermarks with an
    explicit completeness signal; never infer it from event age or one offset.
12. Position observation is read-only and cannot become event execution or owner policy.
13. Keep producer storage authoritative for bounded repair.
14. Update module docs, event flow, and recovery guidance with every contract change.
15. Keep the central plan registry limited to status and nearest priority.
