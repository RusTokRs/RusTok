//! Newsletter module — subscriber management, campaign lifecycle, and delivery orchestration.
//!
//! This crate owns the newsletter bounded context: subscriber lists, segments,
//! campaigns, content aggregation from source modules, and email delivery
//! through `rustok-email`.

pub mod error;
pub mod model;

#[cfg(feature = "server")]
pub mod domain;
#[cfg(feature = "server")]
pub mod dto;
#[cfg(feature = "server")]
pub mod entities;
#[cfg(feature = "server")]
pub mod migrations;
#[cfg(feature = "server")]
pub mod module;
#[cfg(feature = "server")]
pub mod ports;
#[cfg(feature = "server")]
pub mod provider;
#[cfg(feature = "server")]
pub mod services;

#[cfg(feature = "server")]
pub mod graphql;

#[cfg(all(test, feature = "server"))]
mod contract_tests;

// Deliberate public re-exports for crate consumers.
pub use error::{NewsletterError, NewsletterResult};
pub use model::*;

#[cfg(feature = "server")]
pub use graphql::NewsletterGraphqlRuntimeData;
#[cfg(feature = "server")]
pub use module::NewsletterModule;
#[cfg(feature = "server")]
pub use ports::{
    CampaignPortImpl, NewsletterCampaignPort, NewsletterSubscriberPort, SubscriberPortImpl,
};
#[cfg(feature = "server")]
pub use provider::{ContentProviderRegistry, NewsletterContentProvider};
#[cfg(feature = "server")]
pub use services::{CampaignService, SubscriberService};
