# rustok-ai-content / CRATE_API

## Public surface

### Constants & Identifiers
- `CONTENT_MODERATION_TASK_SLUG`: task slug for AI content moderation.
- `BLOG_DRAFT_TASK_SLUG`: task slug for AI blog draft generation.
- `CONTENT_MODERATION_TOOL_NAME`: direct tool binding for moderation.
- `BLOG_DRAFT_TOOL_NAME`: direct tool binding for draft generation.
- `CONTENT_AI_VERTICALS`: static list of registered content AI verticals.

### Descriptors & Policies
- `ContentAiApprovalMode`: policy mode (`Auto`, `OperatorApproval`).
- `ContentAiDegradedMode`: degraded fallback mode (`RequireOperatorReview`, `KeepDraftForReview`).
- `ContentAiPolicyRule`: structure defining moderation policies and rationales.
- `ContentAiVerticalDescriptor`: metadata descriptor for content AI capabilities.

### Validators & Registrations
- `validate_blog_draft_payload`: validates generated blog post fields.
- `validate_moderation_decision`: validates content moderation decisions.
- `content_ai_policy_matrix`: returns policy rules across content verticals.
- `register_content_ai_verticals`: registers content verticals into AI registry.
- `register_content_ai_vertical_handlers`: binds handlers to AI runtime orchestrator.

## Platform Capability Bindings
- **Content AI Verticals**: Binds moderation and blog draft tasks to the platform AI orchestrator.
- **Review Gating & Policy**: Enforces approval and degraded modes across content workflows.
- **Domain Seam**: Acts as domain adapter without coupling AI runtime directly to content modules.

## Domain Collaborations
- **`rustok-ai`**: Core AI orchestrator invoking registered content tools and policies.
- **`rustok-blog`**: Consumes AI-assisted drafting capabilities.
- **`rustok-comments`**: Consumes automated comment moderation.

## Minimum Contract Set

### Input DTOs/Commands
- `ContentAiPolicyRule`: parameters defining approval and degraded fallback modes.
- `ContentAiVerticalDescriptor`: configuration inputs for vertical registration.

### Domain Invariants
- **Sensitive Task Review**: Moderation tasks flagged sensitive require operator approval in degraded states.
- **Non-Swallowed Validation**: Malformed generated drafts or invalid moderation outcomes fail with descriptive errors.
- **Decoupled Business Rules**: Content policies remain owned by this adapter rather than host UI or generic AI runtime.

### Events / Outbox Side Effects
- Emits moderation decisions and draft generation completions via standard task-job transports.

### Errors / Failure Codes
- Validation failures produce structured error results detailing invalid text fields or policy violations.

