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
- `pub async fn add_sys_events_tenant_column(manager)` / `drop_sys_events_tenant_column` / `create_sys_events_tenant_index` / `drop_sys_events_tenant_index` / `add_sys_events_integrity_checks` / `drop_sys_events_integrity_checks` (`migration`)
- `pub async fn idempotency::renew_lease(database, lease)` / `idempotency::prune_terminal_receipts(database, older_than, batch_size)` (`idempotency`)
- `pub use entity::{Entity as SysEvents, Model as SysEvent}`

## Events

- Persists established root `EventEnvelope` records and sealed `ContractEventEnvelope` records in `sys_events`.
- Supports transaction-only sealed-contract write-once boundaries keyed by an exact caller-owned envelope UUID, with optional exact predecessor causation.
- Relays root events through `EventTransport::publish`.
- Relays bounded typed families through `EventTransport::publish_contract`.
- Validates registered payload ID, populated tenant projection, type and schema
  version against the durable row before dispatch. Historical null tenant
  projections remain permitted.
- Refreshes the fenced claim before dispatch and checks the timeout budget.
- Retains delivered rows for a configured window: `OutboxRetention::prune_once` deletes only
  aged `dispatched` rows by default. An explicit failed-event window also permits
  aged DLQ rows. Both require all registered consumer acknowledgements and a
  final deletion fenced against concurrent registry writes. Pending rows are never pruned.

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

## Database clock authority

Durable relay claims, retry deadlines, storage timestamps and retention cutoffs
use SQL database time. PostgreSQL uses clock_timestamp(), not transaction-start
time; SQLite uses its SQL clock. Receipt renewals and reclaims use a PostgreSQL
write-side clock expression; reclaim compares the previous token and timestamp.
Relay renewal rotates its token even within the same clock tick and counts
database wait time in its monotonic dispatch budget. Clock errors fail closed.
Relay and retention intervals are bounded to 3650 days. This does not fence
primary clock discontinuities or guarantee exactly-once broker effects.

## Relay execution admission

Relay passes reserve at most max_concurrency rows across their sliding window and
advance a stable (created_at, id) cursor within the invocation. Due/reclaim
eligibility uses one initial SQL-clock horizon. A shared pass guard serializes
clones of a relay instance, including aborted tasks until they actually exit.
Independent updated relay instances/processes share a durable schema-local
execution quota via SQL claims. Reports count the joined
durable outcomes of the specific pass, not differences of lifetime counters.
A failed or cancelled pass can have committed earlier outcomes.
Retry uses equal jitter between half the capped exponential delay and the cap
(with a one millisecond floor). Startup validates relay configuration before
connecting broker transports. The sliding window replenishes available slots without waiting for a full wave.

## Strict owner-operation lease expiry

Owner-operation receipts now enforce a strict five-minute half-open live
interval. A matching token cannot renew, lock, finalize or release an expired
receipt. Sample SQL time after acquiring the receipt write lock, recheck
expiry on reclaim and restamp fresh admission after unique-index contention.
Completion and failure use a transaction or an owner-transaction savepoint;
owner errors require rollback of the entire owner transaction. Expired release
preserves the request fingerprint for exact-request reclamation. Owners must
renew before expiry and keep owner transactions bounded; a row lock alone
does not extend the lease. External effects need owner-specific idempotency.

## Durable temporal clock authority

The outbox_temporal_clock singleton stores the maximum committed SQL-clock
observation across process restarts. Independent relay-pass, receipt admission,
renewal/reclaim and retention boundaries advance it in a short transaction.
Owner transactions read the floor and SQL time without taking the global writer
lock. Missing schema, backward observations below the committed floor, slow
observations (>2s), host-clock steps (>100ms during observation) or SQL/host
skew outside a 2s sanity bound fail closed; host time is not a timestamp
fallback. Relay claim refresh checks its live interval after acquiring row
ownership. Event retention checks time again after the registry fence.

This is bounded discontinuity detection, not a trusted independent distributed
time service: simultaneous host/DB forward jumps, smaller jumps within the
sanity bound and a stale repeatable-read snapshot are not fully fenced.
A floor ahead of actual time blocks affected operations until time catches up
or an explicitly reviewed operator recovery is performed. Do not reset it on
startup. The floor begins at zero on migration; pre-upgrade history is not
retroactively verified. Upgrade the schema before new workers and drain old
workers that bypass the guard. A committed owner transaction may still be
subject to cancellation and ambiguous broker effects.

## Distributed execution admission (2026-11-27)

The outbox_execution_policy singleton owns a per-database-schema budget
(max_in_flight, default 8, range 1..1024) and the shared claim_ttl_ms.
The first updated relay binds the TTL; subsequent embedded and standalone
relays must match it before starting their brokers or claiming work.
The local max_concurrency remains an upper bound, independent of the cluster
budget. Existing sys_events claimed_by/claimed_at pairs are the only durable
execution permits. Claim and refresh lock the policy before event rows;
PostgreSQL uses READ COMMITTED, SKIP LOCKED for candidates and NOWAIT for
refresh. The policy lock is released before any broker effect. Completed
claims release capacity through existing conditional state transitions.
Cancelled or crashed publications may retain reservations until expiry;
transport effects remain at-least-once and may outlive cancellation.
A sliding window replenishes a slot after a joined outcome rather than waiting
for a whole wave. Claim failures stop new admission and drain active tasks.
The pass cursor and initial eligibility horizon are preserved.

Deployment: stop and drain all old relay processes before applying
m20261127_000201_create_outbox_execution_policy. Old binaries bypass the
budget. Ensure no old in-flight publications remain before starting new
relays. Apply the migration before starting new workers; the schema helper
does not reset an existing policy. For a deliberate policy change, stop and
drain every relay, verify there are no live or ambiguous claims, then change
the singleton max_in_flight and claim_ttl_ms through controlled database
maintenance and align all runtime TTL settings before restart. No live
policy editing API is provided; online cap reduction or TTL change is not
supported. Multiple databases/schemas have independent budgets. Direct SQL
claim mutations outside the protocol, primary clock anomalies, uncooperative
transport cancellation and broker-side fencing remain outside this guarantee.

## Publication identity and acknowledgement boundary (2026-11-27)

Canonical root and sealed event UUIDs now populate the Iggy message header
ID without translation. Non-UUID publication keys, including the existing
DLQ replay attempt keys, map to a fixed namespaced UUIDv5; nil UUIDs and
empty keys are rejected. The namespace and the existing partition routing
must remain stable across retries. Explicit direct-mode publication awaits
the broker reply rather than accepting background enqueue as delivery.
No-SDK external connections and publications fail closed. The dedicated
poison-DLQ path retains its existing identity and partition mapping.
The strength of the broker response still depends on topic/server durability
and the reviewed broker deduplication policy. Successful direct send is not
a physical persistence proof or consumer cursor acknowledgement. An empty
confirmation list can be legitimate (including a duplicate response); no
broker offsets or persistence guarantees are inferred. Older publications
without the header identity cannot be retroactively deduplicated. Changing
topology, partition count or broker destination requires coordinated drain
and recovery. Do not treat SDK producer idempotence as cross-process Outbox
exactly-once delivery.

The relay sends invalid stored envelopes directly to its DLQ before invoking
the target. Every target error, including validation/serialization after a
remote send or a local projection, consumes the bounded ordinary retry
budget instead of assuming there was no side effect. Timeout outcomes remain
unconfirmed and may redeliver the same identity. A successful transport
reply followed by SQL failure or process death may also redeliver; the
existing fenced SQL acknowledgement and consumer idempotency still apply.
No transaction spans the database and the broker.
