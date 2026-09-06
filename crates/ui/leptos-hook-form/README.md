# leptos-hook-form

## Purpose

`leptos-hook-form` owns lightweight Leptos form-state helpers for submission, field errors, and validation issue mapping.

## Responsibilities

- Represent form submission state and form-level errors.
- Represent field-level validation errors in a UI-friendly shape.
- Convert structured validation issues into form field errors.

## Entry points

- `FormState`
- `FieldError`
- `ValidationIssue`
- `issues_to_field_errors`

## Interactions

- Used by Leptos applications and UI packages that need a small form-state contract.
- Works well with `leptos-zod` issue payloads and other validation adapters.
- Stays independent from domain-specific form schemas and transport clients.

## Docs

- [Module docs](./docs/README.md)
- [Implementation plan](./docs/implementation-plan.md)
- [Platform docs index](../../docs/index.md)
