# rustok-forms-dioxus / CRATE_API

## Purpose
`rustok-forms-dioxus` provides Dioxus 0.7 reactive component adapters for the framework-agnostic `rustok-ui-forms` library. It exposes structural form layouts, accessible input controls, lifecycle-aware submit buttons, and reactive context coordination with automated WAI-ARIA wiring.

## Architecture & Responsibilities
- **Reactive Context Coordination**: `Form` provides `FormContext` down the Dioxus component tree, while `FormField` scopes individual `FieldContext` instances for nested controls and error banners.
- **WAI-ARIA Accessibility**: Automatically computes and links `for`/`id`, `aria-invalid`, `aria-describedby`, and `aria-errormessage` IDs between labels, controls, descriptions, and error messages.
- **Submission Lifecycle & Double-Submit Guard**: `SubmitButton` observes the reactive form state, rendering loading spinners and preventing duplicate submissions while the form is in `Submitting` status.
- **Rich Input Primitives**: Provides accessible, Tailwind-styled Dioxus components for all standard form input kinds (Text, Password with reveal toggle, Number, Search, Color, Range, Split OTP/2FA, Textarea, Select, Checkbox, Switch, RadioGroup, File input).

## Public Surface
### Primary Components
- `Form`: Top-level form component providing reactive `FormContext`, double-submit guard, and dirty tracker coordination.
- `FormField`: Scoped field container injecting `FieldContext` for children.
- `FormLabel`: Accessible label linked to input controls with required asterisk indicator.
- `FormMessage`: Error message linked to `aria-describedby` / `aria-errormessage`.
- `FormDescription` / `FormHelperText`: Accessible help text linked via ARIA.
- `FormError`: Form-level error banner with alert semantics.
- `FormInput`: General text/email/search input control.
- `FormPasswordInput`: Password input with show/hide toggle.
- `FormNumberInput`: Specialized numeric input control.
- `FormSearchInput`: Search input with clear trigger.
- `FormColorInput`: Color picker with swatch indicator.
- `FormRangeInput`: Range slider with live numerical badge.
- `FormOtpInput`: Split-cell 2FA / OTP verification code input.
- `FormHiddenInput`: Hidden input control.
- `FormTextarea`: Multi-line text input control.
- `FormSelect`: Accessible dropdown select control.
- `FormCheckbox`: Accessible checkbox control.
- `FormSwitch`: Accessible toggle switch control.
- `FormRadioGroup`: Accessible radio group control.
- `FormFileInput`: Browser-safe file input control.
- `SubmitButton`: Lifecycle-aware submit button with spinner and double-submit guard.
- `ResetButton`: Reset button clearing errors and dirty states.

### Public Types & Contexts
- `FormContext`: Reactive context managing form lifecycle signals, dirty state, and field errors.
- `FieldContext`: Reactive context scoping a single field's ID, name, error, and ARIA descriptors.
- `ButtonVariant`: Button style presets (`Primary`, `Secondary`, `Destructive`, `Outline`, `Ghost`).
- `ButtonSize`: Button sizing scale (`Sm`, `Md`, `Lg`).

## Invariants & Guarantees
- **Framework Parity**: Maintained with 1:1 structural and component parity against `rustok-forms-leptos`.
- **WAI-ARIA Accessibility**: Automated and complete `for`/`id` linking, `aria-invalid`, `aria-describedby`, and `aria-errormessage` bindings.
- **Double-Submit Prevention**: Submission triggers are disabled whenever the underlying `FormState` reports `is_submitting`.
- **Boundary Isolation**: Depends purely on `rustok-ui-forms` core models and `dioxus`; never accesses backend storage or private tables directly.

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `rustok-ui-forms`: Framework-agnostic form state, validation, field contracts, and dirty tracking core.
  - `dioxus`: Dioxus 0.7 reactive framework primitives, RSX macro, and component hooks.
  - `serde`: Serialization traits for component props and options.
- **Zero Host Coupling**: Can be used in any Dioxus 0.7 web, desktop, or mobile application.
