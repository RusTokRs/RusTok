# rustok-ai / CRATE_API

## Platform Capability Bindings
- Implements `RusToKModule` via `AiModule`.
- Integrates with `rustok-core`, `rustok-mcp`, `rustok-runtime`, and `sea-orm`.
- Provides Rig-based inference provider bindings (Gemini, Vertex AI, Bedrock, OpenAI-compatible).

## Domain Collaborations
- Collaborates with `rustok-ai-content`, `rustok-ai-order`, `rustok-ai-product`, and `rustok-ai-media` for vertical AI tasks.
- Collaborates with `rustok-ai-admin` as a registered admin module package.
- Collaborates with `rustok-blog` and `rustok-product` via structured task ports.

## Public surface
- `pub mod module`: Native module definition (`AiModule`).
- `pub mod agent`: Agent descriptor catalog, stage statuses, and principals (`AgentCatalog`, `AgentDescriptor`, `AgentKind`, `AgentPrincipal`).
- `pub mod direct`: Task execution for first-party verticals.
- `pub mod engine`: Rig engine wrappers, provider configurations, and completion runtimes.
- `pub mod error`: Error definitions (`AiError`).
- `pub mod ports`: Neutral port definitions (`AiStructuredTaskPort`).
- `pub mod rag`: Provider-neutral RAG contracts, chunking, and document embeddings.
- `pub use module::AiModule`.

## Minimum Contract Set

### Input DTOs/Commands
- Agent run requests, chat session inputs, and task execution commands.
- Provider profile configurations (`ProviderSlug`, `ProviderTargetId`, `SecretRef`).
- Structured task requests with policy, schema, input digests, and limits.

### Domain Invariants
- AI is deployment-scoped (`ModuleKind::Core`) and globally active across tenants.
- Permission enforcement: effective permissions are the intersection of initiating subject and agent principal.
- Closed agent taxonomy (`product`, `code`, `orchestrator`, `review`).
- All executions expose content-free request bindings to support durable recovery without raw input retention.

### Events / Outbox Side Effects
- AI task executions publish run events, completion receipts, and outbox notifications when configured.

### Errors / Failure Codes
- Provider failures, egress denials, and schema validation errors produce typed `AiError` variants.

