use std::time::Instant;

use rustok_api::{Action, Resource};
use rustok_core::SecurityContext;

use super::rbac::enforce_scope;
use crate::error::ForumResult;

/// The shared fence of the Forum reconciliation reports (P2#16).
///
/// Each report declares one executor with its metric labels and page bounds.
/// `execute` records the entry-point metrics and requires both
/// `ForumCategories::Manage` and `ForumTopics::Manage` before it runs the report
/// body, so a future non-GraphQL adapter cannot bypass the operator boundary.
/// `effective_limit` is the only normalization of a requested page size.
///
/// The three labels are separate fields because they are separate, already
/// published series: dashboards key the entry-point counter and the module error
/// counter on the short names, and the span metrics on the `forum.`-prefixed one.
pub struct ReconciliationExecutor {
    /// `operation` label of the span duration and span error metrics.
    pub operation: &'static str,
    /// `entry_point` label of the module entry-point counter.
    pub entrypoint: &'static str,
    /// `error_type` label of the module error counter.
    pub error_type: &'static str,
    pub default_limit: u64,
    pub max_limit: u64,
}

impl ReconciliationExecutor {
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
        rustok_telemetry::metrics::record_module_entrypoint_call(
            "forum",
            self.entrypoint,
            "library",
        );
        let started = Instant::now();
        let res = match self.enforce_operator(security) {
            Ok(()) => f().await,
            Err(e) => Err(e),
        };
        rustok_telemetry::metrics::record_span_duration(
            self.operation,
            started.elapsed().as_secs_f64(),
        );
        if res.is_err() {
            rustok_telemetry::metrics::record_span_error(self.operation, "owner_report");
            rustok_telemetry::metrics::record_module_error("forum", self.error_type, "error");
        }
        res
    }
}
