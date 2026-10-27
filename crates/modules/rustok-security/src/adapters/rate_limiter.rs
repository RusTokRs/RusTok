/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use crate::domain::RateLimitDecision;
use crate::error::SecurityResult;
use crate::ports::RateLimiterPort;
use async_trait::async_trait;
use chrono::Utc;
use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

/// Lightweight in-memory rate limiter backed by Moka concurrent cache.
#[derive(Clone)]
pub struct MokaRateLimiter {
    cache: Arc<Cache<String, u32>>,
}

impl MokaRateLimiter {
    /// Creates a new MokaRateLimiter with max entry capacity and eviction TTL.
    pub fn new(max_capacity: u64, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(ttl)
            .build();
        Self {
            cache: Arc::new(cache),
        }
    }
}

impl Default for MokaRateLimiter {
    fn default() -> Self {
        // Default: up to 50,000 tracked keys, evicted after 5 minutes of inactivity
        Self::new(50_000, Duration::from_secs(300))
    }
}

#[async_trait]
impl RateLimiterPort for MokaRateLimiter {
    async fn check_rate_limit(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> SecurityResult<RateLimitDecision> {
        let now_secs = Utc::now().timestamp().max(0) as u64;
        let safe_window = window_secs.max(1);
        let window_bucket = now_secs / safe_window;
        let cache_key = format!("{}:{}", key, window_bucket);

        let current = self.cache.get(&cache_key).await.unwrap_or(0);
        let retry_after_secs = safe_window.saturating_sub(now_secs % safe_window).max(1);

        if current >= limit {
            Ok(RateLimitDecision {
                allowed: false,
                current_count: current,
                limit,
                retry_after_secs: Some(retry_after_secs),
            })
        } else {
            let next_count = current + 1;
            self.cache.insert(cache_key, next_count).await;
            Ok(RateLimitDecision {
                allowed: true,
                current_count: next_count,
                limit,
                retry_after_secs: None,
            })
        }
    }

    async fn reset(&self, key: &str) -> SecurityResult<()> {
        let now_secs = Utc::now().timestamp().max(0) as u64;
        // Invalidate current and previous window bucket
        for offset in 0..=2 {
            let window_bucket = now_secs.saturating_sub(offset);
            self.cache
                .invalidate(&format!("{}:{}", key, window_bucket))
                .await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_moka_rate_limiter_allows_under_limit() {
        let limiter = MokaRateLimiter::default();
        let key = "test:ip:127.0.0.1";

        let d1 = limiter.check_rate_limit(key, 2, 60).await.unwrap();
        assert!(d1.allowed);
        assert_eq!(d1.current_count, 1);

        let d2 = limiter.check_rate_limit(key, 2, 60).await.unwrap();
        assert!(d2.allowed);
        assert_eq!(d2.current_count, 2);

        // Third attempt exceeds limit of 2
        let d3 = limiter.check_rate_limit(key, 2, 60).await.unwrap();
        assert!(!d3.allowed);
        assert_eq!(d3.current_count, 2);
        assert!(d3.retry_after_secs.is_some());
    }

    #[tokio::test]
    async fn test_moka_rate_limiter_different_keys_isolated() {
        let limiter = MokaRateLimiter::default();
        let key1 = "ip:10.0.0.1";
        let key2 = "ip:10.0.0.2";

        let _ = limiter.check_rate_limit(key1, 1, 60).await.unwrap();
        let d1_block = limiter.check_rate_limit(key1, 1, 60).await.unwrap();
        assert!(!d1_block.allowed);

        // key2 is unaffected
        let d2_allow = limiter.check_rate_limit(key2, 1, 60).await.unwrap();
        assert!(d2_allow.allowed);
    }
}
