# rustok-outbox / CRATE_API

## Public Modules
`entity`, `migration`, `ports`, `relay`, `transactional`, `transport`; `loco` is available with the `loco-adapter` feature.

## Primary Public Types and Signatures
- `pub struct TransactionalEventBus`
- `pub struct OutboxRelay`, `pub struct RelayConfig`, `pub struct RelayMetricsSnapshot`
- `pub struct OutboxTransport`
- `pub struct SysEventsMigration`
- `pub use entity::{Entity as SysEvents, Model as SysEvent}`

## Events
- Publishes: `EventEnvelope` to transport after transaction commit.
- Consumes: outbox records (`sys_events`) for relay/dispatch.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-api`
- `rustok-events`

## Common AI Mistakes
- Publishes event directly to transport instead of `TransactionalEventBus::publish` inside a transaction.
- Confuses `OutboxTransport` with the real L2 transport (`rustok-iggy`).

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate (see sections with `Create*Input`/`Update*Input`/query/filter above and corresponding `pub` exports in `src/lib.rs`).
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
