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
use moka::ops::compute::{CompResult, Op};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
struct RateLimitEntry {
    bucket: u64,
    window_secs: u64,
    count: u32,
}

/// Lightweight in-memory rate limiter backed by Moka concurrent cache.
#[derive(Clone)]
pub struct MokaRateLimiter {
    cache: Arc<Cache<String, RateLimitEntry>>,
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
        let retry_after_secs = safe_window.saturating_sub(now_secs % safe_window).max(1);

        let result = self
            .cache
            .entry_by_ref(key)
            .and_compute_with(|maybe_entry| async move {
                let current = maybe_entry
                    .map(|entry| entry.into_value())
                    .filter(|entry| {
                        entry.bucket == window_bucket && entry.window_secs == safe_window
                    })
                    .map(|entry| entry.count)
                    .unwrap_or(0);

                if current >= limit {
                    Op::Nop
                } else {
                    Op::Put(RateLimitEntry {
                        bucket: window_bucket,
                        window_secs: safe_window,
                        count: current.saturating_add(1),
                    })
                }
            })
            .await;

        match result {
            CompResult::Inserted(entry) | CompResult::ReplacedWith(entry) => {
                Ok(RateLimitDecision {
                    allowed: true,
                    current_count: entry.into_value().count,
                    limit,
                    retry_after_secs: None,
                })
            }
            CompResult::Unchanged(entry) => Ok(RateLimitDecision {
                allowed: false,
                current_count: entry.into_value().count,
                limit,
                retry_after_secs: Some(retry_after_secs),
            }),
            CompResult::StillNone(_) | CompResult::Removed(_) => Ok(RateLimitDecision {
                allowed: false,
                current_count: 0,
                limit,
                retry_after_secs: Some(retry_after_secs),
            }),
        }
    }

    async fn reset(&self, key: &str) -> SecurityResult<()> {
        self.cache.invalidate(key).await;
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

    #[tokio::test]
    async fn test_moka_rate_limiter_reset_clears_active_bucket() {
        let limiter = MokaRateLimiter::default();
        let key = "test:ip:10.0.0.99";

        assert!(limiter.check_rate_limit(key, 1, 60).await.unwrap().allowed);
        assert!(!limiter.check_rate_limit(key, 1, 60).await.unwrap().allowed);

        limiter.reset(key).await.unwrap();
        assert!(limiter.check_rate_limit(key, 1, 60).await.unwrap().allowed);
    }
}
