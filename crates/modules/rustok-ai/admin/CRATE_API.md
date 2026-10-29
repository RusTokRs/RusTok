# rustok-ai-admin / CRATE_API

## Public surface

### Entry Point & Components
- `AiAdmin`: root mounted administration component rendering the AI control plane.
- `transport`: transport adapters for native server function and GraphQL communication.

## Platform Capability Bindings
- **Admin Control Plane**: Registers `AiAdmin` as the administration panel for AI provider and session configuration.
- **Provider & Agent Management**: Manages AI providers, model assignments, tool policies, and execution profiles.
- **Diagnostics & Streaming**: Subscribes to live session events via GraphQL WebSocket (`aiSessionEvents`) and monitors run telemetry.

## Domain Collaborations
- **`rustok-ai`**: Core AI capability providing LLM abstractions, provider adapters, and orchestration.
- **`rustok-mcp`**: Tool protocol integration for model context protocol tool execution.
- **`rustok-ui-core`**: Core UI text normalization and parsing primitives.
- **`rustok-ui-i18n`**: Fluent translation loader and localization context.

## Minimum Contract Set

### Input DTOs/Commands
- `ImageTaskPayloadInput`: input parameters for AI image generation task requests.
- `alloy_task_payload`: payload constructor for Alloy script execution tasks.
- `image_task_payload`: payload serializer for media generation requests.

### Domain Invariants
- **Non-Swallowed Errors**: AI execution errors and diagnostic failures must be explicitly displayed and propagated rather than swallowed.
- **Advisory Output**: AI-generated suggestions (orders, media, text) remain advisory and review-gated until accepted by operators.
- **Context Localization**: UI chrome uses package-owned Fluent bundles (`en.ftl`, `ru.ftl`) driven by host route locale.

### Events / Outbox Side Effects
- Emits task execution commands across native server functions and GraphQL subscriptions.
- Streams live assistant tokens and tool invocation events via `aiSessionEvents`.

### Errors / Failure Codes
- `UiError`: presentation and input validation error states.
- `TransportError`: network and RPC failure codes across native server and GraphQL transports.

