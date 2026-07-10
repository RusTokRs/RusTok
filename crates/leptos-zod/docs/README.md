# `leptos-zod` Documentation

`leptos-zod` provides generic, serializable validation issue DTOs for
Leptos-facing flows. It does not execute schemas or interpret domain policy.

## Public contract

- `ZodIssue` holds a nested field path and message.
- `ZodError` wraps an issue list and exposes `is_empty`.
- `ZodError::from_api` constructs an envelope from an API-compatible issue list.

## Boundary

Hosts, validation adapters, and domain modules own schema execution,
localization, field labels, transport mapping, and validation policy. Add
behavior here only when it is reusable issue-envelope behavior.

## Related documents

- [Crate README](../README.md)
- [Implementation plan](./implementation-plan.md)
