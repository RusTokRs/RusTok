//! # RusToK Email Module
//!
//! SMTP transport, email rendering, and transactional delivery contracts for RusToK.
//!
//! Provides configuration models, Tera-powered localized template rendering,
//! transactional email sender abstractions, and transport-neutral email delivery ports.

pub mod config;
pub mod error;
pub mod module;
pub mod ports;
pub mod service;
pub mod template;

#[cfg(test)]
mod contract_tests;

pub use config::{EmailConfig, SmtpConfig};
pub use error::EmailError;
pub use module::EmailModule;
pub use ports::{
    EmailDeliveryPort, EmailDeliveryReceipt, EmailDeliveryRequest, EmailProviderMode,
    require_email_delivery_policy,
};
pub use service::{
    EmailService, PasswordResetEmail, PasswordResetEmailSender, SmtpEmailSender,
    TransactionalEmailSender,
};
pub use template::{EmailTemplateProvider, RenderedEmail};
