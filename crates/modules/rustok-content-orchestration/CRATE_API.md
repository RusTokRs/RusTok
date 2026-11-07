# rustok-content-orchestration / CRATE_API

## Public surface

- **Modules**: `errors`, `runtime`, `graphql`, `bridge`, `route_resolver`.
- **Core Types**:
  - `SharedContentOrchestrationService` (implements `Clone`, `Debug`) - runtime service handle wrapper.
  - `ServerContentOrchestrationBridge` (implements `Debug`, `rustok_content::ContentOrchestrationBridge`) - server-side transactional bridge across blog, forum, comments, taxonomy, and outbox.
  - `OwnerCanonicalRouteResolver` (implements `Debug`, `rustok_content::CanonicalRouteResolver`) - canonical route and historical alias resolver for content modules.
  - `ContentOrchestrationMutation` - GraphQL root mutation type exposing orchestration operations to the API gateway.
- **GraphQL Inputs / Payloads**:
  - `PromoteTopicToPostInput`, `DemotePostToTopicInput`, `SplitTopicInput`, `MergeTopicsInput` (implement `InputObject`, `Debug`, `Clone`).
  - `ContentOrchestrationPayload` (implements `SimpleObject`, `Debug`, `Clone`, `From<OrchestrationResult>`).
- **Core Functions**:
  - `build_content_orchestration_service(db, event_bus) -> SharedContentOrchestrationService`
  - `content_orchestration_from_shared(&SharedContentOrchestrationService) -> Arc<ContentOrchestrationService>`

## Feature Flags

- `mod-content`: Activates integration with `rustok-content` port contracts and domain types.
- `mod-blog`: Activates blog post conversions and taxonomy synchronization.
- `mod-forum`: Activates forum topic conversions and taxonomy synchronization.
- `mod-comments`: Activates comment thread transformations.
- Full bridge and GraphQL mutations require all four features enabled simultaneously (`mod-content`, `mod-blog`, `mod-forum`, `mod-comments`). In reduced builds, a lightweight mock handle is provided without pulling external module dependencies.

## Platform Capability Bindings

- **Cross-Domain Content Orchestration Bridge**: Implements the `rustok_content::ContentOrchestrationBridge` contract for blog, forum, and comments domain workflows.
- **Canonical Route Resolution**: Implements `rustok_content::CanonicalRouteResolver` via `OwnerCanonicalRouteResolver` to resolve canonical content URLs and historical redirects.
- **Host Runtime Construction**: Provides `build_content_orchestration_service` to initialize the orchestration runtime within the server composition root without leaking domain models into `apps/server`.

## Domain Collaborations

- **`rustok-content`**: Consumes orchestration command contracts, DTOs, and port interfaces (`ContentOrchestrationBridge`, `CanonicalRouteResolver`).
- **`rustok-blog`**: Manages blog post entities, categories, and route publication during cross-domain content movements.
- **`rustok-forum`**: Manages forum topic and post entities, categories, and route publication during cross-domain content movements.
- **`rustok-comments`**: Re-attaches or converts comment threads during promotion and demotion flows.
- **`rustok-taxonomy`**: Synchronizes taxonomy terms and tags between different domain representations.
- **`rustok-outbox`**: Emits audit trails and outbox domain notifications upon orchestration events.

## Minimum Contract Set

### Input DTOs/Commands

- Consumes commands from `rustok-content`: `PromoteTopicToPostInput`, `DemotePostToTopicInput`, `SplitTopicInput`, `MergeTopicsInput`.
- Exposes GraphQL mutation root `ContentOrchestrationMutation` for server composition.

### Domain Invariants

- **Multi-Tenant Isolation**: Every database read, mutation, and delete statement explicitly enforces `tenant_id` equality (`Column::TenantId.eq(tenant_id)`).
- **Non-Nil UUID Guards**: All tenant IDs, topic IDs, post IDs, category IDs, and reply IDs are validated against `is_nil()` before database operations.
- **Atomic Cross-Module Transformations**: Moving entities between Blog and Forum must be transactionally safe within a single database transaction or fail closed.
- **Canonical URL Preservation**: Redirects are registered in owner tables (`blog_post_routes`, `forum_topic_routes`) on transformation, and obsolete routes are purged.
- **Incompatible Content Safety**: Blog-to-Forum or Forum-to-Blog conversion is rejected when comments/replies cannot be losslessly translated until universal richtext adoption is complete.
- **Unique Reply IDs**: Splitting topics validates that all reply IDs are unique, non-empty, and belong to the source topic.
- **Unique Source Topics**: Merging topics requires at least one source topic, validates uniqueness, and prohibits including the target topic among the sources.

### Events / Outbox Side Effects

- Outbox notifications are dispatched via `TransactionalEventBus` for cross-domain actions: `TopicPromotedToPost`, `PostDemotedToTopic`, `TopicSplit`, `TopicsMerged`.
- Emits `CanonicalUrlChanged` and `UrlAliasPurged` events upon route mutation.

### Errors / Failure Codes

- Domain errors return typed `ContentError` variants:
  - `Validation(String)`: Invalid UUIDs, missing required parameters, duplicate target IDs, or richtext format incompatibilities.
  - `NodeNotFound(Uuid)`: Source topic or blog post does not exist.
  - `CategoryNotFound(Uuid)`: Target category does not exist.
  - `TranslationNotFound { id, locale }`: Required translation not found for requested locale.
  - `DuplicateSlug { locale, slug }`: Target slug already taken.
  - `Forbidden(String)`: Caller lacks required moderation permissions.
  - `ConcurrentModification { id, version }`: Optimistic locking conflict during mutation.
  - `InvalidStatusTransition(InvalidStatusTransition)`: Invalid lifecycle transition.
  - `Database(DbErr)`: Storage engine errors.
  - `Core(CoreError)`: Platform infrastructure errors.
  - `Rich(RichTextError)`: AST parsing and schema validation errors.
- GraphQL mapping (`map_content_error`) converts domain errors to typed `FieldError` instances with appropriate internal error masking for database and core failures.
