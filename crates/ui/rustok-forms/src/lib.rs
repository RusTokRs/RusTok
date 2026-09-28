//! # `rustok-forms`
//!
//! Framework-agnostic form state management, declarative validation engine, field descriptor schemas,
//! and dirty tracking for the RusToK platform and modern Rust web applications.
//!
//! `rustok-forms` is a pure **Framework-Free Core (FFA)** crate designed with zero DOM, zero UI framework
//! dependencies, and zero RusToK-specific internal dependencies. It depends only on `serde` and `serde_json`
//! for serialization.
//!
//! ## Key Capabilities
//!
//! - **Form Lifecycle ([`FormState`], [`FormSubmissionStatus`])**: Track submission state, top-level errors,
//!   and per-field errors with robust lifecycle transitions.
//! - **Validation Engine ([`FormValidator`], [`validation_rules`])**: Declarative, composable validation builder
//!   supporting standard rules (email, URL, slug, ranges, phone, date, UUID, etc.) and nested sub-form validation.
//! - **Field Descriptors ([`FieldDescriptor`], [`FieldConstraints`], [`FieldKind`], [`FieldOption`])**: Metadata contracts
//!   enabling schema-driven forms, UI reflection, and cross-framework parity.
//! - **Form Schemas ([`FormSchema`])**: Collective form definition and validation against maps or JSON objects.
//! - **Dirty Tracking ([`DirtyTracker`])**: Set-based tracker recording which fields were modified relative to initial state.
//! - **Multi-Step Wizards ([`FormStepTracker`])**: Multi-step funnel and wizard step navigation and completion tracking.
//! - **Sanitization ([`form_sanitizer`])**: Normalization utilities for slugs, phone numbers, emails, HTML tags, and numeric clamping.
//! - **Structured Paths ([`ValidationIssue`], [`parse_field_path`], [`format_field_path`])**: Array and nested object path contracts.
//!
//! ## Companion Framework Adapters
//!
//! - `rustok-forms-leptos`: Leptos 0.8 SSR-first component adapter with WAI-ARIA and RusToK design tokens.
//! - `rustok-forms-dioxus`: Dioxus adapter (planned).
//!
//! ## Basic Usage
//!
//! ```rust
//! use rustok_forms::{FormState, FormValidator, form_sanitizer};
//!
//! // 1. Create initial form state
//! let mut state = FormState::idle();
//! assert!(!state.is_submitting);
//!
//! // 2. Validate input fields
//! let username = "  alex  ";
//! let clean_username = form_sanitizer::trim(username);
//! let email = "alex@example.com";
//!
//! let validation = FormValidator::new()
//!     .required("username", clean_username, "Username is required")
//!     .min_length("username", clean_username, 3, "Username must be >= 3 characters")
//!     .email("email", email, "Invalid email address")
//!     .finish();
//!
//! match validation {
//!     Ok(_) => {
//!         // Transition to submitting during async action
//!         state.set_submitting();
//!         assert!(state.is_submitting);
//!
//!         // On success:
//!         state.set_submitted_success();
//!         assert!(state.is_success());
//!     }
//!     Err(errors) => {
//!         state.set_field_errors(errors);
//!         assert!(state.has_errors());
//!     }
//! }
//! ```
//!
//! ## Schema-Driven Validation
//!
//! ```rust
//! use rustok_forms::{FieldDescriptor, FieldKind, FormSchema};
//! use std::collections::HashMap;
//!
//! let schema = FormSchema::new("login")
//!     .with_field(FieldDescriptor::new("email", FieldKind::Email).with_required(true))
//!     .with_field(FieldDescriptor::new("password", FieldKind::Password).with_required(true).with_min_length(8));
//!
//! let mut input = HashMap::new();
//! input.insert("email".to_string(), "user@example.com".to_string());
//! input.insert("password".to_string(), "short".to_string());
//!
//! let errors = schema.validate_map(&input);
//! assert_eq!(errors.len(), 1);
//! assert_eq!(errors[0].field, "password");
//! ```

#![warn(missing_docs)]

pub mod dirty;
pub mod error;
pub mod field;
pub mod sanitize;
pub mod schema;
pub mod state;
pub mod step;
pub mod validation;

#[doc(inline)]
pub use dirty::DirtyTracker;
#[doc(inline)]
pub use error::{
    FieldError, ValidationIssue, field_errors_to_issues, format_field_path, issues_to_field_errors,
    parse_field_path,
};
#[doc(inline)]
pub use field::{FieldConstraints, FieldDescriptor, FieldKind, FieldOption};
#[doc(inline)]
pub use sanitize::sanitize as form_sanitizer;
#[doc(inline)]
pub use schema::FormSchema;
#[doc(inline)]
pub use state::{FormState, FormSubmissionStatus};
#[doc(inline)]
pub use step::FormStepTracker;
#[doc(inline)]
pub use validation::{FormValidator, rules as validation_rules};
