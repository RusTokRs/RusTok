# rustok-ai-product / CRATE_API

## Public surface

### Constants & Identifiers
- `PRODUCT_COPY_TASK_SLUG`: task slug for product copy generation.
- `PRODUCT_ATTRIBUTES_TASK_SLUG`: task slug for attribute enrichment.
- `PRODUCT_COPY_TOOL_NAME`: tool binding for product copy.
- `PRODUCT_ATTRIBUTES_TOOL_NAME`: tool binding for attribute extraction.

### Descriptors & Models
- `ProductAiAgentDescriptor`: declarations for product copywriter and attribute enricher.
- `ProductAiWorkflowDescriptor`: multi-stage product enrichment workflow descriptor.
- `GeneratedProductCopy`: structured generated marketing copy and bullet points.
- `GeneratedProductAttributes`: structured generated attribute schema.
- `GeneratedFlexAttribute`: typed attribute key/value pair.

### Validators & Registrations
- `validate_product_agent_stage_input`: checks product ID validity before task scheduling.
- `validate_product_copy_payload`: validates generated product title and description text.
- `validate_product_attributes_payload`: validates generated attribute collections.
- `product_ai_agents`: returns registered product agent descriptors.
- `product_ai_workflows`: returns registered enrichment workflows.
- `register_product_ai_verticals`: discovery helper.
- `register_product_ai_vertical_handlers`: registers product vertical handlers into the AI orchestrator.

## Platform Capability Bindings
- **Product AI Verticals**: Binds product copy and attribute enrichment to the platform AI orchestrator.
- **Enrichment Workflow**: Declares multi-stage enrichment pipelines with review approval gates.
- **Domain Seam**: Keeps ecommerce product AI policies decoupled from core AI runtime.

## Domain Collaborations
- **`rustok-ai`**: Orchestrator scheduling agents and invoking registered direct tools.
- **`rustok-product`**: Downstream owner for product entities, catalogs, and attributes.
- **`rustok-commerce`**: Commercial context and merchandising rules.

## Minimum Contract Set

### Input DTOs/Commands
- `GeneratedProductCopy`: structured payload containing title, description, and bullet points.
- `GeneratedProductAttributes`: structured payload containing extracted attribute lists.
- `ProductAiAgentDescriptor`: agent configuration inputs.

### Domain Invariants
- **Valid Product Reference**: Stage input validation requires a valid, non-nil product UUID.
- **Non-Swallowed Validation**: Empty titles or invalid attribute entries yield typed validation errors.
- **Advisory Staging**: Generated product copy and attributes require human operator approval before persisting to catalog.

### Events / Outbox Side Effects
- Dispatches product enrichment jobs and audit logs via standard task-job transports.

### Errors / Failure Codes
- Validation failures produce descriptive error strings on missing product IDs, empty strings, or schema violations.

