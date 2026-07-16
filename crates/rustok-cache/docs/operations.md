# Cache operations and recovery runbook

This runbook owns the operational response for the shared `rustok-cache` capability. Domain-specific rebuilds and generation recovery remain in the owning module runbook.

## Operating principles

1. Redis PubSub is a low-latency invalidation fast path, not a durable queue.
2. A healthy bounded local fallback does not make a configured Redis backend healthy.
3. A generation bump or persisted cursor is the durable recovery boundary. Do not acknowledge recovery before the owner-defined clear, rotation or rebuild succeeds.
4. Normal compare-and-set mismatches are optimistic-concurrency outcomes. Compare-and-set failures are backend or runtime errors.
5. Increase cache limits only after measuring payload weight, loader latency and eviction pressure. Larger limits can hide a leak without correcting its cause.

## Prometheus alerts

The canonical rules are in `ops/prometheus/alert_rules.yml`.

| Alert | Default severity | Meaning | First response |
| --- | --- | --- | --- |
| `CacheRedisDegraded` | critical | Redis is configured but client initialization or connectivity is unhealthy for two minutes | Check Redis reachability, credentials/TLS, DNS and circuit-breaker state; keep the pod degraded until connectivity recovers |
| `CacheGenerationBumpFailure` | critical | A shared generation could not advance | Identify the owning mutation, confirm whether its database transaction committed, and do not blindly retry a non-idempotent mutation |
| `CacheInvalidationPublishFailures` | warning | Redis PubSub fast-path publication failed | Confirm the owner has a durable generation/cursor and that reconciliation is advancing; investigate Redis without treating PubSub as the source of truth |
| `CacheRefreshSaturated` | warning | The bounded refresh coordinator rejected work | Inspect loader duration, in-flight refreshes and hot keys; reduce loader latency or isolate hot keys before raising concurrency |
| `CacheCompareAndSetFailures` | warning | Atomic CAS returned an error rather than `Mismatch` | Check Redis/backend health and timeouts; verify fallback did not report a process-local success while the shared primary was unavailable |
| `LowCacheHitRate` / `VeryLowCacheHitRate` | warning / critical | Cache effectiveness is below the current baseline | Check key version churn, TTL changes, generation rotation and source-of-truth latency before increasing TTL |
| `HighCacheEvictionRate` | warning | Capacity pressure is causing frequent eviction | Compare entry weights with configured byte budgets; find oversized values or unexpected identity cardinality |

Thresholds are starting defaults. Change them only with a recorded workload baseline and a rollback criterion.

## Redis degradation triage

1. Confirm `rustok_cache_redis_url_present == 1`.
2. Check `rustok_cache_redis_client_initialized`. A zero value indicates configuration/client construction failure rather than a network outage.
3. Check `rustok_cache_redis_connectivity_healthy` and `rustok_cache_redis_degraded` on every serving replica.
4. Correlate with invalidation publish failures, generation read/bump failures and circuit-breaker metrics.
5. Verify that correctness-sensitive owner caches have durable reconciliation running and visible in readiness.
6. Restore Redis and observe connectivity recovery before changing cache limits or disabling alerts.

Do not convert a configured Redis deployment into silent memory-only operation during an incident. That changes cross-replica consistency semantics.

## Generation or invalidation incident

### Generation bump failure

Treat a bump failure as a correctness incident because replicas cannot safely identify the new epoch.

1. Determine whether the owner transaction committed the generation in the same database transaction.
2. If the transaction did not commit, retry only through the owner’s normal idempotent mutation contract.
3. If domain data committed but the generation did not, follow the owner recovery procedure: reserve a new durable generation or perform a namespace rebuild/full clear.
4. Verify every replica seeds from persisted state before accepting new fast-path invalidations.
5. Confirm the applied/acknowledged generation advances only after recovery succeeds.

### PubSub publication failure

A PubSub failure does not by itself prove stale data if the owner has a durable generation or cursor.

1. Confirm local delivery occurred where applicable.
2. Confirm the durable generation/cursor advanced.
3. Confirm remote reconciliation is running and readiness has not reported a terminal worker.
4. Measure convergence time against the owner’s documented recovery interval.
5. Escalate to critical if the owner has no durable recovery path or reconciliation is not advancing.

### Gap, lag or unverified first event

Never apply only the latest event and assume intermediate invalidations were harmless.

1. Stop acknowledging the stream/generation.
2. Execute the owner-defined full clear, namespace rotation or source-of-truth rebuild.
3. Seed the tracker from persisted state.
4. Resume fast-path events only after recovery completes.
5. Acknowledge the recovered offset/generation last.

## Refresh saturation triage

Use these metrics together:

- `rustok_cache_refresh_started_total`;
- `rustok_cache_refresh_completed_total`;
- `rustok_cache_refresh_failed_total`;
- `rustok_cache_refresh_saturated_total`;
- `rustok_cache_refresh_rejected_total`;
- `rustok_cache_refresh_in_flight`.

Investigate in this order:

1. Long source-of-truth latency or loader timeouts.
2. One or more hot keys repeatedly entering stale refresh.
3. A refresh concurrency limit below the measured normal peak.
4. Callers bypassing single-flight/refresh coordination.
5. Hard-expiry or TTL alignment causing synchronized refresh waves.

Prefer jitter, loader optimization and hot-key isolation over an unconditional concurrency increase.

## CAS triage

- `rustok_cache_cas_mismatch_total` is normally expected when another writer wins.
- `rustok_cache_cas_failed_total` is not an optimistic mismatch and requires investigation.
- A sustained `rustok_cache_cas_in_flight` value with no completions indicates backend latency, cancellation or a stuck runtime.

For an incident:

1. Check Redis/backend health and operation timeout counters.
2. Compare attempted, applied, mismatch and failed deltas over the same interval.
3. Verify binary payload comparison and TTL arguments at the caller boundary.
4. Confirm fallback CAS remains fail-closed while the shared primary is unavailable.
5. Do not replace CAS with read-then-write logic.

## Capacity and TTL tuning

For each active cache, use `host-cache-inventory.md` as the contract record.

Before changing a byte budget:

1. Sample p50, p95 and maximum encoded/estimated entry weight.
2. Measure active identity cardinality and eviction rate.
3. Reserve headroom for keys and per-entry metadata, not only payload bytes.
4. Set a rollback threshold for process RSS and eviction rate.

Before changing TTL:

1. Confirm the source-of-truth load cost and allowed stale bound.
2. Confirm invalidation is durable where stale data changes correctness.
3. Check whether the change creates synchronized expiry; retain deterministic jitter.
4. Keep negative TTL independent and shorter unless the negative result is explicitly stable.

Do not use TTL as a substitute for a missing durable invalidation path in correctness-sensitive multi-replica caches.

## Deployment verification

Run compiled and live Redis verification on the same revision before declaring the capability operationally verified. The canonical command list is in `implementation-plan.md`.

Minimum incident-drill evidence:

- Redis unavailable at startup;
- Redis disconnect and reconnect while serving bounded fallback reads;
- PubSub publication failure with durable generation reconciliation;
- listener lag/gap followed by full-clear recovery and delayed acknowledgement;
- CAS applied, mismatch, timeout and backend failure;
- synchronized stale refresh load with bounded saturation;
- worker termination reflected in readiness.

Record the revision, environment, Redis version, scenario result and recovery duration. Do not copy raw logs into the implementation plan.
