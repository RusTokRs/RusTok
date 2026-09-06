# rustok-iggy-connector / CRATE_API

## Public Modules
- API is declared in `lib.rs`.
- Feature `migrations` exposes connector migrations, the neutral consumer-poison receipt store, and read-only aggregate inspection through `rustok_iggy_connector::migrations`.

## Primary Public Types and Signatures
- `pub enum ConnectorMode { Bundled, External }`
- `pub struct BundledConnectorConfig`, `ExternalConnectorConfig`, `ConnectorConfig`
- `pub trait IggyConnector`
- `pub trait IggyConnectorControl`
- `pub struct IggyConnectorConfigurationSnapshot`, `IggyConnectorSettingsInput`
- `pub trait MessageSubscriber`, `pub trait ConsumerCursor`
- `pub struct SubscriberMessage`, `SubscriberMessageMetadata`
- `pub enum ConnectorError`
- `migrations::ConsumerPoisonIdentity`
- `migrations::ConsumerPoisonReceiptStore`
- `migrations::ConsumerPoisonReceiptState { Reserved, Publishing, Published, Acknowledged }`
- `migrations::ConsumerPoisonPublishClaim`
- `migrations::ConsumerPoisonReceiptInspector`
- `migrations::ConsumerPoisonReceiptSummary`
- Implementations: `ExternalConnector`, `BundledConnector` and subscriber structs.

## Events
- Publishes/consumes Iggy binary messages within the connector; it does not interpret domain authorization.
- The neutral poison receipt stores exact undecodable bytes and immutable broker coordinates only after a higher transport layer supplies the deterministic connector delivery UUID.
- Aggregate inspection is scoped to one validated consumer-group identifier and returns counts only.

## Dependencies on Other RusToK Crates
- No direct dependencies on other `rustok-*`.
- Feature `migrations` uses SeaORM storage primitives for connector control-plane persistence.

## Common AI Mistakes
- Confuses `Bundled` and `External` configs during initialization.
- Considers the connector as a full EventBus rather than a connection/IO layer.
- Adds tenant or domain event identity to a receipt for bytes that have not decoded successfully.
- Rejects an empty payload even though zero bytes are a valid exact broker delivery that can fail decoding.
- Treats error classification or observed retry count as part of immutable poison identity.
- Publishes a DLQ entry or acknowledges an offset from `ConsumerPoisonReceiptStore`; the store owns durable result states only.
- Uses `ConsumerPoisonReceiptInspector` to reclaim, delete, acknowledge, or choose alert policy; it is read-only aggregate storage inspection.
- Exposes delivery IDs, source coordinates, payloads, error classifications, publisher identities, or timestamps through operational summaries.
- Treats `published` as proof of source acknowledgement or as broker exactly-once.

## Minimum Contract Set

### Input DTOs/Commands
- Connector DTO changes remain breaking changes and require synchronized transport adapters.
- `ConsumerPoisonIdentity` requires a non-nil deterministic delivery UUID, bounded consumer group/stream/topic, positive partition, representable offset, and exact payload bytes; an empty payload is valid. Its fields are private after construction and exposed read-only.
- `reserve_and_claim` requires one stable bounded error code, positive observed delivery-attempt count, non-nil publisher identity, and a whole-second lease between 1 and 86400 seconds.
- `ConsumerPoisonReceiptInspector::summarize` requires the same bounded, trimmed, control-character-free consumer-group identity used by durable receipts.

### Domain Invariants
- `Bundled` accepts exactly one loopback TCP address matching its configured TCP port and launches the configured executable directly, without a shell.
- `Bundled` is unavailable on Windows because upstream `iggy-server` does not support that operating system; Windows deployments use `External`.
- Persistent consumer groups require TCP. `External` supports SDK TLS options; `Bundled` keeps broker/client loopback-only and rejects TLS bootstrap.
- Persisted external credentials are resolver/key references; plaintext passwords are resolved only inside server runtime.
- Poison receipt source coordinates are unique. Reuse with another deterministic delivery UUID or exact payload fails closed as an identity conflict. The UUID itself is also globally bound to one set of source coordinates and bytes.
- Stable error code and delivery-attempt count are first-observed diagnostics retained on initial reservation. Later decoder classification or retry-count changes do not redefine the connector delivery identity.
- Receipt transitions are `reserved -> publishing -> published -> acknowledged`; expired publication leases may be reclaimed.
- The receipt stores no tenant, decoded event, actor, claims, locale, credential, acknowledgement token, or authorization fact.
- `ConsumerPoisonReceiptSummary` exposes only total, reserved, publishing, expired-publishing, published, and acknowledged counts. Expired publishing is a subset of publishing.
- Inspection fails closed when known-state counts do not equal total or an expired-publishing count exceeds publishing.

### Events / Outbox Side Effects
- Domain publication still uses the transactional outbox/transport contract.
- The poison receipt store performs no publish, subscribe, DLQ routing, offset commit, or acknowledgement operation.
- Aggregate inspection performs one read-only query and has no claim, release, publish, acknowledge, repair, retention, or deletion side effect.
- A consumer may acknowledge only after it has persisted or recognized a terminal result and completed any required external publication.

### Errors / Failure Codes
- Receipt and inspection errors expose bounded codes under `iggy.connector.poison_*` for invalid identity, identity conflict, invalid stored state, lost claim, and storage failure.
- Only lost-claim and storage outcomes are classified retryable.
- Infrastructure details must not leak into domain-facing owner errors.
