//! # rustok-forms
//!
//! Form submission intake, abuse controls, spam mitigation, and lead triage inbox for the RusToK platform.
//!
//! Exposes REST and GraphQL APIs for dynamic form submissions, rate limiting, honeypot detection,
//! submission state management (`new`, `read`, `handled`, `spam`), and transactional notification dispatch.

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod http;
pub mod migrations;
pub mod module;
pub mod openapi;
pub mod services;

#[cfg(test)]
mod contract_tests;

pub use dto::{
    FormSubmissionResponse, FormSubmissionState, ListFormSubmissionsFilter, SubmitFormResponse,
    UpdateFormSubmissionStateInput,
};
pub use entities::FormSubmission;
pub use error::{FormsError, FormsResult};
pub use graphql::{FormsMutation, FormsQuery};
pub use module::FormsModule;
pub use services::{
    FORM_HONEYPOT_FIELD, FORM_SUBMIT_RATE_LIMIT, FORM_SUBMIT_RATE_WINDOW_MINUTES,
    FormsNotification, FormsService, MAX_FORM_PAYLOAD_BYTES, MAX_FORM_PAYLOAD_FIELDS,
    notification_from_shared,
};
