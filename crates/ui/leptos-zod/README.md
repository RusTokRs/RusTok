# leptos-zod

## Purpose

`leptos-zod` owns validation issue DTOs for Leptos-facing RusToK form and API flows.

## Responsibilities

- Represent structured validation issues in a client-friendly shape.
- Provide a lightweight validation error envelope for UI consumption.
- Keep generic validation payload contracts reusable across Leptos packages.

## Entry points

- `ZodIssue`
- `ZodError`

## Interactions

- Used by Leptos forms and validation-aware UI packages.
- Works with helpers such as `leptos-hook-form` when mapping structured issues to field errors.
- Stays transport-friendly and independent from domain-specific validation schemas.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
