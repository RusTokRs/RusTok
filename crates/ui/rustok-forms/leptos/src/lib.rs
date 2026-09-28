//! `rustok-forms-leptos` — Leptos 0.8 adapter for the framework-agnostic `rustok-forms` library.
//!
//! Provides structural form layout components, accessible input controls,
//! lifecycle-aware submit buttons, and reactive context coordination.
//!
//! # Example
//!
//! ```rust,ignore
//! use leptos::prelude::*;
//! use rustok_forms::{FormState, FormValidator};
//! use rustok_forms_leptos::*;
//!
//! #[component]
//! pub fn MyForm() -> impl IntoView {
//!     let form_state = RwSignal::new(FormState::idle());
//!     let (title, set_title) = signal(String::new());
//!
//!     let on_submit = move |_| {
//!         let val = title.get();
//!         let validation = FormValidator::new()
//!             .required("title", &val, "Title is required")
//!             .finish();
//!
//!         match validation {
//!             Ok(_) => {
//!                 // execute async action...
//!                 form_state.update(|s| s.set_submitted_success());
//!             }
//!             Err(errs) => {
//!                 form_state.update(|s| s.set_field_errors(errs));
//!             }
//!         }
//!     };
//!
//!     view! {
//!         <Form state=form_state on_submit=on_submit>
//!             <FormError />
//!             <FormField name="title">
//!                 <FormLabel required=true>"Title"</FormLabel>
//!                 <FormInput value=title on_input=set_title placeholder="Enter post title..." />
//!                 <FormDescription>"Public title of this resource"</FormDescription>
//!                 <FormMessage />
//!             </FormField>
//!             <SubmitButton submitting_text="Saving...">"Save"</SubmitButton>
//!         </Form>
//!     }
//! }
//! ```

pub mod button;
pub mod context;
pub mod field;
pub mod form;
pub mod inputs;

pub use button::{ButtonSize, ButtonVariant, ResetButton, SubmitButton};
pub use context::{FieldContext, FormContext};
pub use field::{
    FormControl, FormDescription, FormError, FormField, FormHelperText, FormItem, FormLabel,
    FormMessage,
};
pub use form::Form;
pub use inputs::{
    FormCheckbox, FormFileInput, FormHiddenInput, FormInput, FormNumberInput, FormPasswordInput,
    FormRadioGroup, FormSearchInput, FormSelect, FormSwitch, FormTextarea,
};
