# RusToK UI Forms

`rustok-ui-forms` owns framework-neutral form submission and validation-result
state for RusToK UI adapters.

## Responsibilities

- `FormState` and `FormSubmissionStatus` lifecycle values;
- `FieldError` and `ValidationIssue` transport-friendly contracts;
- deterministic validation-issue to field-error mapping.

## Boundary

Domain modules own validation rules. Hosts own effective locale and translated
copy. Leptos, Dioxus, React, and mobile adapters own signals, hooks, event
binding, focus behavior, and rendering.

Do not add framework runtimes, DOM types, hard-coded validation copy, transport
clients, or domain schemas to this crate.

## Entry point

- `src/lib.rs` — framework-neutral form state and validation-result mapping.
