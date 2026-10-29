# rustok-ui-forms / CRATE_API

Framework-agnostic form state management, validation rule engine, value sanitization, and field descriptor contracts for RusToK.

## Public surface

### Primary Public Types
- `FormState`: Core form lifecycle state container (`is_submitting`, `is_success`, `form_error`, `field_errors`).
- `FormSubmissionStatus`: Enum representing form status (`Idle`, `Submitting`, `Success`, `Failure(String)`).
- `FormValidator`: Composable builder for multi-field validation and error accumulation.
- `FieldError`: Single field error with field key and message.
- `ValidationIssue`: Structured validation error with hierarchical path segments.
- `FieldDescriptor`: Schema descriptor defining UI reflection, kind, label, and constraints.
- `FieldKind`: Enum of supported field kinds (`Text`, `Email`, `Password`, `Number`, `Select`, `Checkbox`, etc.).
- `FieldConstraints`: Bounds on length, range, requiredness, and patterns.
- `FormSchema`: Collection of field descriptors for declarative schema-driven forms.
- `DirtyTracker`: Set-based dirty state tracker.
- `FormStepTracker`: Multi-step form/wizard navigation tracker.

### Public Modules
- `state`: Form lifecycle, mutation methods, and state queries.
- `error`: Error representations and field path parser/formatter (`parse_field_path`, `format_field_path`).
- `validation`: Declarative rule builder and standard validators (`validation_rules` / `rules`).
- `schema`: Field descriptors, options, and collective schema definitions.
- `dirty`: Dirty field tracking utilities.
- `wizard`: Multi-step wizard and funnel tracking.
- `sanitizer`: Normalization utilities (`form_sanitizer`).

## Invariants

1. **Framework-Free Core (FFA)**: Zero DOM, zero UI framework dependencies (no Leptos, Dioxus, or web-sys types).
2. **Deterministic Validation**: Validation rules are side-effect free pure functions.
3. **Unicode Safety**: String length constraints measure Unicode scalar values/graphemes correctly where appropriate.
4. **Serialization Parity**: All public descriptors and states implement `Serialize` and `Deserialize` for seamless transport across SSR, hydration, and API boundaries.
