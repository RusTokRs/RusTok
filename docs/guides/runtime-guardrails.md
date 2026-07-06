---
id: doc://docs/guides/runtime-guardrails.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Runtime Guardrails

This document describes the operator-facing contract for runtime guardrails in `apps/server`.

## Why This Exists

Runtime guardrails aggregate live runtime signals into a single snapshot, so an operator can quickly see:

1. whether traffic can continue to be served;
2. which subsystem is currently degrading the runtime.

The current snapshot includes:

- rate-limit backend status and memory saturation;
- event transport fallback status;
- event bus backpressure status.
- `rustok.registry.remote_executor` status for the lease-based validation runner path.

## Endpoints

- `GET /health/runtime` — structured runtime guardrail snapshot;
- `GET /health/ready` — readiness with aggregated status;
- `GET /metrics` — Prometheus guardrail metrics.

## Snapshot Shape

`GET /health/runtime` returns:

- `status` — effective runtime status after rollout policy;
- `observed_status` — raw severity before softening in `observe` mode;
- `rollout` — `observe` or `enforce`;
- `reasons` — human-readable reasons for degradation;
- `rate_limits` — per-namespace limiter state (`api`, `auth`, `oauth`);
- `event_bus` — backpressure budget snapshot;
- `event_transport` — relay fallback state.
- `remote_executor` — internal validation runner contract state (`enabled`, token/config, active/expired claims, lease policy).

## How to Read the Snapshot

If `status != ok`, check fields in this order:

1. `reasons`
2. `rate_limits[*].healthy`
3. `rate_limits[*].state`
4. `rate_limits[*].policy`
5. `event_transport.relay_fallback_active`
6. `event_bus.state`
7. `remote_executor.state`

## Common Scenarios

Rate-limit backend unavailable:

- `rate_limits[*].healthy = false`;
- usually means Redis or another distributed backend is unavailable;
- `/health/ready` should contain a matching `runtime_guardrails` reason.

Memory limiter saturation:

- `rate_limits[*].distributed = false`;
- `total_entries` crossed warning/critical thresholds;
- typically resolved by reducing cardinality, shortening retention, or switching to a distributed backend.

Event relay fallback active:

- `event_transport.relay_fallback_active = true`;
- for production this is real degradation, not a harmless warning.

Event bus backpressure:

- `event_bus.state = degraded` or `critical`;
- `current_depth` is approaching `max_depth` or already hitting it;
- `events_rejected` indicates whether the runtime has started losing work.

Remote executor degradation:

- `remote_executor.enabled = true`, but `token_configured = false` — critical misconfiguration;
- `remote_executor.expired_claims > 0` — the reaper should already return stage to `queued`, but the operator still needs to look at runner availability and lease policy;
- `remote_executor.active_claims` helps distinguish an idle host from a host where thin runners are actually working.

## Metrics

Published via `/metrics`:

- `rustok_runtime_guardrail_rollout_mode`
- `rustok_runtime_guardrail_observed_status`
- `rustok_runtime_guardrail_status`
- `rustok_runtime_guardrail_rate_limit_backend_healthy`
- `rustok_runtime_guardrail_rate_limit_state`
- `rustok_runtime_guardrail_rate_limit_total_entries`
- `rustok_runtime_guardrail_rate_limit_active_clients`
- `rustok_runtime_guardrail_rate_limit_config`
- `rustok_runtime_guardrail_event_transport_fallback_active`
- `rustok_runtime_guardrail_event_backpressure_state`
- `rustok_runtime_guardrail_remote_executor_enabled`
- `rustok_runtime_guardrail_remote_executor_state`
- `rustok_runtime_guardrail_remote_executor_active_claims`
- `rustok_runtime_guardrail_remote_executor_expired_claims`
- `rustok_runtime_guardrail_remote_executor_config`


## Runtime Diagnostics Runbook

This section captures a short runbook for fast P0/P1 diagnostics
of runtime invariants. It is intended for situations where an operator or reviewer
needs to check the module graph, request context, locale cache, and migration safety without
a full workspace compilation.

### Module Graph Drift

Symptoms:

- `cargo xtask validate-manifest` fails on mismatch of `modules.toml`,
  generated runtime registry, or central registry evidence;
- `scripts/verify/verify-runtime-context-invariants.mjs` reports that
  `pages -> [content, page_builder]` is no longer confirmed by source-level
  evidence.

Quick diagnostics:

```bash
cargo xtask validate-manifest
node scripts/verify/verify-runtime-context-invariants.mjs
```

What to check:

1. `modules.toml` — canonical dependency graph.
2. `apps/server/src/modules/mod.rs` — runtime registry/test evidence.
3. `docs/modules/registry.md` — central documentation evidence.

A fix is considered correct only if the manifest, runtime registry, and docs
again describe the same graph; a manual special-case for one module is not
considered sufficient.

### Channel Resolution Without Locale or OAuth/Client Dimension

Symptoms:

- channel resolver receives an empty `RequestFacts.locale` despite having a resolved
  locale in request extensions;
- different OAuth/client contexts share a single channel cache key;
- a negative cache entry is reused for a different locale/client context.

Quick diagnostics:

```bash
node scripts/verify/verify-runtime-context-invariants.mjs
./scripts/verify/verify-all.sh runtime-context-invariants
```

What to check:

1. `apps/server/src/middleware/channel.rs` — `build_request_facts` reads
   `AuthContextExtension` and `ResolvedRequestLocale`.
2. `ChannelCacheKey` contains `oauth_app_id` and `locale`.
3. `apps/server/src/services/app_router.rs` preserves the actual execution
   order: locale -> auth_context -> channel.

### Locale DB Amplification

Symptoms:

- repeated tenant-bound requests consistently increase
  `rustok_tenant_locale_db_queries_total` without cache hits;
- `rustok_tenant_locale_cache_misses_total` grows on every request for the same
  tenant within TTL;
- `rustok_tenant_locale_cache_entries` does not reflect expected tenant snapshots.

Quick diagnostics:

```bash
node scripts/verify/verify-runtime-context-invariants.mjs
curl -s http://localhost:5150/metrics | rg 'rustok_tenant_locale_(cache|db)'
```

What to check:

1. `apps/server/src/middleware/locale.rs` — tenant locale policy cache is enabled
   before DB lookup.
2. `apps/server/src/controllers/metrics.rs` — cache hit/miss/db query/
   invalidation counters and entries gauge are exported.
3. If the policy was changed manually, check the invalidation path or wait for TTL
   snapshot refresh before comparing metrics.

### Migration Dependency Failure

Symptoms:

- `migration-smoke` fails on an empty PostgreSQL DB;
- a dependency descriptor references a missing migration;
- order/cycle validation fails after adding a module migration.

Quick diagnostics:

```bash
./scripts/verify/verify-migration-smoke.sh
RUSTOK_MIGRATION_SMOKE_INCREMENTAL=1 ./scripts/verify/verify-migration-smoke.sh
```

What to check:

1. A module crate with cross-module FK/order assumptions declares
   `migration_dependencies()` alongside `migrations()`.
2. The server migrator aggregates descriptors via module `MigrationSource`, not
   via a package-local allowlist for a single crate.
3. Descriptor names reference existing migrations, with no duplicate/cycle.
4. If the failure reproduces only in GitHub Actions, document exactly the
   environment-specific reason rather than disabling the smoke job.

### Inventory Admin Boundary Drift

Symptoms:

- the inventory admin write facade starts using a transitional GraphQL
  fallback;
- `set_variant_quantity` / `adjust_variant_quantity` again derive `inStock`
  only from numeric quantity;
- the transitional adapter contains inventory mutation markers.

Quick diagnostics:

```bash
node scripts/verify/verify-inventory-admin-boundary.mjs
./scripts/verify/verify-all.sh inventory-admin-boundary
```

What to check:

1. `crates/rustok-inventory/src/services/inventory.rs` — typed write result
   is built from committed quantity + inventory policy.
2. `crates/rustok-inventory/admin/src/api.rs` — write facades go through
   `crate::native::*` without `fallback_*`.
3. `crates/rustok-inventory/admin/src/transport.rs` — transitional GraphQL
   adapter remains read-only until the adapter is removed.

## Stop-the-Line Conditions

- any limiter backend becomes unhealthy;
- event relay fallback is activated;
- event bus reaches critical backpressure;
- readiness is degraded due to runtime guardrails, and the cause is not explained by the operator.

## Related Files

- [health.rs](../../apps/server/src/controllers/health.rs)
- [metrics.rs](../../apps/server/src/controllers/metrics.rs)
- [runtime_guardrails.rs](../../apps/server/src/services/runtime_guardrails.rs)
- [rate-limiting.md](./rate-limiting.md)
