---
id: doc://docs/tenant-cache-stampede-protection.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Tenant Cache Stampede Protection

## Overview

This document describes the implementation of cache stampede protection for the tenant resolution middleware in RusToK.

## Problem Statement

In multi-tenant systems, the tenant resolver middleware is a critical component that executes on every request. Without proper cache stampede protection, when a cache miss occurs (e.g., after cache invalidation or cold start), multiple concurrent requests for the same tenant will all attempt to query the database simultaneously.

**Impact:**
- 1000 concurrent requests = 1000 identical SELECT queries to the database
- Database connection pool exhaustion
- Increased latency for all requests
- Potential cascade failures during cache invalidation events

## Solution: Singleflight / Request Coalescing Pattern

The singleflight pattern (also known as request coalescing) ensures that when multiple concurrent requests need the same uncached data, only **one** request performs the expensive operation (database query), while all other requests wait for and share the result.

## Implementation

### Architecture

```
Request 1 ─┐
Request 2 ─┤
Request 3 ─┼──> Cache Miss ──> Singleflight Gate ──> [1 DB Query] ──> Cache Write ──> All Requests
Request 4 ─┤                         ↓
Request N ─┘                    Other requests wait here
```

### Key Components

#### 1. In-Flight Request Tracking

```rust
in_flight: Arc<Mutex<HashMap<String, Arc<Notify>>>>
```

- Tracks active database queries by cache key
- Uses `tokio::sync::Notify` for efficient async coordination
- Automatically cleaned up after query completion

#### 2. Coalescing Logic

The `get_or_load_with_coalescing` method implements the core pattern:

```rust
async fn get_or_load_with_coalescing<F, Fut>(
    &self,
    cache_key: &str,
    loader: F,
) -> Result<TenantContext, StatusCode>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<Option<TenantContext>, StatusCode>>,
```

**Flow:**
1. Acquire lock on `in_flight` map
2. Check if a request for this key is already in-flight
   - If yes: Wait for notification, then check cache
   - If no: Insert new Notify, proceed with DB query
3. Execute DB query (only first request)
4. Cache result and notify all waiting requests
5. Clean up in-flight entry

#### 3. Metrics

A new metric `coalesced_requests` tracks the effectiveness of the pattern:

```rust
pub struct TenantCacheStats {
    // ... existing metrics
    pub coalesced_requests: u64,  // Number of requests that waited instead of querying
}
```

**Monitoring:**
```bash
# Check coalescing effectiveness
curl http://localhost:3000/metrics | grep tenant_cache_coalesced

```

## Usage

The stampede protection is automatically enabled when using the tenant middleware:

```rust
use rustok_server::middleware::tenant;

// Initialize tenant cache infrastructure (includes stampede protection)
tenant::init_tenant_cache_infrastructure(&ctx).await;

// Use middleware
let app = Router::new()
    .route("/api/...", ...)
    .layer(middleware::from_fn_with_state(
        ctx.clone(),
        tenant::resolve,  // Automatically includes coalescing
    ));
```

## Edge Cases & Considerations

### 1. Error Handling

If the database query fails, the error is propagated to all waiting requests:
- No cached error state
- All requests receive the same error
- Next attempt will retry the query

### 2. Race Conditions

The implementation handles race conditions gracefully:
- Lock is held only briefly to check/insert
- Notify mechanism ensures all waiters are woken
- Loop structure handles cache still being empty after notification

### 3. Lock Contention

The `Mutex` on the in-flight map is held for minimal time:
- Check existence: ~1μs
- Insert notify: ~1μs
- DB query happens **outside** the lock

**Bottleneck:** Not a concern even at high concurrency

### 4. Memory Cleanup

In-flight entries are automatically removed:
- After successful query
- After error
- No memory leaks

## Monitoring & Alerts

### Key Metrics

```promql
# High coalescing rate indicates stampede protection is working
rate(tenant_cache_coalesced_requests[5m]) > 100

# Alert if coalescing rate is low but misses are high
rate(tenant_cache_misses[5m]) > 10 AND rate(tenant_cache_coalesced_requests[5m]) < 10
```

### Dashboard Panels

1. **Cache Hit Rate:** `hits / (hits + misses)`
2. **Coalescing Effectiveness:** `coalesced_requests / (coalesced_requests + misses)`
3. **Database Query Load:** `rate(tenant_db_queries[5m])`

## References

- [ARCHITECTURE_RECOMMENDATIONS.md](../ARCHITECTURE_RECOMMENDATIONS.md#14-tenant-cache-stampede-protection)
- [Singleflight Pattern (Go)](https://pkg.go.dev/golang.org/x/sync/singleflight)
- [Request Coalescing at Cloudflare](https://blog.cloudflare.com/introducing-request-coalescing/)

## Related Issues

- Issue #4: Tenant Cache Stampede Protection ✅ **COMPLETE**
- Issue #1: Event Schema Versioning ✅ **COMPLETE**
- Issue #2: Transactional Event Publishing ✅ **COMPLETE**
- Issue #3: Test Utilities Crate ✅ **COMPLETE**

---

**Last Updated:** February 11, 2026  
**Status:** ✅ Implemented
