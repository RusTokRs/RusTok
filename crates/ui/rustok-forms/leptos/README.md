# rustok-forms-leptos

Leptos 0.8 SSR-first adapter for the [`rustok-forms`](../) core library.

## Overview

`rustok-forms-leptos` provides structural form layout components, accessible input controls, lifecycle-aware buttons, and reactive context coordination designed for Leptos 0.8 and styled using RusToK design tokens.

Features:
- **SSR-First**: Server-side rendering generates correct HTML attributes (`selected`, `checked`, `novalidate`, `aria-*`, `method`, `action`) without hydration mismatches.
- **Accessibility (WAI-ARIA)**: Full `for`/`id` linking between `FormLabel` and controls, `aria-invalid` signaling, `aria-describedby` and `aria-errormessage` pointing to `{field}-message` / `{field}-description`, `aria-busy` on submit buttons, and `role="alert"` on errors.
- **Double-Submit Guard**: `<Form>` automatically suppresses subsequent submit attempts while in the `submitting` state.
- **Dirty Tracking Integration**: `<Form [dirty_tracker=...]>` coordinates with `DirtyTracker`, automatically marking fields dirty on user input and clearing on reset.
- **Layout Shift Prevention**: `<SubmitButton>` displays a spinner without collapsing button dimensions or hiding content when submitting text is omitted.
- **Browser-Safe File Uploads**: `<FormFileInput>` prevents `InvalidStateError` DOM exceptions by avoiding JavaScript `prop:value` bindings on file inputs.
- **Rich Input Controls**: Includes `FormInput`, `FormPasswordInput`, `FormNumberInput`, `FormSearchInput`, `FormHiddenInput`, `FormTextarea`, `FormSelect`, `FormCheckbox`, `FormSwitch`, `FormRadioGroup`, and `FormFileInput`.

## Components

### Structural Layout & Feedback
- `<Form state=... on_submit=... [dirty_tracker=...] [auto_submitting=true] [on_reset=...] [method=...] [action=...] [id=...] [class=...]>`: Top-level form providing reactive `FormContext`.
- `<FormField name="fieldName" [id=...] [class=...]>`: Scope provider injecting `FieldContext` for its children.
- `<FormLabel [html_for=...] [required=true] [class=...]>`: Label that automatically turns `text-destructive` when the field is invalid and displays a red `*` with `aria-hidden="true"` when required.
- `<FormMessage [message=...] [show_all=false] [class=...]>`: Reactive error message with `role="alert"` and ID linked to the input's `aria-describedby` / `aria-errormessage`.
- `<FormDescription [class=...]>`: Subdued help text below input controls (alias: `FormHelperText`).
- `<FormError [title=...] [message=...] [on_dismiss=...] [class=...]>`: Banner displaying form-level (non-field) errors with `role="alert"`.

### Input Controls
- `<FormInput value=... [on_input=...] [input_type="text"|"email"|"search"|...] [required=true] [pattern=...] [min=...] [max=...]>`: Text input with error ring and ARIA attributes.
- `<FormPasswordInput value=... [on_input=...]>`: Dedicated password input with interactive show/hide visibility toggle.
- `<FormNumberInput value=... [min=...] [max=...] [step=...]>`: Specialized numeric input.
- `<FormSearchInput value=... [on_clear=...]>`: Search input with search icon and clear button.
- `<FormHiddenInput value=... name=...>`: Hidden input for IDs, tokens, or fixed metadata.
- `<FormTextarea value=... [on_input=...] [rows=3] [cols=...]>`: Multi-line text input.
- `<FormSelect value=... options=... [placeholder=...] [multiple=false]>`: Dropdown select with SSR `selected` attribute support.
- `<FormCheckbox checked=... [on_change=...] [label=...] [description=...]>`: Boolean checkbox with invalid state indicator.
- `<FormSwitch checked=... [on_change=...] [label=...] [description=...]>`: iOS-style animated toggle switch with ARIA switch semantics.
- `<FormRadioGroup value=... options=... [horizontal=false]>`: Accessible radio option group (`role="radiogroup"`).
- `<FormFileInput [accept=...] [multiple=true] [on_change=...]>`: Browser-safe file input.

### Actions
- `<SubmitButton [variant=ButtonVariant::Primary] [size=ButtonSize::Md] [full_width=false] [submitting_text="Saving..."]>`: Submit button with loading spinner, `aria-busy`, and double-submit disable state.
- `<ResetButton [variant=ButtonVariant::Outline] [size=ButtonSize::Md] [full_width=false] [on_reset=...]>`: Form reset button.

## Usage Example

```rust,ignore
use leptos::prelude::*;
use rustok_forms::{DirtyTracker, FormState, FormValidator};
use rustok_forms_leptos::*;

#[component]
pub fn PostEditor() -> impl IntoView {
    let form_state = RwSignal::new(FormState::idle());
    let dirty_tracker = RwSignal::new(DirtyTracker::new());
    let (title, set_title) = signal(String::new());
    let (slug, set_slug) = signal(String::new());

    let on_submit = move |_| {
        let t = title.get();
        let s = slug.get();

        let val_result = FormValidator::new()
            .required("title", &t, "Title is required")
            .slug("slug", &s, "Slug must be valid kebab-case")
            .finish();

        match val_result {
            Ok(_) => {
                // Execute async API mutation...
                form_state.update(|state| state.set_submitted_success());
            }
            Err(errs) => {
                form_state.update(|state| state.set_field_errors(errs));
            }
        }
    };

    view! {
        <Form state=form_state dirty_tracker=dirty_tracker on_submit=on_submit class="space-y-4 max-w-md">
            <FormError />

            <FormField name="title">
                <FormLabel required=true>"Title"</FormLabel>
                <FormInput value=title on_input=set_title placeholder="Enter post title" />
                <FormDescription>"The public title of your article."</FormDescription>
                <FormMessage />
            </FormField>

            <FormField name="slug">
                <FormLabel required=true>"Slug"</FormLabel>
                <FormInput value=slug on_input=set_slug placeholder="e.g. my-first-post" />
                <FormMessage />
            </FormField>

            <div class="flex items-center gap-3">
                <SubmitButton submitting_text="Publishing...">"Publish"</SubmitButton>
                <ResetButton>"Cancel"</ResetButton>
            </div>
        </Form>
    }
}
```
