# rustok-ai-order / CRATE_API

## Public surface

### Constants & Identifiers
- `ORDER_ANALYTICS_TASK_SLUG`: task slug for order analytics generation.
- `ORDER_OPS_ASSISTANT_TASK_SLUG`: task slug for order operations assistant.
- `ORDER_ANALYTICS_TOOL_NAME`: direct tool binding for analytics.
- `ORDER_OPS_ASSISTANT_TOOL_NAME`: direct tool binding for operations assistant.
- `ORDER_AI_VERTICALS`: static list of registered order AI verticals.

### Descriptors & Policies
- `OrderAiExecutionPolicy`: execution policy (`review_required`, `persistence`).
- `OrderAiVerticalDescriptor`: metadata descriptor for order AI capabilities.
- `GeneratedOrderAnalytics`: generated schema for order analysis.
- `GeneratedOrderOpsAssistant`: generated schema for operator suggestions.

### Validators & Registrations
- `validate_order_analytics_payload`: validates summary and non-empty finding arrays.
- `validate_order_ops_assistant_payload`: validates recommended actions and confidence bounds.
- `validate_order_ops_assistant_confidence`: bounds check confidence between 0 and 100.
- `order_ai_verticals`: retrieves slice of order vertical descriptors.
- `order_ai_execution_policy`: queries policy by task slug.
- `register_order_ai_verticals`: discovery helper.
- `register_order_ai_vertical_handlers`: registers vertical handlers into AI runtime orchestrator.

## Platform Capability Bindings
- **Order AI Verticals**: Exposes order analytics and operations assistant to `rustok-ai`.
- **Advisory Output Gating**: Mandates human operator review (`review_required = true`) for all order actions.
- **Contract Boundary**: Enforces schema validation without compile-time coupling to order entity tables.

## Domain Collaborations
- **`rustok-ai`**: Orchestrator executing order tools and dispatching workflows.
- **`rustok-order`**: Downstream consumer and domain authority for actual order modifications.
- **`rustok-commerce`**: Commercial context and customer order lifecycle contracts.

## Minimum Contract Set

### Input DTOs/Commands
- `GeneratedOrderAnalytics`: structured output payload for order analysis.
- `GeneratedOrderOpsAssistant`: structured suggestion payload for order operators.
- `OrderAiVerticalDescriptor`: vertical configuration definition.

### Domain Invariants
- **Advisory Guardrail**: All order AI outputs are purely advisory; automatic order mutations without human approval are prohibited.
- **Confidence Bounding**: Operations assistant confidence scores must strictly remain within `0..=100`.
- **Non-Swallowed Validation**: Empty summaries or blank array values return descriptive validation errors.

### Events / Outbox Side Effects
- Emits analytics and recommendation payloads through standard task-job transports.

### Errors / Failure Codes
- Validation failures produce descriptive error strings on blank strings, missing summaries, or invalid confidence.

