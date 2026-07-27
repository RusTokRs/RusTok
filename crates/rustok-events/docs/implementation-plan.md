# Implementation plan for `rustok-events`

## Source of truth

This file is the live plan for shared event contracts and the guarantees remote
consumers may rely on. Transport execution remains owned by Outbox, Iggy, the server
runtime, or the consuming owner module.

Last reconciled with `main`: 2026-07-27.

## Current state

`rustok-events` is the canonical source of `DomainEvent`, `EventEnvelope`, sealed
typed families, schema metadata, semantic validation, and versioning policy.
`rustok-core::events` is a compatibility re-export only.

The committed `contracts/event-contract-digests.json` gates the root registry and
root/typed wire schemas. Digest generation is deterministic and never occurs as an
implicit test/build side effect. The current release train intentionally allows only
version-1 event schemas until remote consumer migration ownership is retained.

Root and typed envelopes validate at event-bus, outbox write, relay, and JSON or
MessagePack decode boundaries. The configured `EventRuntime` is published before
module dispatcher startup. The outbound relay has explicit task ownership, restart,
shutdown, and readiness guardrails.

The typed-family implementation includes sealed
`social_graph.relation.state_changed` v1. Its payload contains relation id,
source/target user ids, canonical relation kind, active state, and revision only.
Tenant and actor remain envelope metadata; command idempotency, expected revision,
request context, receipt snapshots, claims, roles, locale, and channel are excluded.

The Social Graph owner publishes this fact transactionally and provides bounded,
service/system-only, tenant/UUID-cursor replay over authoritative relation state.
Replay is dry-run capable and page-atomic.

## Delivered Social Graph → Index consumer contract

- Index is the first approved consumer of the sealed relation family.
- Active relations map to generic non-localized upserts, inactive relations to
  tombstones, relation id to entity identity, and revision to monotonic
  `source_version`.
- `SocialGraphIndexProjector` registers or exactly recognizes the tenant schema
  through Index-owned `PostgresSchemaRegistrationStore` before mutation apply.
- `PostgresMutationStore` commits inbox terminal state with projection state.
- `Applied`, `Duplicate`, and `StaleIgnored` are durable terminal outcomes.
- Persistent group `rustok-social-graph-index` consumes the shared `domain` topic.
- Unrelated sealed families are acknowledged without schema registration or mutation.
- Staged receive/project/ack retains one outstanding broker item across retries.
- The server lifecycle is default-off and requires explicit
  `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED=true`, a worker host, and effective
  `outbox_iggy` delivery.
- Shared `StopHandle` controls graceful shutdown and a worker handle exposes task
  readiness source state.
- Projection failures use bounded exponential retry from reviewed event settings.
- Before a durable owner result, permanent/exhausted failures may publish exact
  original broker bytes to DLQ and only then acknowledge, when DLQ policy is enabled.
- When DLQ is disabled or publication fails, the source offset remains uncommitted.
- After a durable Index result, only acknowledgement is retried; the delivery is not
  DLQed because redelivery is duplicate/stale safe.
- Successfully decoded contract deliveries retain exact raw bytes for lossless DLQ.
  Undecodable broker bytes remain unacknowledged pending a lower-level connector
  poison-message contract.
- Bounded Social Graph replay uses the same schema/inbox/source-version path for
  projection repair.
- Profiles privacy remains on synchronous authoritative Social Graph ports.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `in_progress`
- Structural shape: `core -> transport -> ui/leptos`, with a sibling module-owned
  Next package.
- `rustok-events-module` remains the cycle-free runtime/manifest adapter and owns its
  admin delivery-profile surface.
- The host provides route composition and shared delivery control; it does not own
  event schemas or module UI.

## Completed source results

- [x] Keep one canonical event/envelope/schema definition in `rustok-events`.
- [x] Validate root and typed payloads at publication, relay, and decode boundaries.
- [x] Keep the root registry and committed digest artifact synchronized.
- [x] Generate reviewed Draft 2020-12 wire schemas deterministically.
- [x] Keep contact data out of shared user-registration facts.
- [x] Keep `translation.target.changed` content-free and transactionally published.
- [x] Own and guard the server outbound relay lifecycle.
- [x] Add sealed `social_graph.relation.state_changed` v1 and owner transactional
  publication.
- [x] Add bounded authoritative Social Graph replay through the same family.
- [x] Add generic Index conversion and Index-owned tenant schema registration.
- [x] Add persistent result-first Index consumption with duplicate/stale recognition.
- [x] Add default-off server startup, strict delivery-profile gating, shared shutdown,
  bounded retry, exact-byte DLQ-before-ack, and acknowledgement-only recovery after
  durable apply.
- [x] Add permanent source guards for lifecycle order and foreign-table isolation.

## Open results

1. **Keep the reviewed event-contract digest synchronized.**
   Regenerate only through the deterministic example when a reviewed contract shape
   changes. Done when canonical tests report no drift.

2. **Keep event types, registry, release artifact, and consumer imports synchronized.**
   New families require direct imports from `rustok-events`, semantic coverage, and
   matching owner/outbox/recovery guidance.

3. **Complete operator-visible remote consumer lifecycle.**
   Wire `SocialGraphIndexWorkerHandle` into `/health/ready`, metrics, and required vs
   disabled reporting. Preserve default-off semantics and fail startup on invalid
   enablement/profile combinations.

4. **Prove remote cursor recovery.**
   Execute real-Iggy restart, missed fast path, redelivery, ack failure, DLQ failure,
   connector loss, shutdown, and multi-replica ownership scenarios. Done when a
   replica can restart, recover from persisted state, and acknowledge only after a
   durable owner result.

5. **Prove Index repair and concurrency.**
   Execute PostgreSQL concurrent schema registration/mutation, deliberately create
   projection drift, and repair it through bounded owner replay/rescan while privacy
   remains on owner ports.

6. **Close the DLQ acknowledgement window.**
   Decide whether publish-success/source-ack-failure requires a durable owner receipt
   or another idempotent DLQ identity contract. Undecodable raw deliveries also need
   a connector-level poison-message shape.

7. **Synchronize recovery guidance.**
   Outbox, replay, reindex, and DLQ procedures must name the exact schema and avoid
   transport-owned copies of payload definitions.

## Verification

- `cargo xtask module validate events`
- `cargo xtask module test events`
- `cargo test -p rustok-events --test social_graph_contracts -- --nocapture`
- `cargo run -p rustok-events --example event_contract_digests`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-index --all-targets`
- `cargo test -p rustok-index schema_registration --lib -- --nocapture`
- `node scripts/verify/verify-index-schema-registration.mjs`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index --all-targets`
- `cargo test -p rustok-social-graph --features index index::tests -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index-consumer --all-targets`
- `cargo test -p rustok-social-graph --features index-consumer index_consumer::tests -- --nocapture`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets`
- `cargo test -p rustok-server social_graph_index_worker --lib -- --nocapture`
- `cargo test -p rustok-social-graph --test relation_event_replay_sqlite -- --nocapture`
- `node scripts/verify/verify-social-graph-relation-event-replay.mjs`
- `node scripts/verify/verify-social-graph-index-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-runtime-consumer.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `cargo test -p rustok-server --test event_bus_runtime_guard`
- `cargo test -p rustok-server event_forwarder --lib`
- `cargo clippy -p rustok-server --lib -- -D warnings`
- Real-broker multi-replica restart/recovery and PostgreSQL evidence.

These commands and scenarios remain maintainer-run and were not executed manually in
this slice.

## Change rules

1. Keep canonical event payloads and schemas in this module.
2. Keep transport execution in its runtime owner; do not copy payload definitions.
3. Update the digest artifact only through intentional contract review.
4. Consumers import sealed contracts directly and persist or recognize their tenant
   schema and owner result before acknowledgement.
5. Source consumers never write another owner's schema/projection tables directly.
6. DLQ publication, when permitted, precedes source acknowledgement; durable-result
   ack failure is acknowledgement-only recovery.
7. Keep producer storage authoritative for bounded repair.
8. Update module docs, event flow, and recovery guidance with every contract change.
9. Keep the central plan registry limited to status and nearest priority.
