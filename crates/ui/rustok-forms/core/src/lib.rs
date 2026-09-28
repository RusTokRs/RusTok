//! `rustok-forms` — framework-agnostic form state, validation, and field contracts.
//!
//! This crate owns the core form abstractions shared by all UI framework adapters:
//!
//! - [`FormState`] / [`FormSubmissionStatus`] — form submission lifecycle.
//! - [`FieldError`] / [`ValidationIssue`] — validation result contracts.
//! - [`FieldKind`] / [`FieldDescriptor`] / [`FieldOption`] — field metadata.
//! - [`DirtyTracker`] — tracks which fields have been modified.
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
pub mod state;

pub use dirty::DirtyTracker;
pub use error::{FieldError, ValidationIssue, issues_to_field_errors};
pub use field::{FieldConstraints, FieldDescriptor, FieldKind, FieldOption};
pub use state::{FormState, FormSubmissionStatus};
