# flex

`flex` contains shared Flex attached-mode contracts extracted from `apps/server`.

## Purpose

- Provide transport-agnostic registry contracts for Flex field definitions.
- Keep module-to-module dependencies clean while attached-mode is still hosted by server adapters.

## Responsibilities

- `FieldDefinitionService` trait.
- `FieldDefRegistry` runtime registry.
- Command/view DTOs for field-definition CRUD orchestration.

## Multilingual status

The current Flex multilingual contract is already partially live and must be treated as canonical by contributors and agents:

- `FieldDefinition` now carries explicit `is_localized` semantics in `rustok-core`, registry DTOs, GraphQL inputs, and attached-mode persistence.
- Attached-mode registered consumers are `user`, `product`, `order`, and `topic`. `node` is not part of the live attached contract yet.
- Standalone schema UI copy (`name`, `description`) no longer belongs in `flex_schemas`; it is stored in `flex_schema_translations`.
- Standalone entry payloads no longer treat inline locale-aware JSON as the canonical path: shared values stay in `flex_entries.data`, while locale-aware values now live in `flex_entry_localized_values`.
- Generic attached localized value storage now lives in the shared `flex` crate and persists into `flex_attached_localized_values`; live donor read/write paths now exist for `user`, `product`, `order`, and `topic`.
- `topic` is no longer schema-only: forum topics now use `forum_topics.metadata` as the donor payload, and locale-aware Flex keys are resolved through the same attached multilingual contract as the other live donors. Any locale-aware JSON payloads that still live inline in donor metadata or entry data remain transitional fallback, not the target storage standard.

Do not implement new Flex multilingual behavior from older plans that assume inline localized copy in base rows or treat JSON blobs as the canonical multilingual storage path.

## Interactions

- Depends on `rustok-core` (`FlexError`, `FieldType`, `ValidationRule`).
- Depends on `rustok-events` (`EventEnvelope`).
- Consumed by `apps/server` GraphQL and bootstrap wiring.

## Entry points

- `flex::FieldDefRegistry`
- `flex::FieldDefinitionService`
- `flex::{CreateFieldDefinitionCommand, UpdateFieldDefinitionCommand, FieldDefinitionView}`

## Docs

- Module documentation: [`docs/README.md`](./docs/README.md)
- Implementation plan: [`docs/implementation-plan.md`](./docs/implementation-plan.md)
