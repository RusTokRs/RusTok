# rustok-forms

Framework-agnostic form state management, validation rule engine, and field descriptor contracts for the RusToK platform.

## Overview

`rustok-forms` is a pure **Framework-Free Core (FFA)** crate designed with zero DOM, zero UI framework dependencies, and zero RusToK-specific internal dependencies. It depends only on `serde` for serialization.

It acts as the single canonical source of truth for:
- Form submission lifecycle (`FormState`, `FormSubmissionStatus`)
- Validation result contracts (`FieldError`, `ValidationIssue`)
- Declarative validation rules and builder (`FormValidator`, `validation_rules`)
- Field schema descriptors and constraints (`FieldDescriptor`, `FieldKind`, `FieldConstraints`, `FieldOption`)
- Dirty field tracking (`DirtyTracker`)

## Modules & Types

### 1. `state` — Form Lifecycle
- `FormState`: Owns `is_submitting: bool`, `form_error: Option<String>`, `field_errors: Vec<FieldError>`.
- `FormSubmissionStatus`: `Idle`, `Submitting`, `Success`, `Failure(String)`.
- Methods: `idle()`, `submitting()`, `with_form_error()`, `with_field_errors()`, `set_submitting()`, `set_submitted_success()`, `set_submitted_failure()`, `set_field_errors()`, `clear_field_error()`, `clear_errors()`, `reset()`, `is_valid()`, `is_success()`, `is_field_invalid()`, `field_error()`, `field_errors_for()`.

### 2. `validation` — Declarative Validation Engine
- Standard rules:
  - `required`: Non-empty, non-whitespace string check.
  - `min_length` / `max_length`: Unicode-safe character counting (UTF-8 multi-byte friendly).
  - `email`: Comprehensive syntax validation (local part, domain, dot presence, no whitespace, no consecutive dots).
  - `url`: Valid `http://` or `https://` protocol and host syntax.
  - `slug`: Kebab-case ASCII slug (`[a-z0-9-]`, no consecutive or boundary hyphens).
  - `matches`: Value equality check (e.g. password confirmation).
  - `range`: Numeric bounds `[min, max]` inclusive.
  - `custom` / `validate`: Custom predicate or closure condition.
- `FormValidator`: Fluent accumulator builder returning `Result<(), Vec<FieldError>>`.

### 3. `field` — Field Metadata & Schema
- `FieldKind`: Enum covering `Text`, `Email`, `Password`, `Number`, `Url`, `Tel`, `Textarea`, `Select`, `Checkbox`, `Switch`, `Radio`, `File`, `Date`, `DateTime`, `Time`, `Color`, `Hidden`, `RichText`, `Custom`.
- `FieldDescriptor`: Builder pattern configuring name, kind, label, placeholder, constraints, options, disabled state, default values.
- `FieldConstraints`: Required, min/max length, numeric range, pattern, accept, multiple.
- `FieldOption`: Selectable options for dropdowns, radios, and checkbox groups.

### 4. `dirty` — Modification Tracking
- `DirtyTracker`: Set-based tracker recording modified fields since last reset.
- Methods: `mark()`, `unmark()`, `mark_clean()`, `record_change()`, `is_dirty()`, `is_any_dirty()`, `count()`, `dirty_fields()`, `reset()`.

## Adapters

- [`rustok-forms-leptos`](./leptos): Leptos 0.8 SSR-first component adapter with WAI-ARIA and RusToK styling tokens.
