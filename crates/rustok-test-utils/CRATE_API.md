# rustok-test-utils / CRATE_API

## Public Modules
`db`, `events`, `fixtures`, `helpers`.

## Primary Public Types and Signatures
- `pub async fn setup_test_db(...)`
- `pub struct MockEventBus`, `pub struct MockEventTransport`
- `pub fn mock_transactional_event_bus() -> TransactionalEventBus`
- Domain entity fixtures in `fixtures::*`.

## Events
- Publishes: test `DomainEvent` through mock transport.
- Consumes: recorded event envelopes for assertions.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-outbox`

## Common AI Mistakes
- Adds the crate to production dependencies (should be dev-only).
- Expects real broker delivery instead of behavior mock transport.

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
