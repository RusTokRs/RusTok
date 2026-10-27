/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Canonical platform owner of Security capabilities in RusToK.
//!
//! # Architecture and Guidelines
//!
//! This crate follows Fluid Backend Architecture (FBA).
//! For design and implementation standards, see:
//! - [Backend Module Implementation Guide](https://github.com/RusTokRs/RusTok/blob/main/docs/backend/module-backend-implementation.md)
//! - [Module Platform Overview](https://github.com/RusTokRs/RusTok/blob/main/docs/modules/overview.md)

#![deny(clippy::float_arithmetic)]

pub mod adapters;
pub mod domain;
pub mod dto;
pub mod error;
pub mod events;
pub mod integrations;
pub mod module;
pub mod ports;
pub mod services;

#[cfg(test)]
mod contract_tests;

pub mod entities;
pub mod migrations;

pub use adapters::{HoneypotVerifier, MokaBruteForceTracker, MokaRateLimiter, NoopCaptchaVerifier};
#[cfg(feature = "captcha-http")]
pub use adapters::{RecaptchaVerifier, TurnstileVerifier};
pub use domain::{
    CaptchaProviderKind, IpNet, IpRule, IpRuleType, LockoutDecision, RateLimitDecision,
    SecurityAuditLog, SecurityEventType, SecurityPolicy, SecuritySeverity, SecurityStatus,
};
pub use dto::{
    CreateIpRuleInput, LogSecurityEventInput, SecuritySummary, UpdateSecurityPolicyInput,
};
pub use error::{SecurityError, SecurityResult};
pub use events::{
    SECURITY_EVENT_TOPIC, SecurityCreatedEvent, SecurityDeletedEvent, SecurityDomainEvent,
    SecurityUpdatedEvent,
};
pub use module::SecurityModule;
pub use ports::{BruteForceTrackerPort, CaptchaVerifierPort, IpFilterPort, RateLimiterPort};
pub use services::SecurityService;
