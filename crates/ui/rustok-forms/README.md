# rustok-forms

Framework-agnostic form state management, validation rule engine, and field descriptor contracts for the RusToK platform.

## Overview

`rustok-forms` is a pure **Framework-Free Core (FFA)** crate designed with zero DOM, zero UI framework dependencies, and zero RusToK-specific internal dependencies. It depends only on `serde` for serialization.

It acts as the single canonical source of truth for:
- Form submission lifecycle (`FormState`, `FormSubmissionStatus`)
- Validation result contracts (`FieldError`, `ValidationIssue`)
- Path manipulation contracts (`parse_field_path`, `format_field_path`)
- Declarative validation rules and accumulator (`FormValidator`, `validation_rules`)
- Field schema descriptors and constraints (`FieldDescriptor`, `FieldKind`, `FieldConstraints`, `FieldOption`)
- Dirty field tracking (`DirtyTracker`)

## Modules & Types

### 1. `state` — Form Lifecycle
- `FormState`: Owns `is_submitting: bool`, `is_success: bool`, `form_error: Option<String>`, `field_errors: Vec<FieldError>`.
- `FormSubmissionStatus`: `Idle`, `Submitting`, `Success`, `Failure(String)`.
- Constructors: `idle()`, `submitting()`, `success()`, `with_form_error()`, `with_field_errors()`.
- Transitions: `set_submitting()`, `set_submitted_success()`, `set_submitted_failure()`, `set_field_errors()`, `set_field_error()`, `add_field_error()`, `add_field_errors()`.
- Queries: `submission_status()`, `is_valid()`, `is_success()`, `is_field_invalid()`, `field_error()`, `field_errors_for()`, `has_errors()`, `has_form_error()`, `has_field_errors()`, `error_count()`, `field_error_count()`, `first_error()`, `all_errors()`.
- Mutators: `clear_field_error()`, `clear_form_error()`, `clear_errors()`, `reset()`, `retain_field_errors()`.

### 2. `error` — Validation Error Contracts
- `FieldError`: Represents a validation error for a field (`field: String`, `message: String`), implements `Display`, `std::error::Error`, and `Hash`.
- `ValidationIssue`: Structured validation issue with path segments (`path: Vec<String>`, `message: String`), implements `Display`, `std::error::Error`, and `Hash`.
- Path helpers: `parse_field_path` (supporting both dot `a.b.c` and bracket `items[0].name` notation), `format_field_path`.
- Helper functions: `issues_to_field_errors`, `field_errors_to_issues`.

### 3. `validation` — Declarative Validation Engine
- Standard validation rules (`validation_rules` / `rules`):
  - `required`: Non-empty, non-whitespace string check.
  - `min_length` / `max_length` / `length_between`: Unicode-safe character counting (UTF-8 multi-byte friendly).
  - `min_length_bytes` / `max_length_bytes`: Byte length constraints.
  - `email`: RFC-compliant syntax validation (domain labels, local-part dot rules, no consecutive dots).
  - `url`: Valid `http://` or `https://` protocol and host syntax.
  - `slug`: Kebab-case ASCII slug (`[a-z0-9-]`, no consecutive or boundary hyphens).
  - `matches`: Value equality check (e.g. password confirmation).
  - `range`: Generic numeric bounds `[min, max]` inclusive for `PartialOrd + Copy`.
  - `numeric`: Checks if string parses to a finite number (`f64`).
  - `integer`: Checks if string parses to a 64-bit integer (`i64`).
  - `range_numeric`: Validates that a string parses to a number within `[min, max]`.
  - `alphanumeric`: Checks if string is purely alphanumeric (unicode-safe).
  - `alphabetic`: Checks if string is purely alphabetic (unicode-safe).
  - `tel`: Phone number format validation.
  - `one_of` / `none_of`: In-list / disallowed-list validation.
  - `uuid`: Standard 8-4-4-4-12 hex UUID format check.
  - `date_ymd`: Calendar date `YYYY-MM-DD` syntax check.
  - `time_hhmm`: 24-hour time `HH:MM` / `HH:MM:SS` syntax check.
  - `ipv4`: IPv4 decimal address check.
  - `hex_color`: Hex color `#RGB`, `#RRGGBB`, `#RRGGBBAA`.
  - `json`: Valid JSON string check.
  - `contains`, `starts_with`, `ends_with`: Substring predicates.
  - `min_items` / `max_items`: Collection size validation.
  - `custom` / `validate`: Custom predicate or closure condition.
- `FormValidator`: Fluent accumulator builder with conditional evaluation (`validate_if`), nested sub-form prefixing (`merge_nested`), error merging (`merge`), and `finish()` returning `Result<(), Vec<FieldError>>`.

### 4. `field` — Field Metadata & Schema
- `FieldKind`: Enum covering `Text`, `Email`, `Password`, `Number`, `Url`, `Tel`, `Search`, `Date`, `DateTime`, `Time`, `Month`, `Week`, `Range`, `Color`, `Textarea`, `Select`, `Checkbox`, `Switch`, `Radio`, `File`, `Hidden`, `RichText`, `Custom`.
- `FieldDescriptor`: Builder pattern configuring name, kind, label, placeholder, constraints, options, disabled state, default values, with `.validate(value)` schema execution.
- `FieldConstraints`: Builder pattern configuring required, min/max length, numeric range, step, pattern, accept, multiple, with `.validate(field, value)` execution.
- `FieldOption`: Selectable options for dropdowns, radios, and checkbox groups with disabled state and tuple conversion.

### 5. `dirty` — Modification Tracking
- `DirtyTracker`: Set-based tracker recording modified fields since last reset.
- Methods: `mark()`, `mark_many()`, `unmark()`, `unmark_many()`, `mark_clean()`, `record_change()`, `is_dirty()`, `contains()`, `is_any_of_dirty()`, `are_all_dirty()`, `is_any_dirty()`, `is_empty()`, `count()`, `len()`, `dirty_fields()`, `iter()`, `retain()`, `reset()`, `clear()`.
- Implements `Extend`, `FromIterator`, `IntoIterator`, `Hash`.

## Adapters

- [`rustok-forms-leptos`](./leptos): Leptos 0.8 SSR-first component adapter with WAI-ARIA and RusToK styling tokens.
