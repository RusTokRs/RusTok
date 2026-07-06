# rustok-events / CRATE_API

## Public Modules
- Does not expose individual modules; the crate re-exports events.

## Primary Public Types and Signatures
- `pub use crate::{DomainEvent, EventEnvelope, EventSchema, FieldSchema}`
- `pub use crate::{EventValidationError, ValidateEvent, event_schema, EVENT_SCHEMAS}`
- `pub use crate::{RootDomainEvent, RootEventEnvelope}`

## Events
- Publishes: N/A (event contracts only).
- Consumes: N/A.

## Dependencies on Other RusToK Crates
- `rustok-telemetry`

## Common AI Mistakes
- Changes payload/event-type without updating contract tests and migration note.
- Continues to import event contracts from `rustok-core` instead of `rustok-events`.
- Adds new compatibility aliases without architectural justification.

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
