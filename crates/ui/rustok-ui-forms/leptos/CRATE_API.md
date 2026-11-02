# rustok-forms-leptos / CRATE_API

## Purpose
`rustok-forms-leptos` provides Leptos 0.8 SSR-first reactive component adapters for the framework-agnostic `rustok-ui-forms` library. It exposes structural form layouts, accessible input controls, lifecycle-aware submit buttons, and reactive context coordination with automated WAI-ARIA wiring.

## Architecture & Responsibilities
- **Reactive Context Coordination**: `<Form>` injects `FormContext` down the Leptos reactive component tree, and `<FormField>` scopes individual `FieldContext` instances for nested controls, labels, and error messages.
- **SSR & Hydration Parity**: Server-side rendering emits exact HTML attributes without hydration mismatches or layout shifts.
- **WAI-ARIA Accessibility**: Automatically computes and links `for`/`id`, `aria-invalid`, `aria-describedby`, and `aria-errormessage` IDs between labels, controls, descriptions, and error messages.
- **Submission Lifecycle & Double-Submit Guard**: `<SubmitButton>` observes the reactive form state, rendering loading spinners and preventing duplicate submissions while the form is in `Submitting` status.
- **Rich Input Primitives**: Provides accessible, Tailwind-styled Leptos components for all standard form input kinds (Text, Password with reveal toggle, Number, Search, Color, Range, Split OTP/2FA, Textarea, Select, Checkbox, Switch, RadioGroup, File input).

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
- **Hydration and SSR Parity**: Emits valid, compliant SSR HTML matching client hydration output without divergence.
- **Framework Parity**: Maintained with 1:1 structural and component parity against `rustok-forms-dioxus`.
- **WAI-ARIA Accessibility**: Automated and complete `for`/`id` linking, `aria-invalid`, `aria-describedby`, and `aria-errormessage` bindings.
- **Double-Submit Prevention**: Submission triggers are disabled whenever the underlying `FormState` reports `is_submitting`.
- **Boundary Isolation**: Depends purely on `rustok-ui-forms` core models and `leptos`; never accesses backend storage or private tables directly.

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `rustok-ui-forms`: Framework-agnostic form state, validation, field contracts, and dirty tracking core.
  - `leptos`: Leptos 0.8 reactive framework primitives, view macro, signals, and SSR/hydration support.
- **Zero Host Coupling**: Can be used in any Leptos 0.8 SSR, CSR, or hybrid application.
