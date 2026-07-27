# `rustok-social-graph` implementation gates

The Social Graph owner was introduced by `SOCIAL-01A / NOTIFY-07C`. This file is
the live owner plan for relation persistence, privacy reads, durable commands,
events, replay, maintenance, and approved projections. Cross-module presentation
policy remains coordinated by `crates/rustok-profiles/docs/implementation-plan.md`.

## Current state

`rustok-social-graph` owns tenant-scoped block, mute, and directional follow state.
Social Graph persistence and synchronous owner ports remain authoritative for
privacy and follower policy. Profiles, Notifications, Index, search, or any other
projection must not read owner tables or authorize from replicated relation state.

The source-complete path now includes durable command receipts, bounded cleanup,
transactional sealed relation events, bounded owner replay, an approved generic
Index relation projection, Index-owned tenant schema registration, result-first
persistent Iggy consumption, and a default-off server lifecycle with bounded retry
and exact-byte DLQ-before-ack behavior.

Compilation, source verifiers, PostgreSQL concurrency, real-broker restart,
multi-replica recovery, readiness endpoint integration, and retained runtime
evidence remain maintainer-run or pending.

## Delivered owner relation contract

- PostgreSQL and SQLite migrations own one tenant/source/target/kind relation row.
- `SocialRelationKind::{Block, Mute, Follow}` defines canonical stored/event values.
- Source and target use tenant-composite integrity; self-relations are rejected.
- Relation revisions are positive and monotonic.
- `SocialGraphCommandPort` requires deadline, non-empty idempotency identity,
  actor/source ownership, target separation, canonical kind, and optional expected
  revision.
- `SocialGraphPrivacyReadPort` owns block/mute/follow policy reads.
- `SocialGraphFollowReadPort` exposes revision-bearing directional state.
- Follow batches are bounded to 100 unique targets and fail closed on owner errors.
- Profiles followers-only presentation always uses these owner ports.

## Delivered public follow transport and telemetry

- Optional feature `graphql` publishes module-owned query and mutation roots.
- Public reads expose actor-owned following state without relation identifiers.
- Follow/unfollow requires explicit idempotency key and supports positive expected
  revision.
- Human-user, tenant, deadline, claim, and channel context is validated before the
  owner port.
- Owner command telemetry records stable operation, bounded identities, outcome,
  duration, error code, and retryability only.
- Idempotency keys, request correlation, claims, roles, locale, channel, receipt
  payloads, and presentation copy remain excluded.

## Delivered durable command receipts

- Migration `m20260726_000003_create_command_receipts` owns versioned processing and
  completed receipts with tenant-scoped normalized idempotency identity.
- Receipt reservation, relation mutation, optional event append, response snapshot,
  completion, and commit share one database transaction.
- Exact replay returns the committed response even when live relation revision later
  advances.
- Reusing a key with another command identity fails as
  `social_graph.idempotency_conflict` without mutation.
- Unsupported, incomplete, or corrupt receipts fail closed as
  `social_graph.command_receipt_corrupt`.

## Delivered receipt maintenance and CLI

- `SocialGraphReceiptMaintenancePort` and its owner service expose service/system-only
  bounded dry-run/live cleanup.
- Selection is tenant-scoped, schema-v1, completed-only, strictly before an explicit
  cutoff, ordered by `(completed_at, id)`, and all candidates are validated before
  deletion.
- Limit is 1..1000; corrupt candidate state fails the complete batch.
- `rustok-social-graph-cli` exposes `social_graph receipt-cleanup` through the
  generated distribution registry.
- Tenant and positive retention days are mandatory; there is no retention default.
- CLI output is aggregate only and no scheduler or automatic cleanup is enabled.
- Receipt retention must cover the longest supported client retry horizon, clock
  skew, and incident replay allowance.

## Delivered transactional relation events and replay

- `rustok-events` owns sealed `social_graph.relation.state_changed` v1.
- Payload contains relation id, source/target user ids, canonical kind, active state,
  and revision only; tenant and actor remain envelope metadata.
- A new relation or persisted active-state transition publishes through
  `TransactionalEventBus::publish_contract_in_tx` before receipt completion and
  shared commit.
- Receipt replay and exact persisted-state no-op publish no new live event.
- Event publication failure rolls relation and receipt back together.
- `SocialGraphRelationEventMaintenancePort` provides service/system-only tenant and
  exclusive-UUID-cursor bounded replay with dry-run and page-atomic publication.
- Replay begins only after event-aware writers are active and is at-least-once.
- Consumers apply by relation id plus monotonic revision and acknowledge only after
  their durable result.
- Social Graph persistence remains authoritative for drift repair.

## Delivered approved Index contract and storage boundary

- `rustok-index` is the first named approved relation-event consumer.
- Feature `index` publishes a non-localized generic relation schema and converts the
  sealed event into `IndexMutation` without broker or database logic.
- Active state maps to upsert, inactive state to revisioned tombstone, relation id to
  entity identity, and relation revision to `source_version`.
- Feature `index-consumer` adds optional Iggy/Index runtime dependencies.
- `SocialGraphIndexProjector` persists or exactly recognizes the tenant schema through
  Index-owned `PostgresSchemaRegistrationStore` before mutation apply.
- Registration is tenant-scoped, exact-version idempotent, monotonic, and fail-closed
  for contract drift, retired state, invalid tenant, unsupported backend, or storage
  failure.
- Social Graph imports no Index entities and never writes `index_schemas` directly.
- `PostgresMutationStore` atomically records inbox terminal state with active/tombstone
  projection state.
- `Applied`, `Duplicate`, and `StaleIgnored` are terminal durable outcomes.

## Delivered persistent consumer and host lifecycle

- Persistent group `rustok-social-graph-index` consumes the shared `domain` topic.
- Only `ContractEventPayload::SocialGraphRelation` reaches schema registration and
  mutation apply; unrelated sealed families are acknowledged without projection.
- `receive_next`, `project_consumed`, and `acknowledge_consumed` expose a staged
  result-first flow while retaining one outstanding cursor delivery.
- Stable error codes and retry classification separate transient transport/storage
  ownership failures from permanent validation/contract conflicts.
- `process_next(&mut self)` remains the direct serialized receive/register/apply/ack
  path.
- The server `mod-social_graph` feature composes `index-consumer`, but execution is
  default-off until `RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED=true`.
- Explicit enablement requires a worker host and effective `outbox_iggy` delivery.
- `SocialGraphIndexWorkerHandle` exposes instance/task state and the worker subscribes
  to shared `StopHandle` shutdown.
- Projection retry is bounded exponential backoff derived from reviewed event relay
  settings.
- Before a durable Index result, a permanent or exhausted projection failure is
  published to DLQ only when event DLQ is enabled; exact original broker bytes are
  published before source acknowledgement.
- When DLQ is disabled or DLQ publication fails, the worker terminates with the source
  offset uncommitted.
- After a durable Index result, only acknowledgement is retried. Ack failure is never
  converted into a poison delivery; redelivery is duplicate/stale safe.
- Malformed bytes that fail before a decoded `ConsumedContractEvent` remain
  unacknowledged pending a lower-level connector poison-delivery contract.

## Remaining Social Graph scope

1. Wire `SocialGraphIndexWorkerHandle` into `/health/ready`, operator metrics, and
   explicit required/disabled lifecycle reporting.
2. Execute PostgreSQL concurrent schema-registration and mutation evidence.
3. Prove real Iggy restart/redelivery, ack failure, DLQ failure, and multi-replica
   cursor ownership behavior.
4. Deliberately corrupt/delete projection state and prove repair through bounded owner
   replay/rescan while Profiles privacy remains on authoritative owner ports.
5. Decide whether DLQ publication requires an additional durable owner receipt to
   deduplicate publish-success/source-ack-failure windows.
6. Configure deployment receipt-retention window/cadence and retain CLI dry-run/live
   evidence.
7. Collect receipt/event concurrency, cleanup, replay-window, relay, rollback, and
   telemetry failure-class evidence.
8. Continue friendship request/accept/remove lifecycle, broader directory/follow UX,
   custom lists, block/mute management transports, and moderation/admin repair.

## Verification

```bash
cargo fmt --all -- --check
RUSTFLAGS="-Dwarnings" cargo check -p rustok-events --all-targets
cargo test -p rustok-events --test social_graph_contracts -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-index --all-targets
cargo test -p rustok-index schema_registration --lib -- --nocapture
node scripts/verify/verify-index-schema-registration.mjs
RUSTFLAGS="-Dwarnings" cargo check -p rustok-iggy --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features graphql --all-targets
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index --all-targets
cargo test -p rustok-social-graph --features index index::tests -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph --features index-consumer --all-targets
cargo test -p rustok-social-graph --features index-consumer index_consumer::tests -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-server --features mod-social_graph --all-targets
cargo test -p rustok-server social_graph_index_worker --lib -- --nocapture
RUSTFLAGS="-Dwarnings" cargo check -p rustok-social-graph-cli --all-targets
cargo test -p rustok-social-graph-cli -- --nocapture
cargo test -p rustok-social-graph --test privacy_sqlite -- --nocapture
cargo test -p rustok-social-graph --test follow_sqlite -- --nocapture
cargo test -p rustok-social-graph --test follow_state_sqlite -- --nocapture
cargo test -p rustok-social-graph --test command_receipts_sqlite -- --nocapture
cargo test -p rustok-social-graph --test receipt_cleanup_sqlite -- --nocapture
cargo test -p rustok-social-graph --test relation_outbox_sqlite -- --nocapture
cargo test -p rustok-social-graph --test relation_event_replay_sqlite -- --nocapture
node scripts/generate/generate-cli-registry.mjs --check
node scripts/verify/verify-social-graph-notification-policy.mjs
node scripts/verify/verify-social-graph-command-receipts.mjs
node scripts/verify/verify-social-graph-receipt-cleanup.mjs
node scripts/verify/verify-social-graph-receipt-cleanup-cli.mjs
node scripts/verify/verify-social-graph-relation-outbox.mjs
node scripts/verify/verify-social-graph-relation-event-replay.mjs
node scripts/verify/verify-social-graph-index-consumer.mjs
node scripts/verify/verify-social-graph-index-runtime-consumer.mjs
node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs
node scripts/verify/verify-profiles-storefront-boundary.mjs
rustok-cli social_graph receipt-cleanup --tenant-id <uuid> --retention-days 30 --limit 100 --dry-run
```

These commands remain maintainer-run and were not executed manually while publishing
this slice. `Cargo.lock` must be refreshed because the optional Index/Iggy feature
edges and server feature resolution change package dependency metadata.
