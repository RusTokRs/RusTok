# rustok-forms-leptos

Leptos 0.8 SSR-first adapter for the [`rustok-forms`](../) core library.

## Overview

`rustok-forms-leptos` provides structural form layout components, accessible input controls, lifecycle-aware buttons, and reactive context coordination designed for Leptos 0.8 and styled using RusToK design tokens.

Features:
- **SSR-First**: Server-side rendering generates correct HTML attributes (`selected`, `checked`, `novalidate`, `aria-*`) without hydration mismatches.
- **Accessibility (WAI-ARIA)**: Full `for`/`id` linking between `FormLabel` and controls, `aria-invalid` signaling, `aria-describedby` pointing to `{field}-message` / `{field}-description`, and `role="alert"` on errors.
- **Double-Submit Guard**: `<Form>` automatically suppresses subsequent submit attempts while in the `submitting` state.
- **Layout Shift Prevention**: `<SubmitButton>` displays a spinner without collapsing button dimensions or hiding content when submitting text is omitted.
- **Browser-Safe File Uploads**: `<FormFileInput>` prevents `InvalidStateError` DOM exceptions by avoiding JavaScript `prop:value` bindings on file inputs.

## Components

### Structural Layout & Feedback
- `<Form state=... on_submit=... [id=...] [class=...]>`: Top-level form providing reactive `FormContext`.
- `<FormField name="fieldName" [class=...]>`: Scope provider injecting `FieldContext` for its children.
- `<FormLabel [html_for=...] [required=true] [class=...]>`: Label that automatically turns `text-destructive` when the field is invalid and displays a red `*` when required.
- `<FormMessage [message=...] [class=...]>`: Reactive error message with `role="alert"` and ID linked to the input's `aria-describedby`.
- `<FormDescription [class=...]>`: Subdued help text below input controls.
- `<FormError [class=...]>`: Banner displaying form-level (non-field) errors with `role="alert"`.

### Input Controls
- `<FormInput value=... [on_input=...] [input_type="text"|"email"|"password"|...]>`: Text input with error ring and ARIA attributes.
- `<FormTextarea value=... [on_input=...] [rows=3]>`: Multi-line text input.
- `<FormSelect value=... options=... [placeholder=...]>`: Dropdown select with SSR `selected` attribute support.
- `<FormCheckbox checked=... [on_change=...] [label=...]>`: Boolean checkbox.
- `<FormSwitch checked=... [on_change=...] [label=...]>`: iOS-style animated toggle switch.
- `<FormRadioGroup value=... options=... [horizontal=false]>`: Accessible radio option group.
- `<FormFileInput [accept=...] [multiple=true] [on_change=...]>`: Browser-safe file input.

### Actions
- `<SubmitButton [submitting_text="Saving..."]>`: Submit button with loading spinner and double-submit disable state.
- `<ResetButton [on_reset=...]>`: Form reset button.

## Usage Example

```rust,ignore
use leptos::prelude::*;
use rustok_forms::{FormState, FormValidator};
use rustok_forms_leptos::*;

#[component]
pub fn PostEditor() -> impl IntoView {
    let form_state = RwSignal::new(FormState::idle());
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
        <Form state=form_state on_submit=on_submit class="space-y-4 max-w-md">
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
