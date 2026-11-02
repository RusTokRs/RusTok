# rustok-outbox / CRATE_API

## Public Modules

`entity`, `migration`, `ports`, `relay`, `retention`, `transactional`, `transport`.

## Primary Public Types and Signatures

- `pub struct TransactionalEventBus`
- `pub async fn TransactionalEventBus::publish_root_in_tx(...)`
- `pub async fn TransactionalEventBus::publish_root_in_tx_with_envelope_id(...) -> Result<Uuid>`
- `pub async fn TransactionalEventBus::publish_in_tx(...)`
- `pub async fn TransactionalEventBus::publish_contract_in_tx<C, E>(...) where E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_direct_in_tx<C, E>(...) -> Result<()> where C: ConnectionTrait, E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_in_tx_with_causation<C, E>(...) where E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_in_tx_with_causation_and_envelope_id<C, E>(...) -> Result<Uuid> where E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_direct_in_tx_with_causation_and_envelope_id<C, E>(...) -> Result<Uuid> where E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id<C, E>(...) -> Result<Uuid, ContractEventWriteOnceError> where E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id_and_causation<C, E>(...) -> Result<Uuid, ContractEventWriteOnceError> where E: EventContract`
- `pub enum ContractEventWriteOnceError { Conflict, Unavailable }`
- `pub struct OutboxRelay`, `pub struct RelayConfig`, `pub struct RelayMetricsSnapshot`
- `pub struct OutboxTransport`
- `pub trait TransactionalEventWriter`
- `pub struct SysEventsMigration`
- `pub struct OutboxRetention`, `pub struct OutboxRetentionConfig`, `pub struct OutboxPruneReport`
- `pub async fn OutboxRetention::prune_once(&self) -> Result<OutboxPruneReport>`
- `pub fn OutboxRetentionConfig::validate(&self) -> Result<()>`
- `pub const DEFAULT_OUTBOX_RETENTION_BATCH_SIZE: u64`
- `pub async fn create_sys_events_schema(manager) -> Result<(), DbErr>` / `drop_sys_events_schema` (`migration`)
- `pub async fn create_sys_events_claim_index(manager)` / `create_sys_events_retention_index(manager)` / `drop_sys_events_claim_index` / `drop_sys_events_retention_index` (`migration`)
- `pub async fn create_sys_events_superseded_indexes(manager)` / `drop_sys_events_superseded_indexes` (`migration`)
- `pub use entity::{Entity as SysEvents, Model as SysEvent}`

## Events

- Persists established root `EventEnvelope` records and sealed `ContractEventEnvelope` records in `sys_events`.
- Supports transaction-only sealed-contract write-once boundaries keyed by an exact caller-owned envelope UUID, with optional exact predecessor causation.
- Relays root events through `EventTransport::publish`.
- Relays bounded typed families through `EventTransport::publish_contract`.
- Validates payload metadata against the durable row before dispatch.
- Retains delivered rows for a configured window: `OutboxRetention::prune_once` deletes only
  `dispatched` rows whose `dispatched_at` is older than the window, in bounded batches, and never
  touches `pending` (still owed) or `failed` (DLQ) rows.

## Platform Capability Bindings

- **rustok-core**: Platform error traits, module lifecycle descriptor, and base migration primitives
- **rustok-api**: Standard API response envelopes and error definitions
- **rustok-events**: Canonical event envelope traits and contract event serialization
- **rustok-telemetry**: Retention metrics counters, outbox queue gauges, and dispatch latency histograms

## Domain Collaborations

- All domain modules emitting transactional events or storing durable command idempotency receipts via `TransactionalEventBus`

## Common AI Mistakes

- Publishes an event directly to a transport instead of the transactional bus inside the owner transaction.
- Calls non-transactional `publish` from an owner command.
- Constructs `OutboxTransport` inside a domain operation instead of using a canonical transactional API.
- Generates a new write-once envelope UUID instead of passing the exact already-durable owner idempotency identity.
- Treats any primary-key collision as an exact replay without decoding and comparing the stored registered envelope.
- Compares volatile trace or timestamp metadata when deciding whether a replay is exact.
- Ignores tenant, actor, causation, correlation, event type, schema version, or typed payload when deciding whether request-ID reuse conflicts.
- Copies a causal predecessor into typed payload data instead of passing it through the caused write-once API.
- Uses the typed envelope's own ID where a rollout contract requires the exact predecessor root-envelope ID.
- Publishes a caused typed envelope after the owner transaction commits.
- Confuses `OutboxTransport` with the real L2 streaming transport (`rustok-iggy`).
- Deserializes every outbox payload only as `EventEnvelope<DomainEvent>` and sends bounded-family events to the DLQ.
- Trusts duplicated `sys_events` metadata without comparing it to the envelope.
- Reimplements relay leases, retry, DLQ, or retention in an owner-specific writer.
- Prunes `pending` or `failed` rows, or prunes without a bounded batch and a configured window.
- Duplicates the `sys_events` DDL in a platform migration instead of calling the owner schema helper.

## Minimum Contract Set

### Input DTOs/Commands

- Root events use `publish_in_tx` or the static `publish_root_in_tx` owner helper.
- Owners that must durably bind audit state to the exact root envelope use
  `publish_root_in_tx_with_envelope_id`; the returned UUID is the ID of the
  validated envelope written by the same transaction.
- Sealed bounded-family events use `publish_contract_in_tx`.
- A composed owner that must retain an exact predecessor uses
  `publish_contract_in_tx_with_causation` or its identity-returning variant.
- A transaction-only owner helper uses
  `publish_contract_direct_in_tx_with_causation_and_envelope_id`; it writes the
  same validated contract envelope directly into the canonical outbox table and
  does not create a second transport path.
- A write-once owner with an existing non-nil UUID uses
  `publish_contract_once_direct_in_tx_with_envelope_id`. The supplied UUID is
  both canonical envelope ID and correlation ID.
- A write-once owner that also has one exact durable predecessor uses
  `publish_contract_once_direct_in_tx_with_envelope_id_and_causation`. The
  predecessor remains envelope metadata and participates in exact-replay
  conflict admission.
- Every API requires the live owner transaction; identity-returning variants do
  not publish a second envelope or reconstruct an ID after persistence.

### Domain Invariants

- Unconditional root/typed publication requires exactly one affected insert row. A suppressed
  zero-row insert is persistence failure, never owner success or proof of write-once replay.
- State, owner timeline, root envelope, caused typed envelope, and command receipt commit or roll back together when one owner transaction publishes them.
- Write-once insertion uses the `sys_events.id` primary key with `ON CONFLICT DO NOTHING`, then reads the winning row through the same transaction.
- Exact replay requires matching envelope ID, correlation ID, causation ID, tenant ID, actor ID, event type, schema version, and validated typed payload.
- Trace ID and envelope timestamp are generated by the first writer and are not compared during exact replay admission.
- Mismatched identity or causation reuse returns `Conflict`; insert/read/deserialize/validation or row-envelope consistency failures return `Unavailable`.
- Replay may return an existing pending, dispatched, or failed canonical row because delivery lifecycle is independent from source idempotency admission.
- Caused typed envelopes preserve a non-nil predecessor identity in envelope metadata.
- Event type and schema version in the row must match the decoded envelope and its typed payload.
- Invalid, unregistered, or inconsistent payloads follow retry/DLQ policy and never reach the target transport.
- Claim ownership must be retained before dispatch completion or retry mutation.

### Events / Outbox Side Effects

- The write-once APIs insert only into canonical `sys_events`; they do not send directly to a broker or transport.
- `OutboxRelay` supports both the established root envelope and the sealed typed-family envelope.
- The target transport receives the matching `publish` or `publish_contract` call.
- Event payload and event-type format remain backward-compatible for cross-module consumers.
- Exact identity and causation-aware publication change envelope construction or metadata only; they do not alter an event family's payload schema or committed event-contract digests.

### Errors / Failure Codes

- `ContractEventWriteOnceError::Conflict` means the requested exact envelope UUID already belongs to different canonical scope, causation, or typed facts.
- `ContractEventWriteOnceError::Unavailable` covers validation, serialization, database, missing-row, corrupt-row, or unreadable-state failure without exposing infrastructure details to the owner.
- Validation, serialization, lost-claim, retry, and DLQ outcomes must remain distinguishable in logs and durable row state.
- Infrastructure details must not leak into domain-facing owner errors.

## Fresh sealed facts and local acknowledgement (2026-11-01; source only)

`publish_contract_direct_in_tx(txn, tenant_id, actor_id, event)` constructs and validates a
fresh canonical sealed envelope and writes it through `OutboxTransport` on the caller's live
owner transaction. No direct broker send, alternate table, receipt or synthetic predecessor is
introduced. Inventory calls it only for fresh positive reservation movement; replay/noop needs
no Outbox insertion. Write/admission failure must abort the owner transaction.

Relay retains matching root/typed transport dispatch and existing retry/DLQ. The configured
server target now awaits explicit sealed local handlers on `outbox`; on `outbox_iggy`, primary
acceptance precedes required local acknowledgement. Local failure leaves the same event identity
retryable, even if Iggy already accepted it. This is at-least-once; remote durable receive/cursor
ACK and crash/restart behavior are not proved by local acknowledgement or source guards.
See the [Inventory packet](../rustok-inventory/contracts/evidence/reservation-events-source.json).

## Inventory source canonical continuation (2026-11-02; uncompiled)

Inventory uses `publish_contract_once_direct_in_tx_with_envelope_id` on the actual caller
`DatabaseTransaction`: retained change UUID for Changed, globally unique original baseline UUID
for fresh final BaselineActivated. Current actors are checked before the shared helper can
filter nil. Building seeds, no-op/replay and observation do not publish. Journal is not outbox,
receipt or global cursor. The existing typed canonical relay handles the registered family;
four new storage/relay definitions remain uncompiled/unrun, not live broker evidence.

Required Inventory facts without a matching active local handler now refuse **before** Iggy
primary acceptance. Matched local failures still propagate **after** primary acceptance and
retain the same frame for at-least-once retry. Remote-only Product primary/cursor routing is
preserved. No production Inventory source listener is selected; current source authority,
durable initial replay and generic Index persist-before-ACK remain pending.
[Source packet](../rustok-inventory/contracts/evidence/source-delivery-source.json).
