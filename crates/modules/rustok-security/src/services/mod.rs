/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Application and domain services orchestrating business use cases for Security Suite.

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::adapters::{HoneypotVerifier, MokaBruteForceTracker, MokaRateLimiter};
use crate::domain::{
    CaptchaProviderKind, IpNet, IpRule, IpRuleType, LockoutDecision, RateLimitDecision,
    SecurityAuditLog, SecurityEventType, SecurityPolicy, SecuritySeverity,
};
use crate::dto::{CreateIpRuleInput, LogSecurityEventInput, UpdateSecurityPolicyInput};
use crate::entities::{audit_log, ip_rule, policy};
use crate::error::{SecurityError, SecurityResult};
use crate::ports::{BruteForceTrackerPort, CaptchaVerifierPort, RateLimiterPort};

/// Central service orchestrating Security Suite capabilities for tenants.
#[derive(Clone)]
pub struct SecurityService {
    db: Option<DatabaseConnection>,
    rate_limiter: Arc<dyn RateLimiterPort>,
    brute_force_tracker: Arc<dyn BruteForceTrackerPort>,
    captcha_verifier: Arc<dyn CaptchaVerifierPort>,
    // In-memory fallback stores for test/offline environments
    in_memory_rules: Arc<RwLock<Vec<IpRule>>>,
    in_memory_logs: Arc<RwLock<Vec<SecurityAuditLog>>>,
    in_memory_policies: Arc<RwLock<std::collections::HashMap<Uuid, SecurityPolicy>>>,
}

impl SecurityService {
    /// Creates a new SecurityService with standard in-memory adapters and optional database pool.
    pub fn new(db: Option<DatabaseConnection>) -> Self {
        Self {
            db,
            rate_limiter: Arc::new(MokaRateLimiter::default()),
            brute_force_tracker: Arc::new(MokaBruteForceTracker::default()),
            captcha_verifier: Arc::new(HoneypotVerifier::default()),
            in_memory_rules: Arc::new(RwLock::new(Vec::new())),
            in_memory_logs: Arc::new(RwLock::new(Vec::new())),
            in_memory_policies: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Builder method to supply a custom RateLimiterPort adapter.
    pub fn with_rate_limiter(mut self, limiter: Arc<dyn RateLimiterPort>) -> Self {
        self.rate_limiter = limiter;
        self
    }

    /// Builder method to supply a custom BruteForceTrackerPort adapter.
    pub fn with_brute_force_tracker(mut self, tracker: Arc<dyn BruteForceTrackerPort>) -> Self {
        self.brute_force_tracker = tracker;
        self
    }

    /// Builder method to supply a custom CaptchaVerifierPort adapter.
    pub fn with_captcha_verifier(mut self, verifier: Arc<dyn CaptchaVerifierPort>) -> Self {
        self.captcha_verifier = verifier;
        self
    }

    // ───────────────────────────────────────────────
    // IP Access Control
    // ───────────────────────────────────────────────

    /// Creates and persists an IP/CIDR rule for a tenant.
    pub async fn add_ip_rule(
        &self,
        tenant_id: Uuid,
        input: CreateIpRuleInput,
    ) -> SecurityResult<IpRule> {
        // Validate CIDR syntax upfront
        let _ = IpNet::parse(&input.ip_or_cidr)?;

        let now = Utc::now();
        let rule = IpRule {
            id: Uuid::new_v4(),
            tenant_id,
            ip_or_cidr: input.ip_or_cidr.trim().to_string(),
            rule_type: input.rule_type,
            reason: input.reason,
            expires_at: input.expires_at,
            created_at: now,
            updated_at: now,
        };

        if let Some(db) = &self.db {
            let active = ip_rule::ActiveModel {
                id: Set(rule.id),
                tenant_id: Set(rule.tenant_id),
                ip_or_cidr: Set(rule.ip_or_cidr.clone()),
                rule_type: Set(match rule.rule_type {
                    IpRuleType::Whitelist => "whitelist".to_string(),
                    IpRuleType::Blacklist => "blacklist".to_string(),
                }),
                reason: Set(rule.reason.clone()),
                expires_at: Set(rule.expires_at),
                created_at: Set(rule.created_at),
                updated_at: Set(rule.updated_at),
            };
            active
                .insert(db)
                .await
                .map_err(|e| SecurityError::Database(e.to_string()))?;
        } else {
            let mut store = self.in_memory_rules.write().await;
            store.push(rule.clone());
        }

        Ok(rule)
    }

    /// Deletes an IP rule by ID.
    pub async fn delete_ip_rule(&self, tenant_id: Uuid, rule_id: Uuid) -> SecurityResult<()> {
        if let Some(db) = &self.db {
            ip_rule::Entity::delete_many()
                .filter(ip_rule::Column::TenantId.eq(tenant_id))
                .filter(ip_rule::Column::Id.eq(rule_id))
                .exec(db)
                .await
                .map_err(|e| SecurityError::Database(e.to_string()))?;
        } else {
            let mut store = self.in_memory_rules.write().await;
            store.retain(|r| !(r.tenant_id == tenant_id && r.id == rule_id));
        }
        Ok(())
    }

    /// Lists active IP rules for a tenant.
    pub async fn list_ip_rules(&self, tenant_id: Uuid) -> SecurityResult<Vec<IpRule>> {
        if let Some(db) = &self.db {
            let models = ip_rule::Entity::find()
                .filter(ip_rule::Column::TenantId.eq(tenant_id))
                .order_by_desc(ip_rule::Column::CreatedAt)
                .all(db)
                .await
                .map_err(|e| SecurityError::Database(e.to_string()))?;

            Ok(models
                .into_iter()
                .map(|m| IpRule {
                    id: m.id,
                    tenant_id: m.tenant_id,
                    ip_or_cidr: m.ip_or_cidr,
                    rule_type: match m.rule_type.as_str() {
                        "whitelist" => IpRuleType::Whitelist,
                        _ => IpRuleType::Blacklist,
                    },
                    reason: m.reason,
                    expires_at: m.expires_at,
                    created_at: m.created_at,
                    updated_at: m.updated_at,
                })
                .collect())
        } else {
            let store = self.in_memory_rules.read().await;
            let rules = store.iter().filter(|r| r.tenant_id == tenant_id).cloned().collect();
            Ok(rules)
        }
    }

    /// Evaluates whether an IP address is permitted to access the tenant.
    pub async fn is_ip_allowed(&self, tenant_id: Uuid, ip: &IpAddr) -> SecurityResult<bool> {
        let rules = self.list_ip_rules(tenant_id).await?;
        let now = Utc::now();

        let mut has_whitelist = false;
        let mut matched_whitelist = false;

        for rule in &rules {
            if rule.is_expired(now) {
                continue;
            }

            if let Ok(net) = rule.parse_net() {
                if net.contains(ip) {
                    match rule.rule_type {
                        IpRuleType::Whitelist => {
                            matched_whitelist = true;
                        }
                        IpRuleType::Blacklist => {
                            return Ok(false);
                        }
                    }
                }
            }

            if rule.rule_type == IpRuleType::Whitelist {
                has_whitelist = true;
            }
        }

        // If a whitelist exists, the IP must match at least one whitelist rule
        if has_whitelist && !matched_whitelist {
            return Ok(false);
        }

        Ok(true)
    }

    // ───────────────────────────────────────────────
    // Security Audit Log
    // ───────────────────────────────────────────────

    /// Records a structured security audit event.
    pub async fn log_event(
        &self,
        tenant_id: Uuid,
        input: LogSecurityEventInput,
    ) -> SecurityResult<SecurityAuditLog> {
        let entry = SecurityAuditLog {
            id: Uuid::new_v4(),
            tenant_id,
            actor_id: input.actor_id,
            ip_address: input.ip_address,
            user_agent: input.user_agent,
            event_type: input.event_type,
            severity: input.severity,
            details: input.details,
            created_at: Utc::now(),
        };

        if let Some(db) = &self.db {
            let active = audit_log::ActiveModel {
                id: Set(entry.id),
                tenant_id: Set(entry.tenant_id),
                actor_id: Set(entry.actor_id),
                ip_address: Set(entry.ip_address.clone()),
                user_agent: Set(entry.user_agent.clone()),
                event_type: Set(format!("{:?}", entry.event_type)),
                severity: Set(format!("{:?}", entry.severity)),
                details: Set(entry.details.clone()),
                created_at: Set(entry.created_at),
            };
            active
                .insert(db)
                .await
                .map_err(|e| SecurityError::Database(e.to_string()))?;
        } else {
            let mut store = self.in_memory_logs.write().await;
            store.push(entry.clone());
        }

        Ok(entry)
    }

    /// Lists recent security audit logs for a tenant.
    pub async fn list_audit_logs(
        &self,
        tenant_id: Uuid,
        limit: u64,
        offset: u64,
    ) -> SecurityResult<Vec<SecurityAuditLog>> {
        if let Some(db) = &self.db {
            let models = audit_log::Entity::find()
                .filter(audit_log::Column::TenantId.eq(tenant_id))
                .order_by_desc(audit_log::Column::CreatedAt)
                .limit(limit)
                .offset(offset)
                .all(db)
                .await
                .map_err(|e| SecurityError::Database(e.to_string()))?;

            Ok(models
                .into_iter()
                .map(|m| SecurityAuditLog {
                    id: m.id,
                    tenant_id: m.tenant_id,
                    actor_id: m.actor_id,
                    ip_address: m.ip_address,
                    user_agent: m.user_agent,
                    event_type: match m.event_type.as_str() {
                        "AuthFailedLogin" => SecurityEventType::AuthFailedLogin,
                        "AuthAccountLocked" => SecurityEventType::AuthAccountLocked,
                        "AuthLoginSuccess" => SecurityEventType::AuthLoginSuccess,
                        "IpBlocked" => SecurityEventType::IpBlocked,
                        "RateLimitExceeded" => SecurityEventType::RateLimitExceeded,
                        "MfaChallenged" => SecurityEventType::MfaChallenged,
                        "MfaFailed" => SecurityEventType::MfaFailed,
                        "PrivilegeEscalation" => SecurityEventType::PrivilegeEscalation,
                        "SettingsChanged" => SecurityEventType::SettingsChanged,
                        "CaptchaFailed" => SecurityEventType::CaptchaFailed,
                        _ => SecurityEventType::AuthFailedLogin,
                    },
                    severity: match m.severity.as_str() {
                        "Critical" => SecuritySeverity::Critical,
                        "Warning" => SecuritySeverity::Warning,
                        _ => SecuritySeverity::Info,
                    },
                    details: m.details,
                    created_at: m.created_at,
                })
                .collect())
        } else {
            let store = self.in_memory_logs.read().await;
            let logs = store
                .iter()
                .filter(|l| l.tenant_id == tenant_id)
                .skip(offset as usize)
                .take(limit as usize)
                .cloned()
                .collect();
            Ok(logs)
        }
    }

    // ───────────────────────────────────────────────
    // Security Policies
    // ───────────────────────────────────────────────

    /// Retrieves effective security policy for a tenant.
    pub async fn get_policy(&self, tenant_id: Uuid) -> SecurityResult<SecurityPolicy> {
        if let Some(db) = &self.db {
            if let Some(m) = policy::Entity::find_by_id(tenant_id)
                .one(db)
                .await
                .map_err(|e| SecurityError::Database(e.to_string()))?
            {
                let roles: Vec<String> = serde_json::from_value(m.mfa_enforced_roles)
                    .unwrap_or_else(|_| vec!["super_admin".to_string()]);

                Ok(SecurityPolicy {
                    tenant_id: m.tenant_id,
                    max_failed_login_attempts: m.max_failed_login_attempts as u32,
                    lockout_duration_secs: m.lockout_duration_secs as u64,
                    rate_limit_per_minute: m.rate_limit_per_minute as u32,
                    captcha_provider: match m.captcha_provider.as_str() {
                        "honeypot" => CaptchaProviderKind::Honeypot,
                        "turnstile" => CaptchaProviderKind::Turnstile,
                        "recaptcha" => CaptchaProviderKind::Recaptcha,
                        _ => CaptchaProviderKind::None,
                    },
                    captcha_site_key: m.captcha_site_key,
                    captcha_secret_key: m.captcha_secret_key,
                    mfa_enforced_for_roles: roles,
                    updated_at: m.updated_at,
                })
            } else {
                let mut default_pol = SecurityPolicy::default();
                default_pol.tenant_id = tenant_id;
                Ok(default_pol)
            }
        } else {
            let store = self.in_memory_policies.read().await;
            Ok(store.get(&tenant_id).cloned().unwrap_or_else(|| {
                let mut p = SecurityPolicy::default();
                p.tenant_id = tenant_id;
                p
            }))
        }
    }

    /// Updates security policy for a tenant.
    pub async fn update_policy(
        &self,
        tenant_id: Uuid,
        input: UpdateSecurityPolicyInput,
    ) -> SecurityResult<SecurityPolicy> {
        let mut policy = self.get_policy(tenant_id).await?;

        if let Some(val) = input.max_failed_login_attempts {
            policy.max_failed_login_attempts = val;
        }
        if let Some(val) = input.lockout_duration_secs {
            policy.lockout_duration_secs = val;
        }
        if let Some(val) = input.rate_limit_per_minute {
            policy.rate_limit_per_minute = val;
        }
        if let Some(val) = input.captcha_provider {
            policy.captcha_provider = val;
        }
        if let Some(val) = input.captcha_site_key {
            policy.captcha_site_key = Some(val);
        }
        if let Some(val) = input.captcha_secret_key {
            policy.captcha_secret_key = Some(val);
        }
        if let Some(val) = input.mfa_enforced_for_roles {
            policy.mfa_enforced_for_roles = val;
        }
        policy.updated_at = Utc::now();

        if let Some(db) = &self.db {
            let active = policy::ActiveModel {
                tenant_id: Set(policy.tenant_id),
                max_failed_login_attempts: Set(policy.max_failed_login_attempts as i32),
                lockout_duration_secs: Set(policy.lockout_duration_secs as i64),
                rate_limit_per_minute: Set(policy.rate_limit_per_minute as i32),
                captcha_provider: Set(match policy.captcha_provider {
                    CaptchaProviderKind::Honeypot => "honeypot".to_string(),
                    CaptchaProviderKind::Turnstile => "turnstile".to_string(),
                    CaptchaProviderKind::Recaptcha => "recaptcha".to_string(),
                    CaptchaProviderKind::None => "none".to_string(),
                }),
                captcha_site_key: Set(policy.captcha_site_key.clone()),
                captcha_secret_key: Set(policy.captcha_secret_key.clone()),
                mfa_enforced_roles: Set(serde_json::to_value(&policy.mfa_enforced_for_roles)
                    .unwrap_or(serde_json::Value::Null)),
                updated_at: Set(policy.updated_at),
            };

            policy::Entity::insert(active)
                .on_conflict(
                    sea_orm::sea_query::OnConflict::column(policy::Column::TenantId)
                        .update_columns([
                            policy::Column::MaxFailedLoginAttempts,
                            policy::Column::LockoutDurationSecs,
                            policy::Column::RateLimitPerMinute,
                            policy::Column::CaptchaProvider,
                            policy::Column::CaptchaSiteKey,
                            policy::Column::CaptchaSecretKey,
                            policy::Column::MfaEnforcedRoles,
                            policy::Column::UpdatedAt,
                        ])
                        .to_owned(),
                )
                .exec(db)
                .await
                .map_err(|e| SecurityError::Database(e.to_string()))?;
        } else {
            let mut store = self.in_memory_policies.write().await;
            store.insert(tenant_id, policy.clone());
        }

        Ok(policy)
    }

    // ───────────────────────────────────────────────
    // Brute-force & Rate Limiting Integration
    // ───────────────────────────────────────────────

    /// Verifies if a login attempt is allowed or blocked by brute-force policy.
    pub async fn check_login_attempt(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
    ) -> SecurityResult<LockoutDecision> {
        self.brute_force_tracker
            .check_lockout(tenant_id, identity, ip)
            .await
    }

    /// Records a failed login attempt against the tenant's policy threshold.
    pub async fn record_login_failure(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
    ) -> SecurityResult<LockoutDecision> {
        let policy = self.get_policy(tenant_id).await?;
        let decision = self
            .brute_force_tracker
            .record_failure(
                tenant_id,
                identity,
                ip,
                policy.max_failed_login_attempts,
                policy.lockout_duration_secs,
            )
            .await?;

        if decision.is_locked {
            let _ = self
                .log_event(
                    tenant_id,
                    LogSecurityEventInput {
                        actor_id: None,
                        ip_address: ip.to_string(),
                        user_agent: None,
                        event_type: SecurityEventType::AuthAccountLocked,
                        severity: SecuritySeverity::Warning,
                        details: serde_json::json!({
                            "identity": identity,
                            "failed_attempts": decision.failed_attempts,
                            "lockout_secs": decision.remaining_lockout_secs,
                        }),
                    },
                )
                .await;
        }

        Ok(decision)
    }

    /// Records successful login, clearing brute-force tracking.
    pub async fn record_login_success(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
    ) -> SecurityResult<()> {
        self.brute_force_tracker
            .record_success(tenant_id, identity, ip)
            .await
    }

    /// Evaluates rate limiting for an operation key against tenant policy.
    pub async fn check_rate_limit(
        &self,
        tenant_id: Uuid,
        key: &str,
        limit_override: Option<u32>,
    ) -> SecurityResult<RateLimitDecision> {
        let policy = self.get_policy(tenant_id).await?;
        let limit = limit_override.unwrap_or(policy.rate_limit_per_minute);
        let scoped_key = format!("tenant:{}:{}", tenant_id, key);

        self.rate_limiter.check_rate_limit(&scoped_key, limit, 60).await
    }

    /// Verifies a captcha token against the configured provider.
    pub async fn verify_captcha(
        &self,
        _tenant_id: Uuid,
        token: &str,
        remote_ip: Option<&IpAddr>,
    ) -> SecurityResult<bool> {
        self.captcha_verifier.verify(token, remote_ip).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ip_whitelist_and_blacklist_evaluation() {
        let service = SecurityService::new(None);
        let tenant_id = Uuid::new_v4();
        let allowed_ip: IpAddr = "192.168.1.100".parse().unwrap();
        let blocked_ip: IpAddr = "10.0.0.99".parse().unwrap();
        let neutral_ip: IpAddr = "8.8.8.8".parse().unwrap();

        // 1. By default with no rules, all IPs are allowed
        assert!(service.is_ip_allowed(tenant_id, &neutral_ip).await.unwrap());

        // 2. Add blacklist rule for 10.0.0.0/24
        service
            .add_ip_rule(
                tenant_id,
                CreateIpRuleInput {
                    ip_or_cidr: "10.0.0.0/24".to_string(),
                    rule_type: IpRuleType::Blacklist,
                    reason: Some("Malicious subnet".to_string()),
                    expires_at: None,
                },
            )
            .await
            .unwrap();

        assert!(!service.is_ip_allowed(tenant_id, &blocked_ip).await.unwrap());
        assert!(service.is_ip_allowed(tenant_id, &neutral_ip).await.unwrap());

        // 3. Add whitelist rule for 192.168.1.0/24
        service
            .add_ip_rule(
                tenant_id,
                CreateIpRuleInput {
                    ip_or_cidr: "192.168.1.0/24".to_string(),
                    rule_type: IpRuleType::Whitelist,
                    reason: Some("Internal office network".to_string()),
                    expires_at: None,
                },
            )
            .await
            .unwrap();

        // Now that a whitelist exists, ONLY whitelisted IPs are allowed
        assert!(service.is_ip_allowed(tenant_id, &allowed_ip).await.unwrap());
        assert!(!service.is_ip_allowed(tenant_id, &neutral_ip).await.unwrap());
        assert!(!service.is_ip_allowed(tenant_id, &blocked_ip).await.unwrap());
    }

    #[tokio::test]
    async fn test_audit_log_recording_and_listing() {
        let service = SecurityService::new(None);
        let tenant_id = Uuid::new_v4();

        let log = service
            .log_event(
                tenant_id,
                LogSecurityEventInput {
                    actor_id: None,
                    ip_address: "1.2.3.4".to_string(),
                    user_agent: Some("curl/8.0".to_string()),
                    event_type: SecurityEventType::IpBlocked,
                    severity: SecuritySeverity::Critical,
                    details: serde_json::json!({"reason": "Port scan detected"}),
                },
            )
            .await
            .unwrap();

        assert_eq!(log.ip_address, "1.2.3.4");
        assert_eq!(log.severity, SecuritySeverity::Critical);

        let list = service.list_audit_logs(tenant_id, 10, 0).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, log.id);
    }

    #[tokio::test]
    async fn test_policy_defaults_and_updates() {
        let service = SecurityService::new(None);
        let tenant_id = Uuid::new_v4();

        let pol = service.get_policy(tenant_id).await.unwrap();
        assert_eq!(pol.max_failed_login_attempts, 5);
        assert_eq!(pol.captcha_provider, CaptchaProviderKind::None);

        let updated = service
            .update_policy(
                tenant_id,
                UpdateSecurityPolicyInput {
                    max_failed_login_attempts: Some(3),
                    captcha_provider: Some(CaptchaProviderKind::Honeypot),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.max_failed_login_attempts, 3);
        assert_eq!(updated.captcha_provider, CaptchaProviderKind::Honeypot);
    }
}
