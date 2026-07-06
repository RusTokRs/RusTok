# rustok-index / CRATE_API

## Public Modules
`content`, `error`, `product`, `search`, `traits`.

## Primary Public Types and Signatures
- `pub struct IndexModule`
- `pub trait Indexer`, `pub trait LocaleIndexer`
- `pub struct IndexerContext`
- `pub enum IndexError`, `pub type IndexResult<T>`

## Events
- Publishes: usually not a source of business events.
- Consumes: indexed content/commerce events through the application integration layer.

## Dependencies on Other RusToK Crates
- `rustok-core`

## Common AI Mistakes
- Confuses domain-level indexer (`traits`) with specific search adapters.
- Indexes without considering locale.

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
