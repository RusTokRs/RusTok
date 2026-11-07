use std::time::Instant;
use uuid::Uuid;
use rustok_core::SecurityContext;
use crate::error::{ForumError, ForumResult};
use super::rbac::enforce_scope;
use rustok_api::{Action, Resource};

/// Generic fence for all Forum reconciliation reports (P2#16).
///
/// Every report is tenant-scoped, bounded by `requested_limit`, and
/// requires both `ForumCategories::Manage` + `ForumTopics::Manage` so
/// future non-GraphQL adapters cannot bypass the operator boundary.
/// Implementors only supply `collect` — everything else (metrics,
/// scope check, limit normalization) lives here once.
pub struct ReconciliationExecutor {
    pub operation: &'static str,
    pub default_limit: u64,
    pub max_limit: u64,
}

impl ReconciliationExecutor {
    pub const fn new(operation: &'static str, default_limit: u64, max_limit: u64) -> Self {
        Self { operation, default_limit, max_limit }
    }

    pub fn effective_limit(&self, requested: Option<u64>) -> u64 {
        let v = requested.unwrap_or(self.default_limit);
        v.clamp(1, self.max_limit)
    }

    pub fn enforce_operator(&self, security: &SecurityContext) -> ForumResult<()> {
        // Operator must hold both category and topic Manage
        enforce_scope(security, Resource::ForumCategories, Action::Manage)?;
        enforce_scope(security, Resource::ForumTopics, Action::Manage)?;
        Ok(())
    }

    pub async fn execute<F, T, Fut>(&self, security: &SecurityContext, f: F) -> ForumResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = ForumResult<T>>,
    {
        rustok_telemetry::metrics::record_module_entrypoint_call("forum", self.operation, "library");
        let started = Instant::now();
        let res = match self.enforce_operator(security) {
            Ok(()) => f().await,
            Err(e) => Err(e),
        };
        rustok_telemetry::metrics::record_span_duration(self.operation, started.elapsed().as_secs_f64());
        if res.is_err() {
            rustok_telemetry::metrics::record_span_error(self.operation, "owner_report");
            rustok_telemetry::metrics::record_module_error("forum", self.operation, "error");
        }
        res
    }
}

// Convenience for topic/category dual-cursor reports (counter, subscription, etc.)
// Keeps `topic_after` / `category_after` keyset logic in one place.
