# rustok-events / CRATE_API

## Public Modules
- Does not expose implementation modules; the crate re-exports canonical event contracts.

## Primary Public Types and Signatures
- `pub use crate::{DomainEvent, EventEnvelope, RootDomainEvent, RootEventEnvelope}`
- `pub use crate::EventEnvelopeError`
- `pub use crate::{EventSchema, FieldSchema, EventValidationError, ValidateEvent}`
- `pub use crate::{EventContract, ContractEventPayload, ContractEventEnvelope, EventContractEnvelopeError}`
- `pub use crate::{ForumMentionEvent, FORUM_MENTION_EVENT_SCHEMAS}`
- `pub use crate::{MarketplaceListingEvent, MARKETPLACE_LISTING_EVENT_SCHEMAS}`
- `ContractEventEnvelope::{payload, into_payload}` return only semantically validated typed payloads
- `pub fn event_schema(event_type: &str) -> Option<&'static EventSchema>`
- `pub fn event_schemas() -> impl Iterator<Item = &'static EventSchema>`
- `pub fn domain_event_json_schema() -> serde_json::Value`
- `pub fn event_envelope_json_schema() -> serde_json::Value`

## Events
- Publishes: N/A (event contracts only).
- Consumes: N/A.
- Established root events use `DomainEvent`/`EventEnvelope`.
- Bounded event families use sealed `EventContract` implementations and `ContractEventEnvelope`.
- `ForumMentionEvent` defines v1 `forum.mention.user_added` and `forum.mention.audience_added` with source revision and target identity only.

## Dependencies on Other RusToK Crates
- `rustok-telemetry`

## Common AI Mistakes
- Changes payload/event-type without updating schema registry, contract tests, relay, and transport evidence.
- Continues to import event contracts from `rustok-core` instead of `rustok-events`.
- Implements arbitrary external `EventContract` types; the trait is intentionally sealed.
- Stores bounded-family payloads as untyped `serde_json::Value` instead of adding one typed `ContractEventPayload` family variant.
- Adds contact data, source body or profile handle snapshots to Forum mention events instead of stable identities.
- Reads a manually deserialized envelope payload without revalidating it.
- Adds new compatibility aliases without architectural justification.

## Minimum Contract Set

### Input DTOs/Commands
- Event input is defined by the public event enums and envelope constructors.
- All public payload field changes are breaking unless a new schema version and consumer migration plan are provided.

### Domain Invariants
- Every root and typed-family event validates before durable publication and again after
  durable/streaming deserialization.
- Envelope event type/schema version must match the typed payload and a registered schema.
- Tenant, envelope, correlation, causation, and optional actor identities must not be nil.
- Root envelope trace identifiers must be non-empty and at most 512 bytes.
- `payload` and `into_payload` fail closed when semantic or schema validation fails.
- Forum mention events expose source revision and resolved user/audience identity only; contact and rendered content remain owner-private.
- Marketplace listing events expose only stable identity/scope/version fields; moderation prose and arbitrary metadata remain owner-private.

### Events / Outbox Side Effects
- Owner modules publish sealed contracts through `TransactionalEventBus::publish_contract_in_tx` inside the owner transaction.
- Root and bounded-family envelopes remain distinct typed transport profiles.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- `EventValidationError`, `EventEnvelopeError`, and `EventContractEnvelopeError` define stable validation classes.
- Unregistered event type, schema mismatch, payload metadata mismatch, invalid decoded payload, and root-family conversion failure must not be hidden as arbitrary transport errors.
