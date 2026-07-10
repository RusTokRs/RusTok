# `leptos-hook-form` Documentation

`leptos-hook-form` provides generic, serializable Leptos form-state helpers.
It does not own schemas, route state, data fetching, or domain validation.

## Public contract

- `FormState` represents idle/submitting state plus form and field errors.
- `FieldError` provides a field/message error shape.
- `ValidationIssue` carries a path/message validation result.
- `issues_to_field_errors` maps structured paths into dot-separated field names.

## Boundary

Host and module UI packages own form schema, validation adapter selection,
transport submission, and domain-specific error policy. Add behavior here only
when it is shared across independent form surfaces.

## Related documents

- [Crate README](../README.md)
- [Implementation plan](./implementation-plan.md)
