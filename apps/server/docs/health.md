---
entities:
  - config://apps/server/config/production.redis.example.yaml#settings.rustok.readiness.outbox_max_pending_lag_seconds
  - config://apps/server/config/production.redis.example.yaml#settings.rustok.readiness.search_max_lag_seconds
---

# Health endpoints (`apps/server`)

This document describes the behavior of health endpoints in `apps/server/src/controllers/health.rs`.

## Endpoints

- `GET /health` — basic process status and application version.
- `GET /health/live` — liveness probe.
- `GET /health/ready` — readiness probe with aggregated status of dependencies and modules.
- `GET /health/runtime` — operator-facing snapshot runtime guardrails.
- `GET /health/modules` — health only for registered modules.

If `apps/server` is running in `settings.rustok.runtime.host_mode = "registry_only"`, the health/observability surface
works as a read-only catalog host, not as a full monolith.

Important: `host_mode` does not replace deployment profile. `DeploymentProfile` continues to describe build/deploy
surface (`monolith`, `server-with-admin`, `server-with-storefront`, `headless-api`), while
`settings.rustok.runtime.host_mode` describes only the runtime-exposed API surface (`full` or
`registry_only`).

A separate compile-time profile invariant: `embed-admin` and `embed-storefront` control not only routes,
but also the linkage of the corresponding UI hosts; similarly `mod-commerce`, `mod-blog`, `mod-forum`
and `mod-pages` control the inclusion of their REST/OpenAPI transport fragments, while the content-only maintenance
binary `migrate_legacy_richtext` requires `mod-content`. A reduced/headless server build is not obligated
to pull ecommerce or content surfaces it doesn't need.

## Readiness model

`/health/ready` returns:

- `status`: `ok | degraded | unhealthy`
- `checks`: infrastructure checks
- `modules`: module health from `ModuleRegistry`
- `degraded_reasons`: list of degradation causes

### Dependency checks

- `database` — critical check for DB availability;
- `database_schema` — critical check for mandatory runtime schema tables:
  `tenants`, `users`, `sys_events` when the active delivery profile is
  `outbox_local` or `outbox_iggy`, and
  `search_documents` when `rustok.features.search_indexing = true`;
- `cache_backend` — basic check of tenant cache path;
- `tenant_cache_invalidation` — non-critical check of the durable tenant-cache generation listener for cross-instance invalidation;
- `event_transport` — critical check of event transport initialization;
- `search_backend` — non-critical check of search connectivity;
- `email_backend` — non-critical configuration check of email transport: `smtp` must be enabled;
  `none` is explicitly reflected as degraded.
- `outbox_pending_lag` — non-critical check of the age of the oldest pending event, enabled for
  the active delivery profile uses the transactional outbox;
- `search_index_lag` — non-critical check of maximum lag between `search_documents.updated_at`
  and `search_documents.indexed_at`.

Lag thresholds are set in `settings.rustok.readiness`:

```yaml
readiness:
  outbox_max_pending_lag_seconds: 300
  search_max_lag_seconds: 300
```

Exceeding the threshold moves `/health/ready` to `degraded`, but not to `unhealthy`: lag requires operator action,
but by itself does not mean the process should be removed from service discovery as hard failure.

### Runtime worker checks

In full runtime `/health/ready` additionally verifies mandatory background workers with actual
handles in `AppContext.shared_store`.

Checks are published in `checks` with `kind = "worker"`:

- `worker:outbox_relay` — critical worker if the active delivery profile uses
  the transactional outbox and runtime
  built relay config;
- `worker:remote_executor_reaper` — critical worker if `rustok.registry.remote_executor.enabled = true`;
- `worker:seo_bulk` — critical worker if SEO bulk worker is enabled and build contains `mod-seo`.

If a worker is disabled by settings, check remains `ok` and `non_critical` with reason
`worker disabled by runtime settings`. If a mandatory worker is not registered in `shared_store`
or its task has already finished, check becomes `critical` + `unhealthy`. This prevents considering full
runtime ready until mandatory relay/worker lifecycle is started.

### Registry-only mode

In `settings.rustok.runtime.host_mode = "registry_only"` readiness aligns with actually started surface:

- only `database`, `cache_backend` and marker-check `host_mode` remain;
- `tenant_cache_invalidation`, `event_transport`, `search_backend`, rate-limit runtime and module runtime are not checked;
- `modules` in readiness are not used as hard gate and return operator marker instead of attempting to validate full module runtime.

### Module health and context-bound dependencies

`RusToKModule::health()` does not receive host-wide context, so the module cannot itself check host-owned runtime dependencies: DB schema, SMTP mailer, outbox relay worker, backlog/DLQ, search connector or indexing lag. For such modules, module-level health should not return unconditional `Healthy`.

Specific checks are performed in `/health/ready`:

- `email_backend` checks effective email transport;
- `event_transport`, `worker:outbox_relay` and `outbox_pending_lag` check outbox runtime;
- `search_backend` and `search_index_lag` check search runtime.

Therefore, context-bound modules like `rustok-email`, `rustok-outbox` and `rustok-search` return `Degraded` at module health level as operator marker, and the final readiness decision is made by readiness aggregation based on runtime checks.

## Aggregation

- if there is a `critical` check with status `unhealthy`, overall status is `unhealthy`;
- if there is no critical `unhealthy`, but there are non-`ok` checks, overall status is `degraded`;
- if all checks are `ok`, overall status is `ok`.

## Runtime guardrails

`/health/runtime` returns a rollout-aware snapshot for operators:

- `status` and `observed_status` for effective/raw severity;
- `rollout` (`observe|enforce`);
- `host_mode` (`full|registry_only`);
- `runtime_dependencies_enabled` — whether full runtime dependency layer is up;
- `reasons` with human-readable degradation causes;
- `rate_limits`, `event_bus`, `event_transport`, `remote_executor`.

Prometheus surface now also publishes:

- `rustok_runtime_guardrail_runtime_dependencies_enabled`
- `rustok_runtime_guardrail_host_mode{mode="full|registry_only"}`
- `rustok_runtime_guardrail_remote_executor_enabled`
- `rustok_runtime_guardrail_remote_executor_state`
- `rustok_runtime_guardrail_remote_executor_active_claims`
- `rustok_runtime_guardrail_remote_executor_expired_claims`
- `rustok_runtime_guardrail_remote_executor_config{setting="lease_ttl_ms|requeue_scan_interval_ms"}`

Worker/readiness metrics:

- `rustok_runtime_worker_state{worker="outbox_relay|remote_executor_reaper|seo_bulk"}`:
  `-1 = missing`, `0 = disabled`, `1 = running`, `2 = stopped`.
- `rustok_runtime_worker_lifecycle_state{worker,state}`:
  `starting = 1`, `ready = 2`, `degraded = 3`, `stopping = 4`, `failed = 5`.
- `rustok_runtime_worker_restarts_total{worker="outbox_relay"}` — number of restart cycles for relay supervisor
  after unexpected termination of internal worker task.

Worker lifecycle transitions are logged structurally through `worker` and `instance_id`: handle start,
relay loop start, shutdown signal, panic/restart and unexpected exit. Auth/email paths log only delivery status
and recipient/error; reset, verification, invite and refresh token values are not included in logs/metrics.

Email backend metrics:

- `rustok_email_backend_state{provider="none|smtp"}`:
  `0 = disabled`, `1 = enabled`, `2 = degraded/miswired`.
- `rustok_email_send_success_total`
- `rustok_email_send_failure_total`
- `rustok_email_send_skipped_total`

Outbox relay metrics:

- `rustok_outbox_backlog_size`
- `rustok_outbox_pending_lag_seconds`
- `rustok_outbox_retries_total`
- `rustok_outbox_dlq_total`
- `rustok_outbox_relay_processed_total`
- `rustok_outbox_relay_success_total`
- `rustok_outbox_relay_failure_total`
- `rustok_outbox_relay_retry_total`
- `rustok_outbox_relay_dlq_total`
- `rustok_outbox_relay_latency_ms_total`
- `rustok_outbox_relay_latency_samples`

Search metrics:

- `rustok_search_queries_total{surface,engine,status}` — search throughput and error rate by `status`;
- `rustok_search_query_duration_seconds{surface,engine}` — latency histogram for search query path;
- `rustok_search_slow_queries_total{surface,engine}`;
- `rustok_search_indexing_operations_total{operation,entity,status}`;
- `rustok_search_indexing_duration_seconds{operation,entity}`;
- `rustok_search_max_lag_seconds`;
- `rustok_search_lagging_tenants_total`.

The detailed snapshot contract and its Prometheus representation are described in [runtime-guardrails.md](../../docs/guides/runtime-guardrails.md).

## Local runbook for `registry_only`

If you need to locally run a read-only catalog host from the same `apps/server` binary, the canonical
minimum is currently:

```bash
RUSTOK_RUNTIME_HOST_MODE=registry_only cargo run -p rustok-server
```

```powershell
$env:RUSTOK_RUNTIME_HOST_MODE="registry_only"
cargo run -p rustok-server
```

Minimum smoke after start:

```bash
curl -i http://127.0.0.1:5150/health/ready
curl -i http://127.0.0.1:5150/health/runtime
curl -i http://127.0.0.1:5150/health/modules
curl -i http://127.0.0.1:5150/catalog?limit=1
curl -i http://127.0.0.1:5150/catalog/blog
curl -i http://127.0.0.1:5150/api/openapi.json
```

Expected behavior:

- `GET /health/ready` and `GET /health/modules` return `200`, despite reduced surface;
- `GET /health/runtime` explicitly returns `host_mode="registry_only"` and `runtime_dependencies_enabled=false`;
- `GET /catalog` returns the current read-only catalog contract with `ETag`, `Cache-Control` and `X-Total-Count`;
- `GET /catalog/{slug}` is the canonical detail contract for external discovery;
- `GET /api/openapi.json` advertises only registry/health/metrics/swagger surface;
- `POST /v2/catalog/publish`, `POST /v2/catalog/publish/{request_id}/validate`, `POST /v2/catalog/publish/{request_id}/stages`, `POST /v2/catalog/publish/{request_id}/request-changes`, `POST /v2/catalog/publish/{request_id}/hold`, `POST /v2/catalog/publish/{request_id}/resume`, `POST /v2/catalog/runner/claim`, `POST /v2/catalog/owner-transfer` and `POST /v2/catalog/yank` should not be available and normally give `404`;
- `GET /api/graphql`, `GET /api/auth/me`, `GET /admin` should not be available and normally give `404`.

For automated local checking, the same runtime contract is covered in
`scripts/verify/verify-deployment-profiles.sh` and `scripts/verify/verify-deployment-profiles.ps1`.
If you need to run the same smoke against an external dedicated host, these same scripts now
understand `RUSTOK_REGISTRY_BASE_URL`, optional `RUSTOK_REGISTRY_SMOKE_SLUG` and optional
`RUSTOK_REGISTRY_EVIDENCE_DIR`.

If checking the reduced build matrix specifically, it's useful to separately confirm the compile-time slice:

- `cargo check -p rustok-server --no-default-features` for the narrowest headless compile-time binary;
- `cargo check -p rustok-server --no-default-features --features redis-cache` for headless binary with Redis-backed runtime integrations;
- with server-side SEO/catalog/runtime changes, additionally one module-sliced profile like
  `cargo check -p rustok-server --no-default-features --features mod-commerce` or targeted
  no-commerce content host, if the specific deployment should not pull foreign transport surface.

## Production rollout for `modules.rustok.dev`

For the external dedicated catalog host, the canonical deployment contract is currently:

- build profile: `headless-api` (`--no-default-features`; add `redis-cache` only if deployment actually uses Redis-backed runtime integrations);
- runtime host mode: `RUSTOK_RUNTIME_HOST_MODE=registry_only`;
- process role: separate read-only host for V1 catalog, not a reduced monolith;
- V2 write-path is not routed to this host and should not be available after rollout.

For this dedicated host, `mod-commerce` is not a mandatory compile-time dependency if the catalog
does not publish ecommerce REST/OpenAPI surface.

Minimum production checklist before switching traffic:

1. Ensure deployment is built with the same `apps/server` binary, but without embedded admin/storefront surface.
2. Ensure runtime env explicitly sets `RUSTOK_RUNTIME_HOST_MODE=registry_only`.
3. Check `/health/ready` and `/health/runtime` on target instance.
4. Check `GET /catalog?limit=1` and `GET /catalog/{slug}` on target instance.
5. Check `ETag`, `Cache-Control` and `X-Total-Count` on `GET /catalog?limit=1`.
6. Check `GET /api/openapi.json` and ensure the spec has no `/v2/catalog/*`, `/api/graphql`, `/api/auth/*`.
7. Check negative smoke: `POST /v2/catalog/publish`, `POST /v2/catalog/publish/{request_id}/validate`, `POST /v2/catalog/publish/{request_id}/stages`, `POST /v2/catalog/publish/{request_id}/request-changes`, `POST /v2/catalog/publish/{request_id}/hold`, `POST /v2/catalog/publish/{request_id}/resume`, `POST /v2/catalog/runner/claim`, `POST /v2/catalog/owner-transfer`, `POST /v2/catalog/yank`, `GET /api/graphql`, `GET /admin` should give `404`.

Provider-agnostic edge/runtime invariants for this host:

- edge/CDN/reverse proxy must not rewrite path prefix and query string for `/catalog*`, `/health/*`, `/metrics`, `/api/openapi.*`;
- edge must not remove `ETag`, `Cache-Control`, `If-None-Match` and `X-Total-Count`, because this is part of the live catalog contract;
- edge must not replace API responses with its own HTML error pages for `404` on write/admin paths;
- `GET /catalog*` can be cached only with respect to origin headers; `/health/*` and `/api/openapi.*` should not become long-lived CDN cache;
- TLS termination/HSTS and redirect policy should be configured on edge, but without path rewrites and without downgrade to `http`;
- WAF/rate-limit layer must not inject auth headers and must not turn expected `404` on write-path into provider-specific `401/403`, otherwise external reduced-surface contract is lost.

Canonical automated smoke for already deployed host:

```bash
export RUSTOK_REGISTRY_BASE_URL="https://modules.rustok.dev"
export RUSTOK_REGISTRY_SMOKE_SLUG="blog"
export RUSTOK_REGISTRY_EVIDENCE_DIR="./tmp/modules-rustok-dev-smoke"
./scripts/verify/verify-deployment-profiles.sh
```

```powershell
$env:RUSTOK_REGISTRY_BASE_URL="https://modules.rustok.dev"
$env:RUSTOK_REGISTRY_SMOKE_SLUG="blog"
$env:RUSTOK_REGISTRY_EVIDENCE_DIR="C:\tmp\modules-rustok-dev-smoke"
./scripts/verify/verify-deployment-profiles.ps1
```

This external smoke does not replace the local build/profile matrix, but complements it:

- checks `/health/ready` and `/health/runtime` already on public host;
- checks `/health/modules` as live marker for registered `ModuleRegistry` even on reduced host;
- checks `GET /catalog?limit=1` and `GET /catalog/{slug}` on live instance;
- checks `ETag`, `Cache-Control` and `X-Total-Count`;
- checks reduced OpenAPI (`/api/openapi.json` and `/api/openapi.yaml`) for absence of write/API/UI surface;
- checks that `POST /v2/catalog/*`, `POST /v2/catalog/runner/claim`, `POST /v2/catalog/owner-transfer`, `POST /v2/catalog/yank` and `GET /admin` actually give `404`.

Minimum evidence package after rollout:

- save stdout/stderr external smoke from `scripts/verify/verify-deployment-profiles.sh` or `.ps1`;
- save `/health/runtime` response as rollout snapshot for this release;
- save snapshot `GET /api/openapi.json` as proof of reduced surface;
- record artifact identifier / build SHA / image tag and smoke check timestamp;
- if CDN/WAF is in front of host, separately note effective cache/TLS policy and absence of path rewrites for catalog endpoints.

If `RUSTOK_REGISTRY_EVIDENCE_DIR` is set, verify script automatically saves there at least:

- `runtime-headers.txt` and `runtime-body.json`;
- `catalog-headers.txt` and `catalog-body.json`;
- `openapi-headers.txt` and `openapi-body.json`;
- `openapi-yaml-headers.txt` and `openapi-yaml-body.yaml`;
- `registry-smoke-metadata.txt` with `base_url`, `smoke_slug` and UTC timestamp.

Minimum acceptance after rollout:

- `/health/ready` returns `200`;
- `/health/runtime` returns `host_mode="registry_only"` and `runtime_dependencies_enabled=false`;
- `GET /catalog` responds as the cache-friendly current contract;
- `GET /catalog/{slug}` responds as the canonical detail contract;
- reduced OpenAPI does not advertise write/API/UI surface;
- V2 write-path and monolith shell are actually unavailable from outside.

Rollback for this host remains normal rollback of deployment artifact or traffic switch to previous release. Important invariant: do not switch `modules.rustok.dev` to `full` runtime as temporary measure, because this breaks dedicated read-only catalog host contract.
Separately for rollback/incident path: if smoke fails specifically on reduced surface, first rollback deployment or traffic switch, not fix problem with temporary full-host routes enablement.

## Production rollback and incident ownership

For full runtime, rollback should not change event delivery, auth or search/index path semantics. Basic order:

1. Record failing artifact identifier, image tag/build SHA, configuration snapshot and rollback reason.
2. Switch traffic to previous verified release or rollback deployment artifact without changing runtime contracts.
3. Do not enable `registry_only` or `full` runtime as hidden workaround if it changes the public surface of current host.
4. Check `/health/ready`, `/health/runtime` and `/metrics` after switch.
5. Check outbox backlog/DLQ, auth login/token flows and search lag before re-enabling traffic.
6. Record post-rollback evidence: timestamp, artifact id, health snapshot, key metrics backlog/lag/error-rate and list of follow-up tasks.

Incident response ownership is established at team responsibility level, without binding to specific people:

| Area | Primary owner | Mandatory escalation path |
|---|---|---|
| Outbox/event delivery | Platform foundation on-call | `crates/rustok-outbox` owner + server runtime owner |
| Auth/JWT/RBAC | Platform security/auth on-call | `crates/rustok-auth` owner + server API owner |
| Search/index projection | Search module on-call | `crates/rustok-search` owner + platform database/runtime owner |

If incident affects multiple areas, Platform foundation on-call becomes coordinator, because it owns composition root and runtime readiness gates.

## Check reliability

For readiness checks, the following are used:

- timeout on check execution;
- in-process circuit breaker;
- fail-fast behavior on open circuit.

This prevents `/health/ready` from hanging on problematic dependency.
