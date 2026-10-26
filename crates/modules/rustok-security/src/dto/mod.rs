/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Request/response, command/query, and data transfer objects for Security.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::domain::{CaptchaProviderKind, IpRuleType, SecurityEventType, SecuritySeverity};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecuritySummary {
    pub slug: String,
    pub title: String,
    pub active_ip_rules_count: usize,
    pub today_audit_events_count: usize,
}

/// Input for creating a new IP or CIDR rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateIpRuleInput {
    pub ip_or_cidr: String,
    pub rule_type: IpRuleType,
    pub reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Input for recording a security audit event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogSecurityEventInput {
    pub actor_id: Option<Uuid>,
    pub ip_address: String,
    pub user_agent: Option<String>,
    pub event_type: SecurityEventType,
    pub severity: SecuritySeverity,
    pub details: serde_json::Value,
}

/// Input for updating tenant security policy settings.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UpdateSecurityPolicyInput {
    pub max_failed_login_attempts: Option<u32>,
    pub lockout_duration_secs: Option<u64>,
    pub rate_limit_per_minute: Option<u32>,
    pub captcha_provider: Option<CaptchaProviderKind>,
    pub captcha_site_key: Option<String>,
    pub captcha_secret_key: Option<String>,
    pub mfa_enforced_for_roles: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_serde_roundtrip() {
        let summary = SecuritySummary {
            slug: "security".into(),
            title: "Security".into(),
            active_ip_rules_count: 5,
            today_audit_events_count: 12,
        };
        let json = serde_json::to_string(&summary).expect("serialize summary");
        let decoded: SecuritySummary = serde_json::from_str(&json).expect("deserialize summary");
        assert_eq!(summary.slug, decoded.slug);
        assert_eq!(summary.title, decoded.title);
        assert_eq!(summary.active_ip_rules_count, decoded.active_ip_rules_count);
    }
}
