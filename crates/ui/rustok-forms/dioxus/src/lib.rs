//! `rustok-forms-dioxus` — Dioxus 0.6 adapter for the framework-agnostic `rustok-forms` library.
//!
//! Provides structural form layout components, accessible input controls,
//! lifecycle-aware submit buttons, and reactive context coordination.
//!
//! # Overview
//!
//! `rustok-forms-dioxus` bridges the framework-agnostic form primitives of
//! [`rustok-forms`](https://docs.rs/rustok-forms) with Dioxus 0.6 reactive RSX views:
//!
//! - **Context Coordination**: [`Form`] provides [`FormContext`], [`FormField`] provides [`FieldContext`].
//! - **Automatic Accessibility**: Propagates `aria-invalid`, `aria-describedby`, and `aria-errormessage` IDs automatically.
//! - **Dirty Tracking**: Automatically tracks modified inputs using `DirtyTracker`.
//! - **Lifecycle-Aware Buttons**: [`SubmitButton`] disables itself and renders a loading spinner during async submission; [`ResetButton`] resets values and clears error/dirty flags.
//! - **Rich Input Controls**: Text, Password (with toggle), Number, Search, Color, Range, Split OTP/2FA, Textarea, Select, Checkbox, Switch, RadioGroup, and File input.
//!
//! # Example
//!
//! ```rust,ignore
//! use dioxus::prelude::*;
//! use rustok_forms::{FormState, FormValidator};
//! use rustok_forms_dioxus::*;
//!
//! #[component]
//! pub fn MyForm() -> Element {
//!     let mut form_state = use_signal(FormState::idle);
//!     let mut title = use_signal(String::new);
//!
//!     let on_submit = move |_| {
//!         let val = title.read().clone();
//!         let validation = FormValidator::new()
//!             .required("title", &val, "Title is required")
//!             .finish();
//!
//!         match validation {
//!             Ok(_) => {
//!                 // execute async action...
//!                 form_state.write().set_submitted_success();
//!             }
//!             Err(errs) => {
//!                 form_state.write().set_field_errors(errs);
//!             }
//!         }
//!     };
//!
//!     rsx! {
//!         Form {
//!             state: form_state,
//!             on_submit: on_submit,
//!             FormError {}
//!             FormField {
//!                 name: "title",
//!                 FormLabel { required: true, "Title" }
//!                 FormInput {
//!                     value: title.read().clone(),
//!                     on_input: move |v| title.set(v),
//!                     placeholder: "Enter post title...",
//!                 }
//!                 FormDescription { "Public title of this resource" }
//!                 FormMessage {}
//!             }
//!             SubmitButton {
//!                 submitting_text: "Saving...",
//!                 "Save"
//!             }
//!         }
//!     }
//! }
//! ```

#![warn(missing_docs)]

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
    FormCheckbox, FormColorInput, FormFileInput, FormHiddenInput, FormInput, FormNumberInput,
    FormOtpInput, FormPasswordInput, FormRadioGroup, FormRangeInput, FormSearchInput, FormSelect,
    FormSwitch, FormTextarea,
};
