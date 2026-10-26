/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure domain model, state machines, invariants, and value objects for Security.
//!
//! In accordance with FBA, this module MUST NOT depend on database or transport frameworks.

pub mod ip_net;
pub use ip_net::IpNet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::{SecurityError, SecurityResult};

/// Type of rule for an IP or CIDR address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IpRuleType {
    Whitelist,
    Blacklist,
}

/// A tenant-scoped IP or CIDR rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpRule {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub ip_or_cidr: String,
    pub rule_type: IpRuleType,
    pub reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl IpRule {
    /// Validates and parses the IP/CIDR string.
    pub fn parse_net(&self) -> SecurityResult<IpNet> {
        IpNet::parse(&self.ip_or_cidr)
    }

    /// Checks if the rule has expired relative to a given timestamp.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.map(|exp| exp <= now).unwrap_or(false)
    }
}

/// Types of security audit events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityEventType {
    AuthFailedLogin,
    AuthAccountLocked,
    AuthLoginSuccess,
    AuthPasswordReset,
    IpBlocked,
    RateLimitExceeded,
    MfaChallenged,
    MfaFailed,
    PrivilegeEscalation,
    SettingsChanged,
    CaptchaFailed,
}

/// Severity classification for security events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecuritySeverity {
    #[default]
    Info,
    Warning,
    Critical,
}

/// A structured security audit log entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SecurityAuditLog {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub event_type: SecurityEventType,
    pub severity: SecuritySeverity,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Supported bot and captcha verification providers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CaptchaProviderKind {
    #[default]
    None,
    Honeypot,
    Turnstile,
    Recaptcha,
}

/// Tenant-level security policy settings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityPolicy {
    pub tenant_id: Uuid,
    pub max_failed_login_attempts: u32,
    pub lockout_duration_secs: u64,
    pub rate_limit_per_minute: u32,
    pub captcha_provider: CaptchaProviderKind,
    pub captcha_site_key: Option<String>,
    pub captcha_secret_key: Option<String>,
    pub mfa_enforced_for_roles: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            tenant_id: Uuid::nil(),
            max_failed_login_attempts: 5,
            lockout_duration_secs: 900, // 15 minutes
            rate_limit_per_minute: 120,
            captcha_provider: CaptchaProviderKind::None,
            captcha_site_key: None,
            captcha_secret_key: None,
            mfa_enforced_for_roles: vec!["super_admin".to_string()],
            updated_at: Utc::now(),
        }
    }
}

/// Outcome of checking brute-force lockout status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockoutDecision {
    pub is_locked: bool,
    pub failed_attempts: u32,
    pub max_attempts: u32,
    pub remaining_lockout_secs: Option<u64>,
}

/// Outcome of checking rate-limit status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitDecision {
    pub allowed: bool,
    pub current_count: u32,
    pub limit: u32,
    pub retry_after_secs: Option<u64>,
}

/// Domain lifecycle state for Security.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SecurityStatus {
    #[default]
    Draft,
    Active,
    Archived,
}

impl SecurityStatus {
    /// Validates whether a state transition is permitted according to domain invariants.
    pub fn can_transition_to(&self, target: Self) -> bool {
        match (self, target) {
            (Self::Draft, Self::Active) => true,
            (Self::Active, Self::Archived) => true,
            (Self::Archived, Self::Draft) => true,
            (current, target) if *current == target => true,
            _ => false,
        }
    }

    /// Attempts state transition with typed domain invariant guard.
    pub fn transition_to(&mut self, target: Self) -> SecurityResult<()> {
        if !self.can_transition_to(target) {
            return Err(SecurityError::Validation(format!(
                "Invalid state transition from {:?} to {:?}",
                self, target
            )));
        }
        *self = target;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_lifecycle_transitions() {
        let mut status = SecurityStatus::Draft;
        assert!(status.transition_to(SecurityStatus::Active).is_ok());
        assert_eq!(status, SecurityStatus::Active);

        // Invalid transition: Active -> Draft
        assert!(status.transition_to(SecurityStatus::Draft).is_err());

        // Valid transition: Active -> Archived
        assert!(status.transition_to(SecurityStatus::Archived).is_ok());
        assert_eq!(status, SecurityStatus::Archived);
    }
}
