# rustok-outbox / CRATE_API

## Public Modules
`entity`, `migration`, `ports`, `relay`, `transactional`, `transport`.

## Primary Public Types and Signatures
- `pub struct TransactionalEventBus`
- `pub async fn TransactionalEventBus::publish_root_in_tx(...)`
- `pub async fn TransactionalEventBus::publish_root_in_tx_with_envelope_id(...) -> Result<Uuid>`
- `pub async fn TransactionalEventBus::publish_in_tx(...)`
- `pub async fn TransactionalEventBus::publish_contract_in_tx<C, E>(...) where E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_in_tx_with_causation<C, E>(...) where E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_in_tx_with_causation_and_envelope_id<C, E>(...) -> Result<Uuid> where E: EventContract`
- `pub async fn TransactionalEventBus::publish_contract_direct_in_tx_with_causation_and_envelope_id<C, E>(...) -> Result<Uuid> where E: EventContract`
- `pub struct OutboxRelay`, `pub struct RelayConfig`, `pub struct RelayMetricsSnapshot`
- `pub struct OutboxTransport`
- `pub trait TransactionalEventWriter`
- `pub struct SysEventsMigration`
- `pub use entity::{Entity as SysEvents, Model as SysEvent}`

## Events
- Persists established root `EventEnvelope` records and sealed `ContractEventEnvelope` records in `sys_events`.
- Relays root events through `EventTransport::publish`.
- Relays bounded typed families through `EventTransport::publish_contract`.
- Validates payload metadata against the durable row before dispatch.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`
- `rustok-events`

## Common AI Mistakes
- Publishes an event directly to a transport instead of the transactional bus inside the owner transaction.
- Calls non-transactional `publish` from an owner command.
- Constructs `OutboxTransport` inside a domain operation instead of receiving a
  composed `TransactionalEventWriter`.
- Uses the typed envelope's own ID where a rollout contract requires the exact predecessor root-envelope ID.
- Publishes a caused typed envelope after the owner transaction commits.
- Confuses `OutboxTransport` with the real L2 streaming transport (`rustok-iggy`).
- Deserializes every outbox payload only as `EventEnvelope<DomainEvent>` and sends bounded-family events to the DLQ.
- Trusts duplicated `sys_events` metadata without comparing it to the envelope.

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
- Every API requires the live owner transaction; identity-returning variants do
  not publish a second envelope or reconstruct an ID after persistence.

### Domain Invariants
- State, owner timeline, root envelope, caused typed envelope, and command receipt commit or roll back together when one owner transaction publishes them.
- Caused typed envelopes preserve a non-nil predecessor identity in envelope metadata.
- Event type and schema version in the row must match the decoded envelope and its typed payload.
- Invalid, unregistered, or inconsistent payloads follow retry/DLQ policy and never reach the target transport.
- Claim ownership must be retained before dispatch completion or retry mutation.

### Events / Outbox Side Effects
- `OutboxRelay` supports both the established root envelope and the sealed typed-family envelope.
- The target transport receives the matching `publish` or `publish_contract` call.
- Event payload and event-type format remain backward-compatible for cross-module consumers.
- Causation-aware publication changes envelope metadata only; it does not alter an event family's payload schema.

### Errors / Failure Codes
- Validation, serialization, lost-claim, retry, and DLQ outcomes must remain distinguishable in logs and durable row state.
- Infrastructure details must not leak into domain-facing owner errors.
