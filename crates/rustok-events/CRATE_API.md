# rustok-events / CRATE_API

## Public Modules
- Does not expose implementation modules; the crate re-exports canonical event contracts.

## Primary Public Types and Signatures
- `pub use crate::{DomainEvent, EventEnvelope, RootDomainEvent, RootEventEnvelope}`
- `pub use crate::EventEnvelopeError`
- `pub use crate::{EventSchema, FieldSchema, EventValidationError, ValidateEvent}`
- `pub use crate::{EventContract, ContractEventPayload, ContractEventEnvelope, EventContractEnvelopeError}`
- `pub use crate::{BlogCommentsDelegationScheduleAuditEvent, BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_SCHEMAS}`
- `pub use crate::{ForumMentionEvent, FORUM_MENTION_EVENT_SCHEMAS}`
- `pub use crate::{ForumSearchProjectionEvent, FORUM_SEARCH_PROJECTION_EVENT_SCHEMAS}`
- `pub use crate::{MarketplaceListingEvent, MARKETPLACE_LISTING_EVENT_SCHEMAS}`
- `pub use crate::{MarketplaceSellerEvent, MARKETPLACE_SELLER_EVENT_SCHEMAS}`
- `pub use crate::{ReactionsEvent, REACTIONS_EVENT_SCHEMAS}`
- `pub use crate::{SocialGraphRelationEvent, SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS}`
- `pub use crate::{TranslationWorkflowEvent, TRANSLATION_WORKFLOW_EVENT_SCHEMAS}`
- `ContractEventEnvelope::new_with_envelope_id(...)` creates a registered typed envelope with one exact non-nil caller-owned durable identity
- `ContractEventEnvelope::new_with_envelope_id_and_causation(...)` creates a registered typed envelope with exact caller-owned identity and exact non-nil predecessor identity
- `ContractEventEnvelope::new_caused_by(...)` creates a registered typed envelope with one exact non-nil predecessor envelope identity and a generated envelope UUID
- `ContractEventEnvelope::{correlation_id, causation_id, tenant_id, actor_id}` expose validated envelope scope metadata
- `ContractEventEnvelope::{payload, into_payload}` return only semantically validated typed payloads
- `pub fn event_schema(event_type: &str) -> Option<&'static EventSchema>`
- `pub fn event_schemas() -> impl Iterator<Item = &'static EventSchema>`
- `pub fn domain_event_json_schema() -> serde_json::Value`
- `pub fn event_envelope_json_schema() -> serde_json::Value`
- `pub fn contract_event_payload_json_schema() -> serde_json::Value`
- `pub fn contract_event_envelope_json_schema() -> serde_json::Value`
- `pub fn event_contract_digests() -> EventContractDigests`
- `cargo run -p rustok-events --example event_contract_digests [-- --write]`
  prints or deliberately updates the committed release artifact.

## Events
- Publishes: N/A (event contracts only).
- Consumes: N/A.
- Established root events use `DomainEvent`/`EventEnvelope`.
- Bounded event families use sealed `EventContract` implementations and `ContractEventEnvelope`.
- `BlogCommentsDelegationScheduleAuditEvent` defines v1
  `blog.comments_delegation_schedule.replacement_succeeded`. It carries the
  exact successful-audit request identity, fixed state key, audit timestamp,
  bounded principal/operation/source categories, and the strictly increasing
  generation pair. Tenant and actor remain envelope metadata.
- `ForumMentionEvent` defines v1 `forum.mention.user_added` and `forum.mention.audience_added` with source revision and target identity only.
- `ForumSearchProjectionEvent` defines v1
  `forum.search_projection.invalidation_issued` with a positive Forum owner
  revision, one bounded projection target type, and a target identity only when
  the scope is a category or topic. The exact legacy root envelope identity is
  carried in typed-envelope `causation_id`, not in the payload.
- `ReactionsEvent` defines v1 `reactions.actor_state.changed` for one committed
  actor-state transition plus bounded aggregate deltas, and
  `reactions.subject.reconciled` for one committed bounded aggregate repair.
  Tenant and actor are envelope metadata. Owner command/repair identities,
  subject identity, positive revisions and bounded reaction keys are payload
  facts; producer content, visibility and presentation remain private.
- `SocialGraphRelationEvent` defines v1 `social_graph.relation.state_changed`
  as an authoritative fact for one persisted relation revision, with relation id,
  source/target user ids, canonical kind, active state, and revision only. Tenant
  and actor remain envelope metadata.
- Social Graph may replay the same persisted revision through its bounded owner
  maintenance port. This is at-least-once delivery, not a second mutation fact.
- `DomainEvent::UserAccountRegistered` defines v1
  `user.account_registered` with only `user_id`; contact data remains private
  to the auth/user owner.
- `TranslationWorkflowEvent` defines content-free v1 job, assignment, blocked
  retry, proposal, apply, and recovery lifecycle facts. Translation
  source/target values, operator reasons, owner receipts, claims, and roles
  remain owner-private.

## Dependencies on Other RusToK Crates
- `rustok-telemetry`

## Common AI Mistakes
- Changes payload/event-type without updating schema registry, committed digest artifact, contract tests, relay, and transport evidence.
- Continues to import event contracts from `rustok-core` instead of `rustok-events`.
- Implements arbitrary external `EventContract` types; the trait is intentionally sealed.
- Stores bounded-family payloads as untyped `serde_json::Value` instead of adding one typed `ContractEventPayload` family variant.
- Calls `new_with_envelope_id` with a newly generated or reconstructed UUID instead of an already durable owner idempotency identity.
- Uses the exact-identity constructor as a replacement for ordinary random envelope generation where no write-once owner contract exists.
- Calls `new_with_envelope_id_and_causation` with a typed envelope UUID as the predecessor instead of the exact durable root envelope UUID.
- Adds a Comments delegation key id, secret, schedule document, schedule digest,
  file path, database URL, token, nonce, claims, roles, raw database error, or
  free-form operator text to the Blog Comments audit event.
- Reconstructs or generates a second request identity instead of preserving the
  exact non-nil Blog audit `request_id` used for canonical handoff idempotency.
- Copies causal identity into the event payload instead of using envelope `causation_id`.
- Uses a nil, reconstructed, or unrelated UUID as a causal predecessor.
- Publishes the Forum Search typed invalidation without the legacy root envelope
  or records the typed envelope id in the Forum owner ledger.
- Adds locale, channel, visibility, rendered content, document payload, reason,
  claims, roles, or Search `ingest_sequence` to the Forum Search invalidation payload.
- Adds contact data, source body or profile handle snapshots to Forum mention events instead of stable identities.
- Publishes a Reactions semantic event outside the owner state/aggregate/receipt
  transaction, emits an event for a no-op or replay, or uses the user command
  UUID instead of the admitted owner-operation UUID as envelope identity.
- Adds producer content, visibility denial reasons, profile presentation,
  claims, roles, locale, channel or free-form repair diagnostics to Reactions
  events.
- Adds idempotency keys, expected revisions, request context, claims, roles, locale,
  channel, or receipt snapshots to Social Graph relation events.
- Treats a replayed Social Graph relation fact as exactly-once or applies a lower
  revision over a newer durable consumer result.
- Lets a Social Graph consumer projection replace the owner relation table as the
  drift-repair authority.
- Reads a manually deserialized envelope payload without revalidating it.
- Adds new compatibility aliases without architectural justification.

## Minimum Contract Set

### Input DTOs/Commands
- Event input is defined by the public event enums and envelope constructors.
- `ContractEventEnvelope::new` creates an uncaused typed envelope.
- `ContractEventEnvelope::new_with_envelope_id` is reserved for a transactional
  write-once boundary that already owns one exact non-nil durable UUID. It sets
  both envelope ID and correlation ID to that UUID and does not create a second
  idempotency identity.
- `ContractEventEnvelope::new_with_envelope_id_and_causation` preserves that
  exact write-once identity while also retaining one exact non-nil predecessor
  envelope UUID in metadata.
- `ContractEventEnvelope::new_caused_by` records one exact durable predecessor
  while generating a new envelope UUID.
- All public payload field changes are breaking unless a new schema version and consumer migration plan are provided.
- The committed `contracts/event-contract-digests.json` artifact must match the
  registry and every root/typed transport wire schema.

### Domain Invariants
- Every root and typed-family event validates before durable publication and again after
  durable/streaming deserialization.
- Envelope event type/schema version must match the typed payload and a registered schema.
- Tenant, envelope, correlation, causation, and optional actor identities must not be nil.
- Exact envelope identity construction preserves the existing serialized shape;
  it changes constructor ownership only and requires `correlation_id == id` for
  the newly constructed envelope.
- Exact caused identity construction also preserves the existing serialized
  shape and places the predecessor only in the already-registered optional
  `causation_id` field.
- Adding exact/caused envelope constructors does not change payload schemas or
  committed event-contract digests.
- Root envelope trace identifiers must be non-empty and at most 512 bytes.
- `payload` and `into_payload` fail closed when semantic or schema validation fails.
- Blog Comments schedule-audit events require audit schema version 1, the exact
  `comments_tcp_delegation_schedule` state key, a non-nil request identity, a
  positive timestamp, `direct_user|service`,
  `reload_file|replace_host_schedule`, `host_provided|file`, and
  `candidate_generation > previous_generation >= 1`.
- The Blog Comments audit `request_id` is payload data because it is the stable
  identity of the already durable source fact and the canonical writer's
  idempotency key and envelope UUID. Control-plane tenant and actor remain
  envelope metadata.
- Forum mention events expose source revision and resolved user/audience identity only; contact and rendered content remain owner-private.
- Forum Search projection invalidations require `owner_revision >= 1`, accept
  only `forum|forum_category|forum_topic`, require `target_id = null` for
  `forum`, and require a non-nil `target_id` for category/topic scope.
- Forum owner revision and Search-owned `ingest_sequence` remain independent
  counters and must never be compared numerically.
- Reactions actor-state events require non-nil command, subject and actor UUIDs,
  positive subject/state revisions, one `add|remove` action, unique bounded key
  arrays, added keys inside the resulting selection and removed keys outside it.
- Reactions reconciliation events require a non-nil repair command/subject,
  positive subject/catalog revisions, non-negative scan counts, at least one
  changed key and a truthful bounded/truncated changed-key sample.
- Social Graph relation events accept only non-nil distinct source/target ids,
  canonical `block|mute|follow` kind, and a positive monotonic revision.
- A Social Graph consumer applies by relation id plus monotonic revision, ignores
  duplicate or lower revisions, persists its owner-specific result, and acknowledges
  only after that result is durable. Social Graph remains authoritative for drift repair.
- User-account registration events expose identity only; email addresses and
  every other contact attribute must not enter the shared event stream.
- Marketplace listing events expose only stable identity/scope/version fields; moderation prose and arbitrary metadata remain owner-private.
- Translation workflow events expose only stable identities, revisions,
  bounded technical status/error codes, retryability, counts, and assignment
  actor identity.

### Events / Outbox Side Effects
- Owner modules publish sealed contracts through `TransactionalEventBus::publish_contract_in_tx` inside the owner transaction.
- Exact write-once owner operations may publish with
  `publish_contract_once_direct_in_tx_with_envelope_id` using the already
  admitted owner-operation UUID. Event conflict or unavailability must abort the
  same transaction as owner state and receipt completion.
- Exact write-once owners with a durable predecessor may use
  `publish_contract_once_direct_in_tx_with_envelope_id_and_causation`; the
  predecessor participates in exact replay admission without becoming payload data.
- This crate defines exact envelope identity and causation construction but does
  not perform database insertion, replay admission, conflict classification,
  source-row handoff, relay, retry, DLQ, or retention.
- Forum dual publication writes the legacy root first, publishes the typed
  contract caused by that exact root id, and retains the root id as owner-ledger
  and downstream projection identity.
- Root and bounded-family envelopes remain distinct typed transport profiles.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.
- The current release train permits only schema version 1. A versioned migration
  requires an accepted ADR and a durable remote-consumer delivery contract first.

### Errors / Failure Codes
- `EventValidationError`, `EventEnvelopeError`, and `EventContractEnvelopeError` define stable validation classes.
- Unregistered event type, schema mismatch, payload metadata mismatch, invalid decoded payload, and root-family conversion failure must not be hidden as arbitrary transport errors.
