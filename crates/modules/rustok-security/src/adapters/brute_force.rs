/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use crate::domain::LockoutDecision;
use crate::error::SecurityResult;
use crate::ports::BruteForceTrackerPort;
use async_trait::async_trait;
use chrono::Utc;
use moka::future::Cache;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Debug)]
struct AttemptRecord {
    failed_count: u32,
    locked_until_secs: Option<u64>,
}

/// In-memory brute force tracker backed by Moka concurrent cache.
#[derive(Clone)]
pub struct MokaBruteForceTracker {
    cache: Arc<Cache<String, AttemptRecord>>,
}

impl MokaBruteForceTracker {
    pub fn new(max_capacity: u64, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(ttl)
            .build();
        Self {
            cache: Arc::new(cache),
        }
    }

    fn key(tenant_id: Uuid, identity: &str, ip: &IpAddr) -> String {
        format!("{}:{}:{}", tenant_id, identity.trim().to_lowercase(), ip)
    }
}

impl Default for MokaBruteForceTracker {
    fn default() -> Self {
        // Track up to 100,000 attempts, evict entries idle for 1 hour
        Self::new(100_000, Duration::from_secs(3600))
    }
}

#[async_trait]
impl BruteForceTrackerPort for MokaBruteForceTracker {
    async fn check_lockout(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
    ) -> SecurityResult<LockoutDecision> {
        let key = Self::key(tenant_id, identity, ip);
        let now_secs = Utc::now().timestamp().max(0) as u64;

        if let Some(record) = self.cache.get(&key).await {
            if let Some(until) = record.locked_until_secs {
                if until > now_secs {
                    let remaining = until - now_secs;
                    return Ok(LockoutDecision {
                        is_locked: true,
                        failed_attempts: record.failed_count,
                        max_attempts: record.failed_count,
                        remaining_lockout_secs: Some(remaining),
                    });
                }
            }
            Ok(LockoutDecision {
                is_locked: false,
                failed_attempts: record.failed_count,
                max_attempts: 0,
                remaining_lockout_secs: None,
            })
        } else {
            Ok(LockoutDecision {
                is_locked: false,
                failed_attempts: 0,
                max_attempts: 0,
                remaining_lockout_secs: None,
            })
        }
    }

    async fn record_failure(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
        max_attempts: u32,
        lockout_duration_secs: u64,
    ) -> SecurityResult<LockoutDecision> {
        let key = Self::key(tenant_id, identity, ip);
        let now_secs = Utc::now().timestamp().max(0) as u64;

        let mut record = self.cache.get(&key).await.unwrap_or(AttemptRecord {
            failed_count: 0,
            locked_until_secs: None,
        });

        // Check if currently locked
        if let Some(until) = record.locked_until_secs {
            if until > now_secs {
                let remaining = until - now_secs;
                return Ok(LockoutDecision {
                    is_locked: true,
                    failed_attempts: record.failed_count,
                    max_attempts,
                    remaining_lockout_secs: Some(remaining),
                });
            }
        }

        record.failed_count += 1;

        if record.failed_count >= max_attempts {
            let until = now_secs + lockout_duration_secs;
            record.locked_until_secs = Some(until);
            self.cache.insert(key, record.clone()).await;

            Ok(LockoutDecision {
                is_locked: true,
                failed_attempts: record.failed_count,
                max_attempts,
                remaining_lockout_secs: Some(lockout_duration_secs),
            })
        } else {
            self.cache.insert(key, record.clone()).await;

            Ok(LockoutDecision {
                is_locked: false,
                failed_attempts: record.failed_count,
                max_attempts,
                remaining_lockout_secs: None,
            })
        }
    }

    async fn record_success(
        &self,
        tenant_id: Uuid,
        identity: &str,
        ip: &IpAddr,
    ) -> SecurityResult<()> {
        let key = Self::key(tenant_id, identity, ip);
        self.cache.invalidate(&key).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_brute_force_lockout_flow() {
        let tracker = MokaBruteForceTracker::default();
        let tenant_id = Uuid::new_v4();
        let ip: IpAddr = "192.168.1.50".parse().unwrap();
        let user = "admin@example.com";

        // Initial check: not locked
        let d = tracker.check_lockout(tenant_id, user, &ip).await.unwrap();
        assert!(!d.is_locked);
        assert_eq!(d.failed_attempts, 0);

        // Attempt 1 fails (max 3)
        let d1 = tracker
            .record_failure(tenant_id, user, &ip, 3, 900)
            .await
            .unwrap();
        assert!(!d1.is_locked);
        assert_eq!(d1.failed_attempts, 1);

        // Attempt 2 fails
        let d2 = tracker
            .record_failure(tenant_id, user, &ip, 3, 900)
            .await
            .unwrap();
        assert!(!d2.is_locked);
        assert_eq!(d2.failed_attempts, 2);

        // Attempt 3 fails -> Lockout triggered!
        let d3 = tracker
            .record_failure(tenant_id, user, &ip, 3, 900)
            .await
            .unwrap();
        assert!(d3.is_locked);
        assert_eq!(d3.failed_attempts, 3);
        assert!(d3.remaining_lockout_secs.is_some());

        // Subsequent check verifies lockout is active
        let d_check = tracker.check_lockout(tenant_id, user, &ip).await.unwrap();
        assert!(d_check.is_locked);

        // Successful login clears the lockout
        tracker.record_success(tenant_id, user, &ip).await.unwrap();
        let d_after = tracker.check_lockout(tenant_id, user, &ip).await.unwrap();
        assert!(!d_after.is_locked);
        assert_eq!(d_after.failed_attempts, 0);
    }
}
