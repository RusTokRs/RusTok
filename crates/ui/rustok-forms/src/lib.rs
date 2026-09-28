//! `rustok-forms` — framework-agnostic form state, validation, and field contracts.
//!
//! This crate owns the core form abstractions shared by all UI framework adapters:
//!
//! - [`FormState`] / [`FormSubmissionStatus`] — form submission lifecycle.
//! - [`FieldError`] / [`ValidationIssue`] — validation result contracts.
//! - [`FieldKind`] / [`FieldDescriptor`] / [`FieldOption`] / [`FieldConstraints`] — field metadata and schemas.
//! - [`FormSchema`] — complete schema-driven form definitions and collective validation.
//! - [`DirtyTracker`] — tracks which fields have been modified.
//! - [`FormStepTracker`] — multi-step wizard and funnel state tracking.
//! - [`FormValidator`] / [`validation_rules`] — declarative validation engine.
//! - [`form_sanitizer`] — value sanitization and normalization utilities.
//!
//! It has zero framework dependencies (no Leptos, Dioxus, DOM, or RusTok-specific
//! imports). It depends only on `serde` for serialization support.
//!
//! # Adapter crates
//!
//! - `rustok-forms-leptos` — Leptos structural form components.
//! - `rustok-forms-dioxus` — Dioxus adapter (planned).

pub mod dirty;
pub mod error;
pub mod field;
pub mod sanitize;
pub mod schema;
pub mod state;
pub mod step;
pub mod validation;

pub use dirty::DirtyTracker;
pub use error::{
    FieldError, ValidationIssue, field_errors_to_issues, format_field_path, issues_to_field_errors,
    parse_field_path,
};
pub use field::{FieldConstraints, FieldDescriptor, FieldKind, FieldOption};
pub use sanitize::sanitize as form_sanitizer;
pub use schema::FormSchema;
pub use state::{FormState, FormSubmissionStatus};
pub use step::FormStepTracker;
pub use validation::{FormValidator, rules as validation_rules};
