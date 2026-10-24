# rustok-content-orchestration / CRATE_API

## Public surface

- Modules: `errors`, `runtime`, `graphql`, `bridge`, `route_resolver`.
- Core Types: `SharedContentOrchestrationService`, `ServerContentOrchestrationBridge`, `OwnerCanonicalRouteResolver`.
- Core Functions: `build_content_orchestration_service`, `content_orchestration_from_shared`.

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

- Atomic cross-module transformations: moving entities between Blog and Forum must be transactionally safe or fail closed.
- Canonical URL preservation: redirects are registered in owner tables (`blog_post_routes`, `forum_topic_routes`) on transformation.
- Incompatible content safety: Blog-to-Forum or Forum-to-Blog conversion is rejected when comments/replies cannot be losslessly translated.

### Events / Outbox Side Effects

- Outbox notifications are dispatched via `TransactionalEventBus` for cross-domain actions: `TopicPromotedToPost`, `PostDemotedToTopic`, `TopicSplit`, `TopicsMerged`.
- Emits `CanonicalUrlChanged` and `UrlAliasPurged` events upon route mutation.

### Errors / Failure Codes

- Domain errors return typed `ContentError` variants (`Validation`, `Conflict`, `NotFound`, `Forbidden`, `Database`).
