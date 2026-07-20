# rustok-outbox / CRATE_API

## Public Modules
`entity`, `migration`, `ports`, `relay`, `transactional`, `transport`.

## Primary Public Types and Signatures
- `pub struct TransactionalEventBus`
- `pub async fn TransactionalEventBus::publish_in_tx(...)`
- `pub async fn TransactionalEventBus::publish_contract_in_tx<C, E>(...) where E: EventContract`
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
- Confuses `OutboxTransport` with the real L2 streaming transport (`rustok-iggy`).
- Deserializes every outbox payload only as `EventEnvelope<DomainEvent>` and sends bounded-family events to the DLQ.
- Trusts duplicated `sys_events` metadata without comparing it to the envelope.

## Minimum Contract Set

### Input DTOs/Commands
- Root events use `publish_in_tx`.
- Sealed bounded-family events use `publish_contract_in_tx`.
- Both APIs require the live owner transaction and preserve the returned envelope identity internally.

### Domain Invariants
- State, owner timeline, outbox envelope, and command receipt commit or roll back together.
- Event type and schema version in the row must match the decoded envelope and its typed payload.
- Invalid, unregistered, or inconsistent payloads follow retry/DLQ policy and never reach the target transport.
- Claim ownership must be retained before dispatch completion or retry mutation.

### Events / Outbox Side Effects
- `OutboxRelay` supports both the established root envelope and the sealed typed-family envelope.
- The target transport receives the matching `publish` or `publish_contract` call.
- Event payload and event-type format remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Validation, serialization, lost-claim, retry, and DLQ outcomes must remain distinguishable in logs and durable row state.
- Infrastructure details must not leak into domain-facing owner errors.
