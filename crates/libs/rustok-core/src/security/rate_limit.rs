//! Rate Limiting - OWASP API Security Top 10
//!
//! Protection against:
//! - Brute force attacks
//! - DDoS attacks
//! - API abuse
//! - Resource exhaustion

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::{SecurityCategory, SecurityFinding, Severity};
use crate::security::SecurityConfig;

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per minute per IP
    pub requests_per_minute: u32,
    /// Burst capacity
    pub burst_size: u32,
    /// Login attempts per minute
    pub login_attempts_per_minute: u32,
    /// API key requests per minute
    pub api_key_requests_per_minute: u32,
    /// Block duration after exceeding limit
    pub block_duration_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 60,
            burst_size: 10,
            login_attempts_per_minute: 5,
            api_key_requests_per_minute: 1000,
            block_duration_seconds: 300, // 5 minutes
        }
    }
}

/// Rate limit result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitResult {
    Allowed,
    Blocked { retry_after: Duration },
    Limited { remaining: u32, reset_at: Instant },
}

/// Token bucket for rate limiting
#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    last_update: Instant,
    rate: f64,     // tokens per second
    capacity: f64, // max tokens
}

impl TokenBucket {
    fn new(rate: f64, capacity: f64) -> Self {
        Self {
            tokens: capacity,
            last_update: Instant::now(),
            rate,
            capacity,
        }
    }

    fn consume(&mut self, tokens: f64) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;

        // Add tokens based on elapsed time
        self.tokens = (self.tokens + elapsed * self.rate).min(self.capacity);

        if self.tokens >= tokens {
            self.tokens -= tokens;
            true
        } else {
            false
        }
    }

    fn retry_after(&self) -> Duration {
        let needed = 1.0 - self.tokens;
        if needed <= 0.0 {
            Duration::from_secs(0)
        } else {
            Duration::from_secs_f64(needed / self.rate)
        }
    }
}

/// Rate limiter implementation
#[derive(Debug)]
pub struct RateLimiter {
    buckets: Arc<RwLock<HashMap<String, TokenBucket>>>,
    config: RateLimitConfig,
}

impl RateLimiter {
    /// Create new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Check if request is allowed for IP
    pub async fn check_ip(&self, ip: IpAddr) -> RateLimitResult {
        let key = format!("ip:{}", ip);
        self.check_key(&key, self.config.requests_per_minute).await
    }

    /// Check login attempt rate limit
    pub async fn check_login(&self, identifier: &str) -> RateLimitResult {
        let key = format!("login:{}", identifier);
        self.check_key(&key, self.config.login_attempts_per_minute)
            .await
    }

    /// Check API key rate limit
    pub async fn check_api_key(&self, api_key: &str) -> RateLimitResult {
        let key = format!("api_key:{}", api_key);
        self.check_key(&key, self.config.api_key_requests_per_minute)
            .await
    }

    /// Check rate limit for arbitrary key
    async fn check_key(&self, key: &str, requests_per_minute: u32) -> RateLimitResult {
        let rate = requests_per_minute as f64 / 60.0;
        let capacity = self.config.burst_size as f64;

        let mut buckets = self.buckets.write().await;

        let bucket = buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(rate, capacity));

        if bucket.consume(1.0) {
            RateLimitResult::Allowed
        } else {
            let retry_after = bucket.retry_after();
            RateLimitResult::Blocked { retry_after }
        }
    }

    /// Clean up old buckets (call periodically)
    pub async fn cleanup(&self) {
        let mut buckets = self.buckets.write().await;
        let now = Instant::now();
        let timeout = Duration::from_secs(self.config.block_duration_seconds);

        buckets.retain(|_, bucket| now.duration_since(bucket.last_update) < timeout);
    }

    /// Reset rate limit for a key
    pub async fn reset(&self, key: &str) {
        let mut buckets = self.buckets.write().await;
        buckets.remove(key);
    }
}

/// Audit rate limiting configuration
pub async fn audit_rate_limiting(config: &SecurityConfig) -> Vec<SecurityFinding> {
    let mut findings = Vec::new();

    if config.rate_limit.requests_per_minute == 0 {
        findings.push(SecurityFinding {
            category: SecurityCategory::AuthFailures,
            severity: Severity::High,
            description: "Rate limiting is disabled (requests_per_minute = 0)".to_string(),
            remediation: "Enable rate limiting to prevent brute force and DDoS attacks".to_string(),
        });
    } else if config.rate_limit.requests_per_minute > 10000 {
        findings.push(SecurityFinding {
            category: SecurityCategory::AuthFailures,
            severity: Severity::Low,
            description: "Rate limit is very high (> 10000 req/min)".to_string(),
            remediation: "Consider lowering rate limit for better protection".to_string(),
        });
    }

    if config.rate_limit.login_attempts_per_minute > 10 {
        findings.push(SecurityFinding {
            category: SecurityCategory::AuthFailures,
            severity: Severity::Medium,
            description: "Login rate limit is too high (> 10 attempts/min)".to_string(),
            remediation: "Lower login rate limit to 3-5 attempts per minute".to_string(),
        });
    }

    if config.rate_limit.block_duration_seconds < 60 {
        findings.push(SecurityFinding {
            category: SecurityCategory::AuthFailures,
            severity: Severity::Low,
            description: "Block duration is very short (< 1 minute)".to_string(),
            remediation: "Increase block duration to at least 5 minutes (300 seconds)".to_string(),
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket() {
        let mut bucket = TokenBucket::new(1.0, 5.0);
        assert!(bucket.consume(1.0));
        assert!(bucket.consume(1.0));
        assert!(bucket.consume(1.0));
        assert!(bucket.consume(1.0));
        assert!(bucket.consume(1.0));
        // Bucket should be empty now
        assert!(!bucket.consume(1.0));
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let config = RateLimitConfig {
            requests_per_minute: 60,
            burst_size: 5,
            ..Default::default()
        };
        let limiter = RateLimiter::new(config);

        // First 5 requests should pass (burst)
        for _ in 0..5 {
            assert_eq!(
                limiter.check_ip("127.0.0.1".parse().unwrap()).await,
                RateLimitResult::Allowed
            );
        }

        // 6th request should be blocked
        let result = limiter.check_ip("127.0.0.1".parse().unwrap()).await;
        assert!(matches!(result, RateLimitResult::Blocked { .. }));
    }
}
