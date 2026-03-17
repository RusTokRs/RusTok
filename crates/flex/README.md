# flex

`flex` contains shared Flex attached-mode contracts extracted from `apps/server`.

## Purpose

- Provide transport-agnostic registry contracts for Flex field definitions.
- Keep module-to-module dependencies clean while attached-mode is still hosted by server adapters.

## Responsibilities

- `FieldDefinitionService` trait.
- `FieldDefRegistry` runtime registry.
- Command/view DTOs for field-definition CRUD orchestration.

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
