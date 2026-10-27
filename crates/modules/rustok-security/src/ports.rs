/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Service Provider Interface (SPI) ports for Security Suite capabilities.

use crate::domain::{LockoutDecision, RateLimitDecision};
use crate::error::SecurityResult;
use async_trait::async_trait;
use std::net::IpAddr;
use uuid::Uuid;

/// Port for rate limiting requests across IP, tenant, or route boundaries.
#[async_trait]
pub trait RateLimiterPort: Send + Sync {
    /// Checks and consumes one permit for the given key.
    async fn check_rate_limit(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> SecurityResult<RateLimitDecision>;

    /// Resets the counter for the given key.
    async fn reset(&self, key: &str) -> SecurityResult<()>;
}

/// Port for verifying bot/human challenge tokens (Captcha, Turnstile, Honeypot).
#[async_trait]
pub trait CaptchaVerifierPort: Send + Sync {
    /// Verifies the challenge response token.
    async fn verify(&self, token: &str, remote_ip: Option<&IpAddr>) -> SecurityResult<bool>;

    /// Returns the provider identifier.
    fn provider_name(&self) -> &'static str;
}

/// Port for evaluating IP access policies (Blacklist / Whitelist).
#[async_trait]
pub trait IpFilterPort: Send + Sync {
    /// Evaluates whether an IP address is permitted to access the tenant.
    async fn is_ip_allowed(&self, tenant_id: Uuid, ip: &IpAddr) -> SecurityResult<bool>;
}

/// Port for tracking failed login attempts and enforcing brute-force lockouts.
#[async_trait]
pub trait BruteForceTrackerPort: Send + Sync {
    /// Checks if the given identity + IP is currently locked out.
    async fn check_lockout(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
    ) -> SecurityResult<LockoutDecision>;

    /// Records a failed login attempt, potentially triggering a lockout.
    async fn record_failure(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
        max_attempts: u32,
        lockout_duration_secs: u64,
    ) -> SecurityResult<LockoutDecision>;

    /// Clears failed attempt counters upon a successful authentication.
    async fn record_success(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
    ) -> SecurityResult<()>;
}
