/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of Counterparty & Legal Entities capabilities in RusToK.
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

pub use domain::{
    BankAccount, Contract, ContractStatus, Party, PartyKind, PartyStatus, TaxDetails,
    validate_account_number, validate_bik, validate_inn_10, validate_inn_12, validate_kpp,
    validate_ogrn_13, validate_ogrnip_15,
};
pub use error::{PartyError, PartyResult};
pub use events::{
    PARTY_EVENT_TOPIC, PartyCreatedEvent, PartyDeletedEvent, PartyDomainEvent, PartyUpdatedEvent,
};
pub use module::PartyModule;
pub use services::PartyService;
