/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of Team Issues & Tasks capabilities in RusToK.
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

pub use domain::TasksStatus;
pub use error::{TasksError, TasksResult};
pub use events::{
    TasksCreatedEvent, TasksDeletedEvent, TasksDomainEvent, TasksUpdatedEvent,
    TASKS_EVENT_TOPIC,
};
pub use module::TasksModule;
pub use services::TasksService;
