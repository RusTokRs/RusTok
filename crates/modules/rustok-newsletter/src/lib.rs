//! Newsletter module — subscriber management, campaign lifecycle, and delivery orchestration.
//!
//! This crate owns the newsletter bounded context: subscriber lists, segments,
//! campaigns, content aggregation from source modules, and email delivery
//! through `rustok-email`.

pub mod domain;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;

#[cfg(feature = "server")]
pub mod graphql;

// Deliberate public re-exports for crate consumers.
pub use error::{NewsletterError, NewsletterResult};
pub use module::NewsletterModule;
pub use services::{CampaignService, SubscriberService};
