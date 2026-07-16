use std::sync::Arc;
use std::time::Duration;

#[path = "rate_limit_base.rs"]
mod base;

pub use base::{
    extract_client_id_pub, rate_limit_for_paths, rate_limit_middleware, PathRateLimitMiddlewareState,
    PathRateLimitPolicy, RateLimitCheckError, RateLimitConfig, RateLimitExceeded, RateLimitInfo,
    RateLimitMiddlewareState, RateLimitStats, RateLimiter, SharedApiRateLimiter,
    SharedAuthRateLimiter, SharedOAuthRateLimiter, SharedSearchRateLimiter,
};

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
        if Arc::strong_count(&limiter) == 1 {
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

    #[tokio::test(start_paused = true)]
    async fn cleanup_worker_exits_after_external_owners_are_dropped() {
        let limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));
        let worker_limiter = Arc::clone(&limiter);
        let worker = tokio::spawn(cleanup_task(worker_limiter));

        tokio::task::yield_now().await;
        assert!(!worker.is_finished());

        drop(limiter);
        tokio::time::advance(Duration::from_secs(300)).await;
        tokio::task::yield_now().await;

        assert!(worker.is_finished());
        worker.await.unwrap();
    }
}
