# rustok-ai-translation / CRATE_API

## Public surface

### Constants & Identifiers
- `MACHINE_TRANSLATION_TASK_SLUG`: task identifier for AI translation jobs.
- `MACHINE_TRANSLATION_PROVIDER_SLUG`: provider identifier (`rustok_ai`).
- `MACHINE_TRANSLATION_PROMPT_POLICY`: policy identifier for proposal-only translations.
- `MACHINE_TRANSLATION_SYSTEM_PROMPT`: immutable prompt enforcing data/instruction boundary.

### Providers & Adapters
- `AiMachineTranslationAdapter`: adapter bridging `MachineTranslationPort` to `AiStructuredTaskPort`.
- `AiMachineTranslationPortFactory`: factory instantiating translation adapters from runtime context.
- `machine_translation_port_from_context`: constructor resolving port from context.

### Descriptors & Digests
- `machine_translation_task_descriptor`: AI structured task registration descriptor.
- `machine_translation_descriptor`: provider descriptor exposing supported languages and capability tags.
- `machine_translation_policy_digest`: digest hashing prompt policy rules.
- `machine_translation_input_schema_digest`: digest hashing input JSON schema.
- `machine_translation_output_schema_digest`: digest hashing output JSON schema.

## Platform Capability Bindings
- **Machine Translation Provider**: Implements `MachineTranslationPort` provider SPI using `AiStructuredTaskPort`.
- **Stateless Bridge Architecture**: Stateless execution mapping with proposal-only review requirements.
- **Protected Token Preservation**: Enforces exact match of protected tokens and required whitespace shapes.

## Domain Collaborations
- **`rustok-translation`**: Machine translation domain port and provider interfaces.
- **`rustok-ai`**: Structured task execution runtime, limits, and budget tracking.
- **`rustok-translation-targets`**: Token ledgers, whitespace shape validation, and value profiles.

## Minimum Contract Set

### Input DTOs/Commands
- `MachineTranslationBatchRequest`: batch of translation units with source and target locales.
- `MachineTranslationEstimate`: non-billable estimate calculation request.
- `AiStructuredTaskRequest`: structured task payload dispatched to AI backend.

### Domain Invariants
- **Non-Swallowed Errors**: Translation, budget, or validation failures return typed domain errors.
- **Proposal-Only Safety**: Translations are marked proposal-only and require operator review before publication.
- **Byte-for-Byte Preservation**: Protected tokens, token multiplicities, and whitespace formatting must match exactly.

### Events / Outbox Side Effects
- Dispatches structured tasks and records token usage and attempt evidence via AI task infrastructure.

### Errors / Failure Codes
- `PortError`: typed domain errors across translation execution and status queries.

