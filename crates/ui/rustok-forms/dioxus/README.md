# rustok-forms-dioxus

Dioxus 0.6 adapter for the [`rustok-forms`](../) core library.

## Overview

`rustok-forms-dioxus` provides structural form layout components, accessible input controls, lifecycle-aware buttons, and reactive context coordination designed for Dioxus 0.6 and styled using RusToK design tokens.

Features:
- **Full Parity with Leptos Adapter**: Mirrors the same API, structural components, and styling conventions as `rustok-forms-leptos`.
- **Accessibility (WAI-ARIA)**: Full `for`/`id` linking between `FormLabel` and controls, `aria-invalid` signaling, `aria-describedby` and `aria-errormessage` pointing to `{field}-message` / `{field}-description`, `aria-busy` on submit buttons, and `role="alert"` on errors.
- **Double-Submit Guard**: `<Form>` automatically suppresses subsequent submit attempts while in the `submitting` state.
- **Dirty Tracking Integration**: `<Form [dirty_tracker=...]>` coordinates with `DirtyTracker`, automatically marking fields dirty on user input and clearing on reset.
- **Layout Shift Prevention**: `<SubmitButton>` displays a spinner without collapsing button dimensions or hiding content when submitting text is omitted.
- **Rich Input Controls**: Includes `FormInput`, `FormPasswordInput`, `FormNumberInput`, `FormSearchInput`, `FormColorInput`, `FormRangeInput`, `FormOtpInput`, `FormHiddenInput`, `FormTextarea`, `FormSelect`, `FormCheckbox`, `FormSwitch`, `FormRadioGroup`, and `FormFileInput`.

## Components

### Structural Layout & Feedback
- `Form { state, on_submit, [dirty_tracker], [auto_submitting=true], [on_reset], [method], [action], [id], [class] }`: Top-level form providing reactive `FormContext`.
- `FormField { name, [id], [class] }`: Scope provider injecting `FieldContext` for its children.
- `FormLabel { [html_for], [required=true], [class] }`: Label that automatically turns `text-destructive` when the field is invalid and displays a red `*` with `aria-hidden="true"` when required.
- `FormMessage { [message], [show_all=false], [class] }`: Reactive error message with `role="alert"` and ID linked to the input's `aria-describedby` / `aria-errormessage`.
- `FormDescription { [class] }`: Subdued help text below input controls (alias: `FormHelperText`).
- `FormError { [title], [message], [on_dismiss], [class] }`: Banner displaying form-level (non-field) errors with `role="alert"`.

### Input Controls
- `FormInput { value, [on_input], [input_type="text"|"email"|"search"|...], [required=true], [pattern], [min], [max] }`: Text input with error ring and ARIA attributes.
- `FormPasswordInput { value, [on_input] }`: Dedicated password input with interactive show/hide visibility toggle.
- `FormNumberInput { value, [min], [max], [step] }`: Specialized numeric input.
- `FormSearchInput { value, [on_clear] }`: Search input with search icon and clear button.
- `FormColorInput { value, [on_change], [label] }`: Color picker input with swatch preview.
- `FormRangeInput { value, [min=0.0], [max=100.0], [step=1.0], [show_value=true] }`: Range slider with live numerical badge.
- `FormOtpInput { value, [length=6], [on_complete] }`: Split-cell 2FA / OTP verification code input.
- `FormHiddenInput { value, name }`: Hidden input for IDs, tokens, or fixed metadata.
- `FormTextarea { value, [on_input], [rows=3], [cols] }`: Multi-line text input.
- `FormSelect { value, options, [placeholder] }`: Dropdown select with `FieldOption` support.
- `FormCheckbox { checked, [on_change], [label], [description] }`: Boolean checkbox with invalid state indicator.
- `FormSwitch { checked, [on_change], [label], [description] }`: iOS-style animated toggle switch with ARIA switch semantics.
- `FormRadioGroup { value, options, [horizontal=false] }`: Accessible radio option group (`role="radiogroup"`).
- `FormFileInput { [accept], [multiple=true] }`: Browser-safe file input.

### Actions
- `SubmitButton { [variant=ButtonVariant::Primary], [size=ButtonSize::Md], [full_width=false], [submitting_text="Saving..."] }`: Submit button with loading spinner, `aria-busy`, and double-submit disable state.
- `ResetButton { [variant=ButtonVariant::Outline], [size=ButtonSize::Md], [full_width=false], [on_reset] }`: Form reset button.

## Usage Example

```rust,ignore
use dioxus::prelude::*;
use rustok_forms::{DirtyTracker, FormState, FormValidator};
use rustok_forms_dioxus::*;

#[component]
pub fn PostEditor() -> Element {
    let mut form_state = use_signal(FormState::idle);
    let dirty_tracker = use_signal(DirtyTracker::new);
    let mut title = use_signal(String::new);
    let mut slug = use_signal(String::new);

    let on_submit = move |_| {
        let t = title.read().clone();
        let s = slug.read().clone();

        let val_result = FormValidator::new()
            .required("title", &t, "Title is required")
            .slug("slug", &s, "Slug must be valid kebab-case")
            .finish();

        match val_result {
            Ok(_) => {
                // Execute async API mutation...
                form_state.write().set_submitted_success();
            }
            Err(errs) => {
                form_state.write().set_field_errors(errs);
            }
        }
    };

    rsx! {
        Form {
            state: form_state,
            dirty_tracker: dirty_tracker,
            on_submit: on_submit,
            class: "space-y-4 max-w-md",

            FormError {}

            FormField {
                name: "title",
                FormLabel { required: true, "Title" }
                FormInput {
                    value: title.read().clone(),
                    on_input: move |v| title.set(v),
                    placeholder: "Enter post title",
                }
                FormDescription { "The public title of your article." }
                FormMessage {}
            }

            FormField {
                name: "slug",
                FormLabel { required: true, "Slug" }
                FormInput {
                    value: slug.read().clone(),
                    on_input: move |v| slug.set(v),
                    placeholder: "e.g. my-first-post",
                }
                FormMessage {}
            }

            div {
                class: "flex items-center gap-3",
                SubmitButton {
                    submitting_text: "Publishing...",
                    "Publish"
                }
                ResetButton {
                    "Cancel"
                }
            }
        }
    }
}
```
