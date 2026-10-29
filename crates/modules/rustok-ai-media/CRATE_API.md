# rustok-ai-media / CRATE_API

## Public surface

### Constants & Identifiers
- `IMAGE_ASSET_TASK_SLUG`: task identifier for image generation flows.
- `IMAGE_ASSET_TOOL_NAME`: tool binding identifier for media creation.
- `MEDIA_AI_VERTICALS`: static list of registered media AI verticals.

### Descriptors & Validators
- `MediaAiVerticalDescriptor`: descriptor declaring task slug, tool name, and sensitivity.
- `normalize_image_size`: validates and bounds image dimensions (`1..=4096`).
- `media_ai_verticals`: retrieves slice of registered media AI verticals.
- `register_media_ai_verticals`: vertical discovery helper.
- `register_media_ai_vertical_handlers`: registers media vertical callbacks into the AI orchestrator.

## Platform Capability Bindings
- **Media AI Vertical**: Exposes the `image_asset` generation vertical to `rustok-ai`.
- **Dimension Validation**: Enforces dimensional bounds for media generation payloads.
- **Support Adapter Decoupling**: Pure metadata and validation layer without coupling to image backends.

## Domain Collaborations
- **`rustok-ai`**: Orchestrator executing media generation tools and tracking task status.
- **`rustok-media`**: Downstream asset storage and presentation owner.

## Minimum Contract Set

### Input DTOs/Commands
- `normalize_image_size`: input string option formatted as `WIDTHxHEIGHT`.
- `MediaAiVerticalDescriptor`: vertical configuration definition.

### Domain Invariants
- **Non-Swallowed Errors**: Invalid dimensions (non-numeric, zero, or exceeding 4096) return typed validation errors.
- **Format Integrity**: Image sizes must follow `WIDTHxHEIGHT` format without unparsed prefixes or suffixes.
- **Domain Decoupling**: Pure validation logic avoids UI or transport runtime dependencies.

### Events / Outbox Side Effects
- Media generation jobs produce asset creation events when processed by the platform orchestrator.

### Errors / Failure Codes
- Dimension parsing returns descriptive string error messages on invalid formats or out-of-bound values.

