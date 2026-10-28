/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # rustok-newsletter
//!
//! Canonical platform owner of newsletter subscriber management, campaign lifecycle, and delivery orchestration in RusToK.
//!
//! # Architecture and Guidelines
//!
//! This crate follows Fluid Backend Architecture (FBA).
//! For design and implementation standards, see:
//! - [Backend Module Implementation Guide](https://github.com/RusTokRs/RusTok/blob/main/docs/backend/module-backend-implementation.md)
//! - [Module Platform Overview](https://github.com/RusTokRs/RusTok/blob/main/docs/modules/overview.md)

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
pub use model::{
    CampaignStatus, ContentFetchRequest, ContentSourceSlug, NewsletterApiError,
    NewsletterContentItem, SubscriberStatus,
};

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
