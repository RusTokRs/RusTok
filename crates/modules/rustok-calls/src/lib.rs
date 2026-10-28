/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of WebRTC Calls & Conferencing capabilities in RusToK.
//!
//! # Architecture and Guidelines
//!
//! This crate follows Fluid Backend Architecture (FBA).
//! For design and implementation standards, see:
//! - [Backend Module Implementation Guide](https://github.com/RusTokRs/RusTok/blob/main/docs/backend/module-backend-implementation.md)
//! - [Module Platform Overview](https://github.com/RusTokRs/RusTok/blob/main/docs/modules/overview.md)

#![deny(clippy::float_arithmetic)]

pub mod domain;
pub mod dto;
pub mod error;
pub mod events;
pub mod integrations;
pub mod module;
pub mod services;

#[cfg(test)]
mod contract_tests;

pub mod entities;
pub mod migrations;

pub use domain::CallsStatus;
pub use error::{CallsError, CallsResult};
pub use events::{
    CALLS_EVENT_TOPIC, CallsCreatedEvent, CallsDeletedEvent, CallsDomainEvent, CallsUpdatedEvent,
};
pub use module::CallsModule;
pub use services::CallsService;
