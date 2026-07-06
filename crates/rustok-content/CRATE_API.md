# rustok-content / CRATE_API

## Public Modules
`dto`, `entities`, `error`, `locale`, `services`, `state_machine`.

## Primary Public Types
- `pub struct ContentModule`
- `pub struct ContentOrchestrationService`
- `pub trait ContentOrchestrationBridge`
- `pub struct PromoteTopicToPostInput`
- `pub struct DemotePostToTopicInput`
- `pub struct SplitTopicInput`
- `pub struct MergeTopicsInput`
- `pub struct OrchestrationResult`
- `pub type ContentResult<T>`
- `pub enum ContentError`

## Runtime Role
- `rustok-content` no longer exposes product GraphQL/REST CRUD surfaces.
- The crate remains a shared helper layer for locale, slug, rich-text, and legacy content helpers.
- `NodeService` remains available only via `rustok_content::services::NodeService` as a shared-node helper and migration surface, but must not be used as the new primary persistence model for `blog`, `forum`, `pages`, or `comments`.
- `ContentOrchestrationService` is a port-based orchestration core. It owns RBAC checks, idempotency, audit logging, and event publication, while domain conversion work is delegated through `ContentOrchestrationBridge`.

## Orchestration Contract
- `ContentOrchestrationService` owns the following cross-domain use cases:
  - `promote_topic_to_post`
  - `demote_post_to_topic`
  - `split_topic`
  - `merge_topics`
- `ContentOrchestrationBridge` is the only extension point for runtime adapters that know how to read/write `blog`, `forum`, and `comments` domain data.
- The crate must not reintroduce direct `NodeService`-based child rebinding for orchestration flows.

## Events
- The crate publishes orchestration events through `TransactionalEventBus`.
- Event payloads and event types must remain backward-compatible for downstream consumers.

## Errors
- `ContentError::Validation(String)` covers invalid orchestration inputs and contract violations.
- `ContentError::Forbidden(String)` covers RBAC failures.
- `ContentError::Database(DbErr)` covers persistence failures, including orchestration audit/idempotency tables.

## Minimum Contract Set

### Input DTOs/Commands
- Public orchestration commands are defined by the `*Input` structs exported from `services`.
- Changes to the public fields of these command types are breaking changes for orchestration consumers.

### Domain Invariants
- Multi-tenant isolation and state-machine validation remain mandatory invariants.
- Invalid transitions, unsafe payloads, and cross-tenant access must fail with domain errors.

### Events / Outbox Side Effects
- Orchestration events must be published through `TransactionalEventBus`.
- Event payloads and event types must remain stable for cross-module consumers.

### Errors / Failure Codes
- `ContentError` and `ContentResult<T>` define the stable failure contract of the crate.
