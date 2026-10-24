# rustok-content / CRATE_API

## Public surface

- Modules: `analytics`, `dto`, `entities`, `error`, `locale`, `richtext`, `services`, `state_machine`, `module`.
- Types: `ContentModule`, `ContentOrchestrationService`, `ContentOrchestrationBridge`, `ContentCountSnapshot`, `ContentError`, `ContentResult<T>`, `Draft`, `Published`, `Archived`.
- Core DTOs: `PromoteTopicToPostInput`, `DemotePostToTopicInput`, `SplitTopicInput`, `MergeTopicsInput`, `OrchestrationResult`.

## Public modules

`analytics`, `dto`, `entities`, `error`, `locale`, `richtext`, `services`, `state_machine`, `module`.

## Platform Capability Bindings

- **Core Module Registration**: Exposes `ContentModule` implementing `RusToKModule` (slug `"content"`) and `MigrationSource`.
- **Shared Content Helpers**:
  - `locale`: Resolved locale handling, normalization, fallback resolution across content entities.
  - `richtext`: RichText parsing, schema validation, profile manifests, HTML rendering, and limits.
  - `analytics`: Post stats snapshots and aggregated content counts.
  - `state_machine`: Typestate transition lifecycle for content nodes (`Draft`, `Published`, `Archived`).
- **Cross-Domain Orchestration Core**:
  - `ContentOrchestrationService`: Orchestrates topic-to-post promotion, post-to-topic demotion, topic splitting, and topic merging.
  - `ContentOrchestrationBridge`: Port abstraction bridging Blog, Forum, and Comments domains without tight coupling.

## Domain Collaborations

- **`rustok-blog`**: Bridges blog post entity lifecycle with cross-domain content transformation.
- **`rustok-forum`**: Bridges forum topic and post entities with orchestration conversion flows.
- **`rustok-comments`**: Re-attaches or converts comment threads during topic and post transformation.
- **`rustok-outbox` / `rustok-events`**: Publishes orchestration audit events and domain conversion notifications.

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
- The crate remains a shared helper layer for locale, slug, richtext, canonical routes and cross-owner orchestration.
- Generic category CRUD, category DTOs, and category runtime entities are not a public Content contract. Retained historical category migrations remain only for database compatibility; live category aggregates and hierarchy semantics belong to domain owners such as Blog and Forum.
- `ContentOrchestrationService` is a port-based orchestration core. It owns RBAC checks, idempotency, audit logging, and event publication, while domain conversion work is delegated through `ContentOrchestrationBridge`.

## Orchestration Contract

- `ContentOrchestrationService` owns the following cross-domain use cases:
  - `promote_topic_to_post`
  - `demote_post_to_topic`
  - `split_topic`
  - `merge_topics`
- `ContentOrchestrationBridge` is the only extension point for runtime adapters that know how to read/write `blog`, `forum`, and `comments` domain data.
- The crate must not reintroduce generic shared-node CRUD or child rebinding for owner-domain orchestration flows.

## Minimum Contract Set

### Input DTOs/Commands

- Public orchestration commands are defined by the `*Input` structs exported from `services`: `PromoteTopicToPostInput`, `DemotePostToTopicInput`, `SplitTopicInput`, `MergeTopicsInput`.
- Changes to the public fields of these command types are breaking changes for orchestration consumers.

### Domain Invariants

- Multi-tenant isolation and state-machine validation remain mandatory invariants.
- Invalid transitions, unsafe payloads, and cross-tenant access must fail with domain errors.
- Typestate nodes (`Draft`, `Published`, `Archived`) strictly enforce valid transition paths.

### Events / Outbox Side Effects

- Orchestration events must be published through `TransactionalEventBus`.
- Event payloads and event types must remain stable and backward-compatible for cross-module consumers.

### Errors / Failure Codes

- `ContentError` and `ContentResult<T>` define the stable failure contract of the crate.
- Variants: `Validation`, `Forbidden`, `Database`, `NotFound`, `Internal`.
