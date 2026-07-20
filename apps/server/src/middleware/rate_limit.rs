use std::sync::Arc;
use std::time::Duration;

// The preserved implementation still contains the former public cleanup loop.
// The wrapper below is the only runtime entrypoint; allow that compatibility
// function to remain byte-for-byte in the private base module without creating
// a workspace-level dead-code warning.
#[allow(dead_code)]
#[path = "rate_limit_base.rs"]
mod base;

pub use base::{
    PathRateLimitMiddlewareState, PathRateLimitPolicy, RateLimitCheckError, RateLimitConfig,
    RateLimitExceeded, RateLimitInfo, RateLimitMiddlewareState, RateLimitStats, RateLimiter,
    SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter, SharedSearchRateLimiter,
    extract_client_id_pub, rate_limit_for_paths, rate_limit_middleware,
};

fn cleanup_task_has_external_owners(limiter: &Arc<RateLimiter>) -> bool {
    Arc::strong_count(limiter) > 1
}

/// Run periodic Moka maintenance while the rate limiter is owned by the runtime.
///
/// The spawned maintenance task holds one `Arc` itself. Once every context,
/// middleware state and request clone has been dropped, that task-owned reference
/// is the only remaining strong reference and the worker exits instead of keeping
/// the complete limiter/cache alive forever.
pub async fn cleanup_task(limiter: Arc<RateLimiter>) {
    let mut interval = tokio::time::interval(Duration::from_secs(300));

    loop {
        interval.tick().await;
        if !cleanup_task_has_external_owners(&limiter) {
            tracing::debug!(
                namespace = limiter.namespace(),
                "Rate limit cleanup worker released its final runtime-owned limiter"
            );
            return;
        }
        limiter.cleanup_expired().await;
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;

    #[test]
    fn cleanup_worker_detects_when_only_its_own_arc_remains() {
        let limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));
        let worker_limiter = Arc::clone(&limiter);
        assert!(cleanup_task_has_external_owners(&worker_limiter));

        drop(limiter);

        assert!(!cleanup_task_has_external_owners(&worker_limiter));
    }
}
