# rustok-ui-forms / CRATE_API

## Purpose
`rustok-ui-forms` is RusToK's framework-agnostic form state management, validation engine, field descriptor schema specification, dirty tracking, and sanitization core under Fluid Frontend Architecture (FFA). It contains zero DOM, zero UI framework dependencies, and provides the pure Rust foundation for companion framework adapters (`rustok-forms-leptos`, `rustok-forms-dioxus`).

## Architecture & Responsibilities
- **Lifecycle & State Machine (`FormState`, `FormSubmissionStatus`)**: Tracks form-level submission status (`Idle`, `Submitting`, `Success`, `Failure`), top-level error messages, field-level errors, and provides atomic state transitions.
- **Composable Validation Engine (`FormValidator`, `validation_rules`)**: Declarative builder accumulating validation errors across standard rules (required, length bounds, numeric ranges, email, URL, phone, slug, regex, UUID, date/time) and custom validator closures.
- **Declarative Field Schemas (`FormSchema`, `FieldDescriptor`, `FieldConstraints`, `FieldKind`, `FieldOption`)**: Provides schema-driven form definitions for UI reflection, JSON validation, and dynamic form generation.
- **Dirty Field Tracking (`DirtyTracker`)**: Tracks modified fields against initial values, supporting batch updates, unmarking, and query predicates.
- **Multi-Step Wizards (`FormStepTracker`)**: State tracker for multi-step wizards, step completion, and linear or non-linear navigation guards.
- **Value Sanitization (`form_sanitizer`)**: Pure-function normalization utilities for whitespace trimming, slugification, email normalization, phone number cleaning, tag stripping, and numeric clamping.
- **Path Resolution (`ValidationIssue`, `parse_field_path`, `format_field_path`)**: Dot-notation and array indexing parsing/formatting for nested form errors.

## Public Surface
### Primary Structs & Enums
- `FormState`: Core form lifecycle state container (`is_submitting`, `is_success`, `form_error`, `field_errors`).
- `FormSubmissionStatus`: Enum representing form submission status (`Idle`, `Submitting`, `Success`, `Failure(String)`).
- `FormValidator`: Composable builder for multi-field validation and error accumulation.
- `FieldError`: Field identifier and error message pair.
- `ValidationIssue`: Structured validation error with hierarchical path segments.
- `FieldDescriptor`: Schema descriptor defining UI reflection, kind, label, constraints, and options.
- `FieldKind`: Enum of supported field kinds (`Text`, `Email`, `Password`, `Number`, `Select`, `Checkbox`, `Radio`, `Textarea`, `Date`, etc.).
- `FieldConstraints`: Bounds on length, range, requiredness, and patterns.
- `FieldOption`: Value, label, and disabled state for select/radio controls.
- `FormSchema`: Collection of field descriptors for declarative schema-driven forms.
- `DirtyTracker`: Set-based dirty state tracker.
- `FormStepTracker`: Multi-step form/wizard navigation tracker.

### Public Modules & Utilities
- `form_sanitizer`: Normalization utilities (`trim`, `slugify`, `normalize_email`, `normalize_phone`, `collapse_whitespace`, `clamp_number`, `strip_tags`).
- `field_errors_to_issues` / `issues_to_field_errors`: Conversion between flat field errors and structured path issues.
- `parse_field_path` / `format_field_path`: Conversion between dot-bracket path notation and path segment slices.
- `validation_rules`: Re-export of standard validation helper predicates.

## Invariants & Guarantees
- **Framework-Free Core (FFA)**: Zero DOM, zero UI framework dependencies (no Leptos, Dioxus, or web-sys types). All rules and states are testable in pure Rust without browser or DOM harness.
- **Deterministic Validation**: Validation rules are pure, side-effect free functions; identical inputs always produce identical validation results.
- **Unicode Safety**: String length constraints measure Unicode characters (scalar values/graphemes) rather than raw byte counts.
- **Serialization Parity**: All public descriptors and state types implement `Serialize` and `Deserialize` for seamless transmission across SSR, hydration, and API boundaries.

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `serde`: Serialization traits for state and schema contracts.
  - `serde_json`: JSON value validation and payload parsing.
  - `regex`: Standard regex pattern matching for email, phone, and custom pattern constraints.
- **Zero Internal RusToK Dependencies**: `rustok-ui-forms` depends on no other RusToK internal crate, making it universally reusable across server, client, CLI, and test runners.
