---
id: doc://docs/guides/circuit-breaker.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Circuit Breaker Pattern Guide

## Overview

Circuit Breaker is a resilience pattern that prevents cascading failures by failing fast when a service is unavailable.

## Benefits

### 🚀 Performance
- **Fail-fast**: Instead of waiting 30s for timeout, fail in <1ms
- **Latency reduction**: 30s → 0.1ms = **99.997% improvement**
- **Resource protection**: Don't waste connections/threads on dead services

### 🛡️ Stability
- **Prevents cascade failures**: One service failure doesn't bring down entire system
- **Automatic recovery**: Tests service periodically, closes circuit when healthy
- **Graceful degradation**: Return cached/default values when service unavailable

### 📊 Observability
- **State tracking**: Closed/Open/HalfOpen
- **Metrics**: Success rate, rejection rate, state transitions
- **Logging**: Detailed state changes and failures

## States

```
┌────────┐  failure_threshold  ┌──────┐  timeout  ┌───────────┐
│ Closed ├────────────────────→│ Open ├──────────→│ Half-Open │
└────┬───┘                      └──────┘           └─────┬─────┘
     │                                                    │
     │                                  success_threshold │
     └────────────────────────────────────────────────────┘
```

### Closed (Normal Operation)
- All requests pass through
- Success resets failure counter
- After N failures → Open

### Open (Service Down)
- All requests fail immediately (fail-fast)
- After timeout → HalfOpen

### HalfOpen (Testing Recovery)
- Limited requests pass through
- Success → Close circuit
- Failure → Reopen circuit

## Basic Usage

### 1. Create Circuit Breaker

```rust
use rustok_core::{CircuitBreaker, CircuitBreakerConfig};
use std::time::Duration;

let breaker = CircuitBreaker::new(CircuitBreakerConfig {
    failure_threshold: 5,      // Open after 5 failures
    success_threshold: 2,      // Close after 2 successes in half-open
    timeout: Duration::from_secs(60), // Wait 60s before testing recovery
    half_open_max_requests: Some(3),  // Max 3 requests in half-open
});
```

### 2. Wrap External Calls

```rust
// Example: Database query with circuit breaker
let result = breaker.call(|| async {
    sqlx::query("SELECT * FROM tenants WHERE id = ?")
        .bind(tenant_id)
        .fetch_one(&pool)
        .await
}).await;

match result {
    Ok(tenant) => {
        // Success - use tenant
    }
    Err(CircuitBreakerError::Open) => {
        // Circuit is open, service unavailable
        // Return cached data or default
        return cached_tenant.or(default_tenant);
    }
    Err(CircuitBreakerError::Execution(e)) => {
        // Actual database error
        tracing::error!("Database error: {}", e);
    }
}
```

## Use Cases

### 1. Database Connection Protection

```rust
use rustok_core::CircuitBreaker;
use std::sync::Arc;

pub struct ProtectedDatabase {
    pool: sqlx::PgPool,
    breaker: Arc<CircuitBreaker>,
}

impl ProtectedDatabase {
    pub async fn query_tenant(&self, id: Uuid) -> Result<Tenant, AppError> {
        self.breaker.call(|| async {
            sqlx::query_as::<_, Tenant>("SELECT * FROM tenants WHERE id = ?")
                .bind(id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| match e {
            CircuitBreakerError::Open => AppError::ServiceUnavailable,
            CircuitBreakerError::Execution(e) => AppError::Database(e),
        })
    }
}
```

### 2. Redis Cache Protection

```rust
use rustok_core::{CircuitBreaker, RedisCacheBackend};

pub struct ProtectedRedisCache {
    cache: RedisCacheBackend,
    breaker: Arc<CircuitBreaker>,
}

impl ProtectedRedisCache {
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CircuitBreakerError> {
        self.breaker.call(|| async {
            self.cache.get(key)
                .await
                .map_err(|e| e.to_string())
        }).await
    }
    
    pub async fn set(&self, key: String, value: Vec<u8>) -> Result<(), CircuitBreakerError> {
        self.breaker.call(|| async {
            self.cache.set(key, value)
                .await
                .map_err(|e| e.to_string())
        }).await
    }
}
```

### 3. External API Protection

```rust
use reqwest::Client;
use rustok_core::CircuitBreaker;

pub struct ExternalApiClient {
    client: Client,
    breaker: Arc<CircuitBreaker>,
}

impl ExternalApiClient {
    pub async fn call_api(&self, endpoint: &str) -> Result<Response, CircuitBreakerError> {
        self.breaker.call(|| async {
            self.client
                .get(endpoint)
                .send()
                .await
                .map_err(|e| e.to_string())
        }).await
    }
}
```

## Advanced: Combining with Retry

```rust
use rustok_core::{CircuitBreaker, RetryPolicy, RetryStrategy};
use std::time::Duration;

let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());

let retry_policy = RetryPolicy {
    max_attempts: 3,
    strategy: RetryStrategy::Exponential {
        base: Duration::from_millis(100),
        max: Duration::from_secs(5),
    },
    retryable_predicate: Some(|err| {
        // Retry only on transient errors
        err.contains("timeout") || err.contains("connection")
    }),
};

// Combine: retry with circuit breaker
let result = retry_policy.execute(|| async {
    breaker.call(|| async {
        external_service.call().await
    }).await
}).await;
```

## Monitoring

### Get Statistics

```rust
let stats = breaker.stats().await;

println!("State: {:?}", stats.state);
println!("Success Rate: {:.2}%", stats.success_rate() * 100.0);
println!("Rejection Rate: {:.2}%", stats.rejection_rate() * 100.0);
println!("Total Requests: {}", stats.total_requests);
println!("State Transitions: {}", stats.state_transitions);
```

### Prometheus Metrics Export

The circuit breaker provides built-in Prometheus-compatible metrics export:

```rust
// Export metrics in Prometheus exposition format
let metrics = breaker.export_prometheus_metrics("redis_cache").await;
println!("{}", metrics);
```

This outputs:
```text
# HELP circuit_breaker_state Current state of the circuit breaker (0=closed, 1=open, 2=half_open)
# TYPE circuit_breaker_state gauge
circuit_breaker_state{name="redis_cache"} 0
# HELP circuit_breaker_requests_total Total number of requests
# TYPE circuit_breaker_requests_total counter
circuit_breaker_requests_total{name="redis_cache"} 150
# HELP circuit_breaker_successes_total Total number of successful requests
# TYPE circuit_breaker_successes_total counter
circuit_breaker_successes_total{name="redis_cache"} 145
# HELP circuit_breaker_failures_total Total number of failed requests
# TYPE circuit_breaker_failures_total counter
circuit_breaker_failures_total{name="redis_cache"} 5
# HELP circuit_breaker_rejected_total Total number of rejected requests (circuit open)
# TYPE circuit_breaker_rejected_total counter
circuit_breaker_rejected_total{name="redis_cache"} 0
# HELP circuit_breaker_state_transitions_total Total number of state transitions
# TYPE circuit_breaker_state_transitions_total counter
circuit_breaker_state_transitions_total{name="redis_cache"} 2
# HELP circuit_breaker_success_rate Current success rate (0.0 - 1.0)
# TYPE circuit_breaker_success_rate gauge
circuit_breaker_success_rate{name="redis_cache"} 0.9667
# HELP circuit_breaker_rejection_rate Current rejection rate (0.0 - 1.0)
# TYPE circuit_breaker_rejection_rate gauge
circuit_breaker_rejection_rate{name="redis_cache"} 0.0
```

#### Available Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `circuit_breaker_state` | gauge | Current state (0=closed, 1=open, 2=half_open) |
| `circuit_breaker_requests_total` | counter | Total number of requests |
| `circuit_breaker_successes_total` | counter | Total number of successful requests |
| `circuit_breaker_failures_total` | counter | Total number of failed requests |
| `circuit_breaker_rejected_total` | counter | Total rejected requests (circuit open) |
| `circuit_breaker_state_transitions_total` | counter | Total state transitions |
| `circuit_breaker_success_rate` | gauge | Current success rate (0.0 - 1.0) |
| `circuit_breaker_rejection_rate` | gauge | Current rejection rate (0.0 - 1.0) |

All metrics include a `name` label for identification.

## Manual Control

### Force Open (Maintenance)

```rust
// Manually open circuit for maintenance
breaker.open().await;

// Perform maintenance...

// Close circuit when done
breaker.close().await;
```

### Reset Statistics

```rust
// Reset all counters and close circuit
breaker.reset().await;
```

## Configuration Guidelines

### Conservative (Default)
```rust
CircuitBreakerConfig {
    failure_threshold: 5,    // Open after 5 failures
    success_threshold: 2,    // Close after 2 successes
    timeout: Duration::from_secs(60),  // 1 minute
    half_open_max_requests: Some(3),
}
```

### Aggressive (Fast Recovery)
```rust
CircuitBreakerConfig {
    failure_threshold: 3,    // Open after 3 failures
    success_threshold: 1,    // Close after 1 success
    timeout: Duration::from_secs(10),  // 10 seconds
    half_open_max_requests: Some(5),
}
```

### Strict (Production Critical)
```rust
CircuitBreakerConfig {
    failure_threshold: 10,   // Open after 10 failures
    success_threshold: 5,    // Close after 5 successes
    timeout: Duration::from_secs(120), // 2 minutes
    half_open_max_requests: Some(2),
}
```

## Best Practices

### ✅ DO
- Use circuit breaker for **all external dependencies**
- Configure **appropriate thresholds** based on SLA
- **Log state transitions** for debugging
- **Monitor metrics** (success rate, rejection rate)
- Provide **fallback values** when circuit is open
- Test circuit breaker behavior in **staging**

### ❌ DON'T
- Don't use for internal, reliable services
- Don't set threshold too low (flaky circuit)
- Don't set timeout too short (can't recover)
- Don't ignore CircuitBreakerError::Open (handle gracefully)
- Don't share breaker across unrelated services

## Testing

### Unit Test: State Transitions

```rust
#[tokio::test]
async fn test_circuit_breaker_states() {
    let breaker = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 2,
        success_threshold: 1,
        timeout: Duration::from_millis(100),
        ..Default::default()
    });
    
    // Closed -> Open
    for _ in 0..2 {
        let _ = breaker.call(|| async { Err::<(), _>("error") }).await;
    }
    assert_eq!(breaker.get_state().await, CircuitState::Open);
    
    // Open -> HalfOpen (after timeout)
    tokio::time::sleep(Duration::from_millis(150)).await;
    let _ = breaker.call(|| async { Ok::<_, String>(()) }).await;
    
    // HalfOpen -> Closed (on success)
    assert_eq!(breaker.get_state().await, CircuitState::Closed);
}
```

### Integration Test: Real Service

```rust
#[tokio::test]
async fn test_database_circuit_breaker() {
    let pool = PgPool::connect("postgres://...").await.unwrap();
    let breaker = CircuitBreaker::new(CircuitBreakerConfig::default());
    
    // Simulate database outage
    drop(pool);
    
    // Circuit should open after failures
    for _ in 0..5 {
        let _ = breaker.call(|| async {
            sqlx::query("SELECT 1").fetch_one(&pool).await
        }).await;
    }
    
    assert_eq!(breaker.get_state().await, CircuitState::Open);
}
```

## Performance Impact

### Without Circuit Breaker
```
Service Down → Request → Wait 30s → Timeout → Return Error
Latency: 30,000ms per request
100 requests = 3,000,000ms (50 minutes!)
```

### With Circuit Breaker
```
Service Down → Circuit Open → Fail Fast → Return Error
Latency: 0.1ms per request
100 requests = 10ms (instant!)
```

**Improvement: 99.9997% faster fail-fast**

## Troubleshooting

### Circuit constantly opening/closing
- **Cause**: Threshold too low or service flaky
- **Fix**: Increase failure_threshold, add retry logic

### Circuit stays open too long
- **Cause**: Timeout too long
- **Fix**: Reduce timeout, increase half_open_max_requests

### False positives (circuit opens on transient errors)
- **Cause**: Transient errors counted as failures
- **Fix**: Add retry before circuit breaker

### Circuit never closes
- **Cause**: Success threshold too high or service still down
- **Fix**: Check service health, lower success_threshold

## References

- [Martin Fowler: Circuit Breaker](https://martinfowler.com/bliki/CircuitBreaker.html)
- [Microsoft: Circuit Breaker Pattern](https://docs.microsoft.com/en-us/azure/architecture/patterns/circuit-breaker)
- [Resilience4j Documentation](https://resilience4j.readme.io/docs/circuitbreaker)

---

**Implementation:** `crates/rustok-core/src/resilience/circuit_breaker.rs`  
**Status:** ✅ Production Ready  
**Version:** 1.0  
**Last Updated:** 2026-02-12
