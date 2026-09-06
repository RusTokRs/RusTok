/// Resilience patterns for fault-tolerant systems
///
/// This module provides:
/// - Circuit Breaker: Prevent cascading failures
/// - Retry: Automatic retry with backoff
/// - Timeout: Enforce operation deadlines
/// - Bulkhead: Isolate resources
pub mod bulkhead;
pub mod circuit_breaker;
pub mod retry;
pub mod timeout;

pub use bulkhead::{Bulkhead, BulkheadConfig, BulkheadError, BulkheadStats};
pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerError, CircuitState,
};
pub use retry::{RetryPolicy, RetryStrategy};
pub use timeout::with_timeout;
