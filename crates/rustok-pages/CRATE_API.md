# rustok-pages / CRATE_API

## Public Modules
`dto`, `entities`, `error`, `services`.

## Primary Public Types and Signatures
- `pub struct PagesModule`
- `pub struct PageService`, `MenuService`, `BlockService`
- `pub struct Page`, `Menu`, `Block`
- `pub enum PagesError`, `pub type PagesResult<T>`

## Events
- Publishes page/menu/block domain events through `TransactionalEventBus`.
- Consumes: does not subscribe to external events directly.

## Dependencies on Other RusToK Crates
- `rustok-core`
- `rustok-content`
- `rustok-outbox`

## Common AI Mistakes
- Confuses `Page` and `Block` in service signatures.
- Forgets to synchronize publish/unpublish in `PageService`.
- Uses DTOs instead of ORM entities in SeaORM queries.

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate (see sections with `Create*Input`/`Update*Input`/query/filter above and corresponding `pub` exports in `src/lib.rs`).
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.
- `PageBodyInput` and legacy `blocks` are independent surfaces: absence of `body` does not synthesize it from blocks, and writing `body` does not delete or automatically convert existing blocks.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
