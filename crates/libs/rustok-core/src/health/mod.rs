//! Health Check System
//!
//! Provides comprehensive health monitoring for RusToK modules and services.
//!
//! # Features
//!
//! - **Multiple Check Types**: Liveness, readiness, and deep health checks
//! - **Configurable Timeouts**: Per-check timeout configuration
//! - **Caching**: Results cached to prevent check flooding
//! - **Metrics Integration**: Automatic metrics collection
//! - **Graceful Degradation**: Support for degraded states
//!
//! # Example
//!
//! ```ignore
//! use rustok_core::health::{HealthRegistry, HealthCheck, HealthResult, HealthStatus};
//! use async_trait::async_trait;
//!
//! struct DatabaseHealthCheck {
//!     pool: DatabasePool,
//! }
//!
//! #[async_trait]
//! impl HealthCheck for DatabaseHealthCheck {
//!     fn name(&self) -> &str {
//!         "database"
//!     }
//!
//!     async fn check(&self) -> HealthResult {
//!         match self.ping().await {
//!             Ok(_) => HealthResult::healthy("database"),
//!             Err(e) => HealthResult::unhealthy("database", &e.to_string()),
//!         }
//!     }
//! }
//!
//! // Register and run checks
//! let mut registry = HealthRegistry::new();
//! registry.register(DatabaseHealthCheck { pool });
//!
//! let health = registry.check_all().await;
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;

pub use crate::module::HealthStatus;

/// Result of a health check
#[derive(Debug, Clone)]
pub struct HealthResult {
    /// Name of the component checked
    pub name: String,
    /// Current health status
    pub status: HealthStatus,
    /// Human-readable message
    pub message: Option<String>,
    /// Latency of the check in milliseconds
    pub latency_ms: u64,
    /// Timestamp of the check
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl HealthResult {
    /// Create a healthy result
    pub fn healthy(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Healthy,
            message: None,
            latency_ms: 0,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create an unhealthy result
    pub fn unhealthy(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Unhealthy,
            message: Some(message.into()),
            latency_ms: 0,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a degraded result
    pub fn degraded(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: HealthStatus::Degraded,
            message: Some(message.into()),
            latency_ms: 0,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Add latency information
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency_ms = latency.as_millis() as u64;
        self
    }
}

/// Trait for implementing health checks
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// Unique name of this health check
    fn name(&self) -> &str;

    /// Perform the health check
    async fn check(&self) -> HealthResult;

    /// Timeout for this check (default: 5 seconds)
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    /// Whether this check is critical (default: true)
    /// Non-critical checks failing don't affect overall status
    fn critical(&self) -> bool {
        true
    }
}

/// Overall system health
#[derive(Debug, Clone)]
pub struct OverallHealth {
    /// Aggregated health status
    pub status: HealthStatus,
    /// Results from all checks
    pub checks: Vec<HealthResult>,
    /// Total latency of all checks
    pub total_latency_ms: u64,
    /// Timestamp of the check
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl OverallHealth {
    /// Check if the system is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.status, HealthStatus::Healthy)
    }

    /// Check if the system is ready (healthy or degraded)
    pub fn is_ready(&self) -> bool {
        !matches!(self.status, HealthStatus::Unhealthy)
    }

    /// Get failed checks
    pub fn failures(&self) -> Vec<&HealthResult> {
        self.checks
            .iter()
            .filter(|c| matches!(c.status, HealthStatus::Unhealthy))
            .collect()
    }

    /// Get degraded checks
    pub fn degraded(&self) -> Vec<&HealthResult> {
        self.checks
            .iter()
            .filter(|c| matches!(c.status, HealthStatus::Degraded))
            .collect()
    }
}

/// Health check registry
pub struct HealthRegistry {
    checks: Vec<Arc<dyn HealthCheck>>,
    cache: Arc<RwLock<HashMap<String, (HealthResult, Instant)>>>,
    cache_ttl: Duration,
}

impl HealthRegistry {
    /// Create a new health registry
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(5),
        }
    }

    /// Create with custom cache TTL
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    /// Register a health check
    pub fn register(&mut self, check: impl HealthCheck + 'static) {
        self.checks.push(Arc::new(check));
    }

    /// Run all health checks
    pub async fn check_all(&self) -> OverallHealth {
        let start = Instant::now();
        let mut checks = Vec::new();

        for check in &self.checks {
            let check_start = Instant::now();

            let result = match tokio::time::timeout(check.timeout(), check.check()).await {
                Ok(result) => result.with_latency(check_start.elapsed()),
                Err(_) => HealthResult::unhealthy(
                    check.name(),
                    format!("Health check timed out after {:?}", check.timeout()),
                )
                .with_latency(check_start.elapsed()),
            };

            checks.push(result);
        }

        // Determine overall status
        let status = if checks.iter().any(|c| {
            matches!(c.status, HealthStatus::Unhealthy)
                && self
                    .checks
                    .iter()
                    .any(|hc| hc.name() == c.name && hc.critical())
        }) {
            HealthStatus::Unhealthy
        } else if checks
            .iter()
            .any(|c| matches!(c.status, HealthStatus::Degraded))
        {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        OverallHealth {
            status,
            checks,
            total_latency_ms: start.elapsed().as_millis() as u64,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Run a specific health check by name
    pub async fn check_one(&self, name: &str) -> Option<HealthResult> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some((result, timestamp)) = cache.get(name)
                && timestamp.elapsed() < self.cache_ttl
            {
                return Some(result.clone());
            }
        }

        // Run the check
        for check in &self.checks {
            if check.name() == name {
                let start = Instant::now();
                let result = match tokio::time::timeout(check.timeout(), check.check()).await {
                    Ok(result) => result.with_latency(start.elapsed()),
                    Err(_) => HealthResult::unhealthy(
                        check.name(),
                        format!("Health check timed out after {:?}", check.timeout()),
                    )
                    .with_latency(start.elapsed()),
                };

                // Update cache
                let mut cache = self.cache.write().await;
                cache.insert(name.to_string(), (result.clone(), Instant::now()));

                return Some(result);
            }
        }

        None
    }

    /// Get all registered check names
    pub fn check_names(&self) -> Vec<&str> {
        self.checks.iter().map(|c| c.name()).collect()
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

impl Default for HealthRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Built-in health checks
pub mod checks {
    use super::*;

    /// Database connectivity check
    pub struct DatabaseHealthCheck {
        name: String,
        check_fn:
            Box<dyn Fn() -> futures::future::BoxFuture<'static, Result<(), String>> + Send + Sync>,
    }

    impl DatabaseHealthCheck {
        /// Create from a database pool
        pub fn new<F, Fut>(name: impl Into<String>, check_fn: F) -> Self
        where
            F: Fn() -> Fut + Send + Sync + 'static,
            Fut: futures::Future<Output = Result<(), String>> + Send + 'static,
        {
            Self {
                name: name.into(),
                check_fn: Box::new(move || Box::pin(check_fn())),
            }
        }
    }

    #[async_trait]
    impl HealthCheck for DatabaseHealthCheck {
        fn name(&self) -> &str {
            &self.name
        }

        async fn check(&self) -> HealthResult {
            match (self.check_fn)().await {
                Ok(_) => HealthResult::healthy(&self.name),
                Err(e) => HealthResult::unhealthy(&self.name, e),
            }
        }
    }

    /// Simple health check from a closure
    pub struct FnHealthCheck {
        name: String,
        check_fn: Box<dyn Fn() -> futures::future::BoxFuture<'static, HealthResult> + Send + Sync>,
        critical: bool,
    }

    impl FnHealthCheck {
        /// Create from an async function
        pub fn new<F, Fut>(name: impl Into<String>, check_fn: F) -> Self
        where
            F: Fn() -> Fut + Send + Sync + 'static,
            Fut: futures::Future<Output = HealthResult> + Send + 'static,
        {
            Self {
                name: name.into(),
                check_fn: Box::new(move || Box::pin(check_fn())),
                critical: true,
            }
        }

        /// Mark as non-critical
        pub fn non_critical(mut self) -> Self {
            self.critical = false;
            self
        }
    }

    #[async_trait]
    impl HealthCheck for FnHealthCheck {
        fn name(&self) -> &str {
            &self.name
        }

        async fn check(&self) -> HealthResult {
            (self.check_fn)().await
        }

        fn critical(&self) -> bool {
            self.critical
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCheck {
        name: String,
        result: HealthResult,
    }

    #[async_trait]
    impl HealthCheck for MockCheck {
        fn name(&self) -> &str {
            &self.name
        }

        async fn check(&self) -> HealthResult {
            self.result.clone()
        }
    }

    #[tokio::test]
    async fn test_healthy_registry() {
        let mut registry = HealthRegistry::new();
        registry.register(MockCheck {
            name: "test".to_string(),
            result: HealthResult::healthy("test"),
        });

        let health = registry.check_all().await;
        assert!(health.is_healthy());
        assert!(health.is_ready());
        assert_eq!(health.checks.len(), 1);
    }

    #[tokio::test]
    async fn test_unhealthy_registry() {
        let mut registry = HealthRegistry::new();
        registry.register(MockCheck {
            name: "test".to_string(),
            result: HealthResult::unhealthy("test", "failed"),
        });

        let health = registry.check_all().await;
        assert!(!health.is_healthy());
        assert!(!health.is_ready());
        assert_eq!(health.failures().len(), 1);
    }

    #[tokio::test]
    async fn test_degraded_registry() {
        let mut registry = HealthRegistry::new();
        registry.register(MockCheck {
            name: "test".to_string(),
            result: HealthResult::degraded("test", "slow"),
        });

        let health = registry.check_all().await;
        assert!(!health.is_healthy());
        assert!(health.is_ready()); // Degraded is still ready
        assert_eq!(health.status, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn test_check_one() {
        let mut registry = HealthRegistry::new();
        registry.register(MockCheck {
            name: "test".to_string(),
            result: HealthResult::healthy("test"),
        });

        let result = registry.check_one("test").await;
        assert!(result.is_some());
        assert!(matches!(result.unwrap().status, HealthStatus::Healthy));

        let result = registry.check_one("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_non_critical_check() {
        struct NonCriticalCheck;

        #[async_trait]
        impl HealthCheck for NonCriticalCheck {
            fn name(&self) -> &str {
                "non-critical"
            }

            async fn check(&self) -> HealthResult {
                HealthResult::unhealthy("non-critical", "failed")
            }

            fn critical(&self) -> bool {
                false
            }
        }

        let mut registry = HealthRegistry::new();
        registry.register(NonCriticalCheck);

        let health = registry.check_all().await;
        // Non-critical failure doesn't make system unhealthy
        assert!(health.is_healthy());
    }
}
