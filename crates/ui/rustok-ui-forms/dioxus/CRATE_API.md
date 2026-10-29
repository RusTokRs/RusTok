# rustok-forms-dioxus / CRATE_API

Dioxus 0.7 adapter for the `rustok-forms` core library.

## Public surface

### Primary Public Components
- `Form`: Top-level form component providing reactive `FormContext`, double-submit guard, and dirty tracker coordination.
- `FormField`: Scoped field container injecting `FieldContext` for children.
- `FormLabel`: Accessible label linked to input controls with required indicator.
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

### Public Modules & Contexts
- `components`: Form layout and control RSX components.
- `context`: Reactive `FormContext` and `FieldContext` signal bindings.

## Invariants

1. **Framework Parity**: Provides 1:1 structural and component parity with `rustok-forms-leptos`.
2. **WAI-ARIA Accessibility**: Complete automatic `for`/`id` linking, `aria-invalid`, `aria-describedby`, and `aria-errormessage` bindings.
3. **Double-Submit Prevention**: Disables submission triggers while in `Submitting` state.
4. **Boundary Isolation**: Depends purely on `rustok-forms` core models and `dioxus`; does not access backend databases or private module tables.
