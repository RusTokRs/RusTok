# rustok-iggy-connector / CRATE_API

## Public Modules
- API is declared in `lib.rs` (no `pub mod` sections).

## Primary Public Types and Signatures
- `pub enum ConnectorMode { Bundled, External }`
- `pub struct BundledConnectorConfig`, `ExternalConnectorConfig`, `ConnectorConfig`
- `pub trait IggyConnector`
- `pub trait IggyConnectorControl`
- `pub struct IggyConnectorConfigurationSnapshot`, `IggyConnectorSettingsInput`
- `pub trait MessageSubscriber`
- `pub struct SubscriberMessage`, `SubscriberMessageMetadata`
- `pub enum ConnectorError`
- Implementations: `ExternalConnector`, `BundledConnector` and subscriber structs.

## Events
- Publishes/consumes Iggy binary messages within the connector (not `DomainEvent` directly).

## Dependencies on Other RusToK Crates
- No direct dependencies on other `rustok-*`.

## Common AI Mistakes
- Confuses `Bundled` and `External` configs during initialization.
- Considers the connector as a full EventBus (it is a connection/IO layer).

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate (see sections with `Create*Input`/`Update*Input`/query/filter above and corresponding `pub` exports in `src/lib.rs`).
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters in `apps/server`.

### Domain Invariants
- Module invariants are enforced in services/state machines and DTO validation; invalid transitions/parameters must result in a domain error.
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.
- `Bundled` accepts exactly one loopback TCP address matching its configured TCP
  port and launches the configured executable directly, without a shell.
- `Bundled` is unavailable on Windows because upstream `iggy-server` does not
  support that operating system; the connector fails configuration explicitly
  and Windows deployments must use `External`.
- Persistent consumer groups require TCP. `External` supports SDK TLS options;
  `Bundled` keeps the broker and client on loopback and rejects TLS bootstrap.
- Persisted external credentials are `SecretRef`-style resolver/key references.
  Plaintext passwords are resolved only inside the server runtime.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
