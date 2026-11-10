//! Health check endpoints for K8s probes and module health aggregation

use crate::error::Result;
use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use chrono::Utc;
use futures_util::{FutureExt, StreamExt, future::BoxFuture, stream};
use once_cell::sync::Lazy;
use rustok_core::{HealthStatus, ModuleRegistry};
use rustok_outbox::entity::{Column as SysEventsColumn, Entity as SysEventsEntity, SysEventStatus};
use rustok_web::json_response;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, Statement,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use utoipa::ToSchema;

use crate::common::settings::{EmailProvider, RustokSettings};
use crate::middleware::rate_limit::{
    SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter,
};
use crate::middleware::tenant::{
    TenantInvalidationListenerStatus, tenant_invalidation_listener_snapshot,
};
use crate::services::app_lifecycle::{
    OutboxRelayWorkerHandle, OutboxRetentionWorkerHandle, RemoteExecutorReaperHandle,
    RuntimeWorkerLifecycleState, StopHandle, additional_runtime_worker_snapshots,
};
use crate::services::event_transport_factory;
use crate::services::runtime_guardrails::{
    RuntimeGuardrailSnapshot, RuntimeGuardrailStatus, collect_runtime_guardrail_snapshot,
};
use crate::services::server_runtime_context::ServerRuntimeContext;

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);
// A probe has one bounded budget even when several dependencies are slow.
const READINESS_TIMEOUT: Duration = Duration::from_secs(5);
const READINESS_CONCURRENCY: usize = 32;
const CIRCUIT_BREAKER_FAILURE_THRESHOLD: u32 = 3;
const CIRCUIT_BREAKER_COOLDOWN: Duration = Duration::from_secs(30);
const CRITICAL_MODULES: &[&str] = &["content", "commerce"];

static CIRCUITS: Lazy<Mutex<HashMap<String, CircuitState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ReadinessStatus {
    Ok,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DependencyCriticality {
    Critical,
    NonCritical,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    pub app: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct ReadinessCheck {
    name: String,
    kind: &'static str,
    criticality: DependencyCriticality,
    status: ReadinessStatus,
    latency_ms: u128,
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ReadinessResponse {
    status: ReadinessStatus,
    checks: Vec<ReadinessCheck>,
    modules: Vec<ReadinessCheck>,
    degraded_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReadinessProfile {
    registry_only: bool,
    runs_background_workers: bool,
}

impl ReadinessProfile {
    fn from_settings(settings: &RustokSettings) -> Self {
        Self {
            registry_only: settings.runtime.is_registry_only(),
            runs_background_workers: settings.runtime.runs_background_workers(),
        }
    }

    fn includes_runtime_dependencies(self) -> bool {
        !self.registry_only
    }

    fn includes_module_health(self) -> bool {
        !self.registry_only
    }

    fn includes_local_worker_checks(self) -> bool {
        self.runs_background_workers
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModuleHealth {
    pub slug: String,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ModulesHealthResponse {
    pub status: &'static str,
    pub modules: Vec<ModuleHealth>,
}

#[derive(Debug, Default, Clone)]
struct CircuitState {
    consecutive_failures: u32,
    open_until: Option<Instant>,
}

/// GET /health - Basic health check
#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    )
)]
pub async fn health() -> Result<Response> {
    tracing::debug!("health::health entry");
    Ok(json_response(HealthResponse {
        status: "ok",
        app: "rustok",
        version: env!("CARGO_PKG_VERSION"),
    }))
}

/// GET /health/live - K8s liveness probe
/// Always returns 200 if the process is running
#[utoipa::path(
    get,
    path = "/health/live",
    tag = "health",
    responses(
        (status = 200, description = "Process is alive")
    )
)]
pub async fn live() -> Result<Response> {
    tracing::debug!("health::live entry");
    Ok(json_response(serde_json::json!({ "status": "ok" })))
}

/// GET /health/ready - K8s readiness probe
/// Checks critical and non-critical infrastructure dependencies and module health.
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "Readiness status; unauthenticated callers receive only the aggregate status"),
        (status = 503, description = "Readiness is unhealthy; unauthenticated callers receive only the aggregate status")
    )
)]
pub async fn ready(
    State(ctx): State<ServerRuntimeContext>,
    Extension(registry): Extension<ModuleRegistry>,
) -> Result<Response> {
    match tokio::time::timeout(READINESS_TIMEOUT, collect_readiness(ctx, registry)).await {
        Ok(response) => {
            Ok((readiness_http_status(response.status), axum::Json(response)).into_response())
        }
        Err(_) => {
            tracing::warn!("readiness probe exceeded its deadline");
            let response = ReadinessResponse {
                status: ReadinessStatus::Unhealthy,
                checks: vec![ReadinessCheck {
                    name: "readiness_deadline".to_string(),
                    kind: "runtime",
                    criticality: DependencyCriticality::Critical,
                    status: ReadinessStatus::Unhealthy,
                    latency_ms: READINESS_TIMEOUT.as_millis(),
                    reason: Some("readiness probe timed out".to_string()),
                }],
                modules: Vec::new(),
                degraded_reasons: vec!["readiness probe timed out".to_string()],
            };
            Ok((StatusCode::SERVICE_UNAVAILABLE, axum::Json(response)).into_response())
        }
    }
}

async fn collect_readiness(
    ctx: ServerRuntimeContext,
    registry: ModuleRegistry,
) -> ReadinessResponse {
    let settings = ctx.settings();
    let profile = ReadinessProfile::from_settings(settings);
    // A bounded stream limits simultaneous work per probe; each task is dropped
    // when the overall deadline expires. The bool identifies module checks.
    let mut pending: Vec<BoxFuture<'static, (bool, ReadinessCheck)>> = Vec::new();
    {
        let db = ctx.db_clone();
        pending.push(
            async move {
                let check = run_guarded_check(
                    "database",
                    DependencyCriticality::Critical,
                    "dependency",
                    move || async move { check_database(&db).await },
                )
                .await;
                (false, check)
            }
            .boxed(),
        );
    }
    {
        let ctx = ctx.clone();
        pending.push(
            async move {
                let check =
                    run_guarded_check(
                        "database_schema",
                        DependencyCriticality::Critical,
                        "dependency",
                        move || async move {
                            check_required_database_schema(&ctx, ctx.settings()).await
                        },
                    )
                    .await;
                (false, check)
            }
            .boxed(),
        );
    }
    {
        let ctx = ctx.clone();
        pending.push(
            async move {
                let check = run_guarded_check(
                    "cache_backend",
                    DependencyCriticality::NonCritical,
                    "dependency",
                    move || async move { check_cache_backend(&ctx).await },
                )
                .await;
                (false, check)
            }
            .boxed(),
        );
    }
    let mut checks = vec![marketplace_provider_check(&ctx)];
    let mut module_checks = Vec::new();

    if profile.includes_runtime_dependencies() {
        {
            let ctx = ctx.clone();
            pending.push(
                async move {
                    let check = run_guarded_check(
                        "tenant_cache_invalidation",
                        DependencyCriticality::NonCritical,
                        "dependency",
                        move || async move { check_tenant_invalidation_listener(&ctx).await },
                    )
                    .await;
                    (false, check)
                }
                .boxed(),
            );
        }
        {
            let ctx = ctx.clone();
            pending.push(
                async move {
                    let check = run_guarded_check(
                        "event_transport",
                        DependencyCriticality::Critical,
                        "dependency",
                        move || async move { check_event_transport(&ctx).await },
                    )
                    .await;
                    (false, check)
                }
                .boxed(),
            );
        }
        if settings.rate_limit.enabled {
            for &namespace in &["api", "auth", "oauth"] {
                let ctx = ctx.clone();
                pending.push(
                    async move {
                        let name = format!("rate_limit:{namespace}");
                        let check = run_guarded_check(
                            &name,
                            DependencyCriticality::Critical,
                            "dependency",
                            move || async move { check_rate_limit_backend(&ctx, namespace).await },
                        )
                        .await;
                        (false, check)
                    }
                    .boxed(),
                );
            }
        }
        {
            let search = settings.search.clone();
            pending.push(
                async move {
                    let check = run_guarded_check(
                        "search_backend",
                        DependencyCriticality::NonCritical,
                        "dependency",
                        move || async move {
                            if !search.enabled
                                || search.driver.eq_ignore_ascii_case("postgres")
                                || search.driver.eq_ignore_ascii_case("memory")
                                || search.url.trim().is_empty()
                            {
                                return Ok(());
                            }
                            let (host, port) = parse_host_port(&search.url)?;
                            TcpStream::connect((host.as_str(), port))
                                .await
                                .map(|_| ())
                                .map_err(|_| "search backend connection failed".to_string())
                        },
                    )
                    .await;
                    (false, check)
                }
                .boxed(),
            );
        }
        {
            let ctx = ctx.clone();
            pending.push(
                async move {
                    let check = check_runtime_guardrails(&ctx).await;
                    (false, check)
                }
                .boxed(),
            );
        }
        {
            let ctx = ctx.clone();
            pending.push(
                async move {
                    let check = check_outbox_pending_lag(&ctx, ctx.settings()).await;
                    (false, check)
                }
                .boxed(),
            );
        }
        if settings.features.search_indexing {
            let ctx = ctx.clone();
            pending.push(
                async move {
                    let check = check_search_index_lag(&ctx, ctx.settings()).await;
                    (false, check)
                }
                .boxed(),
            );
        }
        checks.push(email_backend_check(settings));
        if profile.includes_local_worker_checks() {
            checks.extend(check_runtime_workers(&ctx, settings));
        } else {
            checks.push(worker_host_mode_check());
        }
        #[cfg(feature = "mod-media")]
        {
            let ctx = ctx.clone();
            pending.push(
                async move {
                    let check = run_guarded_check(
                        "storage",
                        DependencyCriticality::NonCritical,
                        "dependency",
                        move || async move { check_storage_backend(&ctx).await },
                    )
                    .await;
                    (false, check)
                }
                .boxed(),
            );
        }
    } else {
        checks.push(ReadinessCheck {
            name: "host_mode".to_string(),
            kind: "runtime",
            criticality: DependencyCriticality::NonCritical,
            status: ReadinessStatus::Ok,
            latency_ms: 0,
            reason: Some("registry_only host mode skips runtime-only readiness checks".to_string()),
        });
    }

    if profile.includes_module_health() {
        for module in registry.modules() {
            let module = module.clone();
            pending.push(
                async move {
                    let criticality = if CRITICAL_MODULES.contains(&module.slug()) {
                        DependencyCriticality::Critical
                    } else {
                        DependencyCriticality::NonCritical
                    };
                    let name = format!("module:{}", module.slug());
                    let check =
                        run_guarded_check(&name, criticality, "module", move || async move {
                            match module.health().await {
                                HealthStatus::Healthy => Ok(()),
                                HealthStatus::Degraded => {
                                    Err("module reported degraded".to_string())
                                }
                                HealthStatus::Unhealthy => {
                                    Err("module reported unhealthy".to_string())
                                }
                            }
                        })
                        .await;
                    (true, check)
                }
                .boxed(),
            );
        }
    } else {
        module_checks.push(ReadinessCheck {
            name: "module_runtime".to_string(),
            kind: "module",
            criticality: DependencyCriticality::NonCritical,
            status: ReadinessStatus::Ok,
            latency_ms: 0,
            reason: Some("registry_only host mode skips module health gating".to_string()),
        });
    }

    let results = run_readiness_checks(pending).await;
    for (is_module, check) in results {
        if is_module {
            module_checks.push(check);
        } else {
            checks.push(check);
        }
    }
    // Stable output is useful to operators and avoids completion-order churn.
    checks.sort_by(|left, right| left.name.cmp(&right.name));
    module_checks.sort_by(|left, right| left.name.cmp(&right.name));
    let status = aggregate_status(&checks, &module_checks);
    let degraded_reasons = collect_reasons(&checks, &module_checks);
    tracing::debug!(?status, "health::ready executed");
    ReadinessResponse {
        status,
        checks,
        modules: module_checks,
        degraded_reasons,
    }
}

async fn run_readiness_checks(
    pending: Vec<BoxFuture<'static, (bool, ReadinessCheck)>>,
) -> Vec<(bool, ReadinessCheck)> {
    stream::iter(pending)
        .buffer_unordered(READINESS_CONCURRENCY)
        .collect()
        .await
}

fn marketplace_provider_check(ctx: &ServerRuntimeContext) -> ReadinessCheck {
    use crate::services::marketplace_catalog::{
        MarketplaceProviderHealthStatus, SharedMarketplaceCatalogService,
    };

    let started_at = Instant::now();
    let Some(catalog) = ctx.shared_get::<SharedMarketplaceCatalogService>() else {
        return ReadinessCheck {
            name: "marketplace_providers".to_string(),
            kind: "dependency",
            criticality: DependencyCriticality::NonCritical,
            status: ReadinessStatus::Degraded,
            latency_ms: started_at.elapsed().as_millis(),
            reason: Some("marketplace provider composition is unavailable".to_string()),
        };
    };
    let snapshots = catalog.0.provider_health();
    let degraded = snapshots
        .iter()
        .filter(|snapshot| {
            matches!(
                snapshot.status,
                MarketplaceProviderHealthStatus::Unknown
                    | MarketplaceProviderHealthStatus::Degraded
            )
        })
        .map(|snapshot| {
            format!(
                "{}={:?}(failures={})",
                snapshot.provider, snapshot.status, snapshot.consecutive_failures
            )
            .to_lowercase()
        })
        .collect::<Vec<_>>();

    ReadinessCheck {
        name: "marketplace_providers".to_string(),
        kind: "dependency",
        criticality: DependencyCriticality::NonCritical,
        status: if degraded.is_empty() {
            ReadinessStatus::Ok
        } else {
            ReadinessStatus::Degraded
        },
        latency_ms: started_at.elapsed().as_millis(),
        reason: (!degraded.is_empty()).then(|| degraded.join(", ")),
    }
}

/// GET /health/runtime - Runtime guardrail snapshot for operators
/// Returns the current rollout-aware guardrail state plus component-level details.
#[utoipa::path(
    get,
    path = "/health/runtime",
    tag = "health",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Runtime guardrail snapshot", body = RuntimeGuardrailSnapshot)
    )
)]
pub async fn runtime(State(ctx): State<ServerRuntimeContext>) -> Result<Response> {
    tracing::debug!("health::runtime entry");
    let snapshot = collect_runtime_guardrail_snapshot(&ctx).await;
    Ok(json_response(snapshot))
}

/// GET /health/modules - Module health aggregation
/// Reports health status of all registered modules
#[utoipa::path(
    get,
    path = "/health/modules",
    tag = "health",
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Module health statuses", body = ModulesHealthResponse)
    )
)]
pub async fn modules(Extension(registry): Extension<ModuleRegistry>) -> Result<Response> {
    tracing::debug!("health::modules entry");
    let mut modules_health = Vec::new();
    let mut any_unhealthy = false;
    let mut any_degraded = false;

    for module in registry.modules() {
        let health = module.health().await;
        let status_str = match health {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => {
                any_degraded = true;
                "degraded"
            }
            HealthStatus::Unhealthy => {
                any_unhealthy = true;
                "unhealthy"
            }
        };

        modules_health.push(ModuleHealth {
            slug: module.slug().to_string(),
            name: module.name().to_string(),
            status: status_str.to_string(),
        });
    }

    let status = resolve_modules_health_status(any_unhealthy, any_degraded);

    Ok(json_response(ModulesHealthResponse {
        status,
        modules: modules_health,
    }))
}

fn resolve_modules_health_status(any_unhealthy: bool, any_degraded: bool) -> &'static str {
    if any_unhealthy {
        "unhealthy"
    } else if any_degraded {
        "degraded"
    } else {
        "ok"
    }
}

#[cfg(feature = "mod-media")]
async fn check_storage_backend(ctx: &ServerRuntimeContext) -> std::result::Result<(), String> {
    use object_store::ObjectStoreExt;
    use rustok_storage::StorageRuntime;

    let Some(storage) = ctx.shared_get::<StorageRuntime>() else {
        return Err("storage runtime is not configured".to_string());
    };

    let probe = rustok_storage::ObjectKey::chronological(
        "platform-health",
        rustok_storage::ObjectZone::Staging,
        rustok_storage::ObjectScope::Platform,
        chrono::Utc::now(),
        uuid::Uuid::nil(),
        "probe",
    )
    .map_err(|_| "storage health probe could not be constructed".to_string())?
    .into_path();
    let data = bytes::Bytes::from_static(b"ok");
    let result = async {
        storage
            .objects
            .put_opts(&probe, data.into(), storage.put_options("text/plain"))
            .await
            .map_err(|_| "storage health probe write failed".to_string())?;
        storage
            .objects
            .delete(&probe)
            .await
            .map_err(|_| "storage health probe cleanup failed".to_string())?;
        Ok(())
    }
    .await;
    rustok_telemetry::metrics::update_storage_health(storage.kind.as_str(), result.is_ok());
    result
}

async fn check_database(db: &DatabaseConnection) -> std::result::Result<(), String> {
    db.execute_unprepared("SELECT 1")
        .await
        .map(|_| ())
        .map_err(|_| "database connectivity check failed".to_string())
}

async fn check_required_database_schema(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
) -> std::result::Result<(), String> {
    let backend = ctx.db().get_database_backend();
    for table in required_database_schema_tables(settings) {
        let stmt = Statement::from_string(backend, format!("SELECT 1 FROM {table} LIMIT 1"));
        if ctx.db().execute_raw(stmt).await.is_err() {
            return Err(format!("required table `{table}` is unavailable"));
        }
    }

    Ok(())
}

fn required_database_schema_tables(settings: &RustokSettings) -> Vec<&'static str> {
    let mut required_tables = vec!["tenants", "users"];

    required_tables.push("sys_events");

    if settings.features.search_indexing {
        required_tables.push("search_documents");
    }

    required_tables
}

async fn check_cache_backend(ctx: &ServerRuntimeContext) -> std::result::Result<(), String> {
    use rustok_cache::CacheService;

    let Some(cache) = ctx.shared_get::<CacheService>() else {
        return Ok(()); // not configured — skip
    };
    let report = cache.health().await;
    if report.is_healthy() {
        Ok(())
    } else if report.is_connected() {
        // Redis answered but recovery is incomplete. The two causes lead to different operator
        // actions, so the readiness payload must not report a transport outage.
        Err(format!(
            "cache recovery is incomplete ({} generations pending{})",
            report.redis_recovery_pending,
            report
                .redis_recovery_error
                .as_deref()
                .map(|error| format!(": {error}"))
                .unwrap_or_default()
        ))
    } else {
        Err("cache backend is unhealthy".to_string())
    }
}

async fn check_tenant_invalidation_listener(
    ctx: &ServerRuntimeContext,
) -> std::result::Result<(), String> {
    let snapshot = tenant_invalidation_listener_snapshot(ctx).await;

    match snapshot.status {
        TenantInvalidationListenerStatus::Disabled | TenantInvalidationListenerStatus::Healthy => {
            Ok(())
        }
        TenantInvalidationListenerStatus::Starting => {
            Err("tenant invalidation listener is starting".to_string())
        }
        TenantInvalidationListenerStatus::Degraded => {
            Err("tenant invalidation listener is degraded".to_string())
        }
    }
}

async fn check_event_transport(ctx: &ServerRuntimeContext) -> std::result::Result<(), String> {
    use rustok_core::events::EventTransport;
    use std::sync::Arc;

    if let Some(runtime) =
        ctx.shared_get::<Arc<crate::services::event_transport_factory::EventRuntime>>()
        && runtime.relay_fallback_active
    {
        return Err(
            "event relay target is degraded: fallback-to-memory mode is active".to_string(),
        );
    }

    ctx.shared_get::<Arc<dyn EventTransport>>()
        .map(|_| ())
        .ok_or_else(|| "event transport not initialized in shared_store".to_string())
}

// Lag is derived from signed database/time values, whereas configured
// thresholds are u64. Never wrap a large valid threshold into a negative i64.
fn lag_exceeds_threshold(lag_seconds: i64, threshold_seconds: u64) -> bool {
    lag_seconds.max(0) as u64 > threshold_seconds
}

async fn check_outbox_pending_lag(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
) -> ReadinessCheck {
    let started_at = Instant::now();
    let threshold = settings.readiness.outbox_max_pending_lag_seconds;
    let result = SysEventsEntity::find()
        .filter(SysEventsColumn::Status.eq(SysEventStatus::Pending))
        .order_by_asc(SysEventsColumn::CreatedAt)
        .one(ctx.db())
        .await;

    let (status, reason) = match result {
        Ok(Some(event)) => {
            let lag_seconds = (Utc::now() - event.created_at).num_seconds().max(0);
            if lag_exceeds_threshold(lag_seconds, threshold) {
                (
                    ReadinessStatus::Degraded,
                    Some(format!(
                        "oldest pending outbox event lag {lag_seconds}s exceeds threshold {threshold}s"
                    )),
                )
            } else {
                (ReadinessStatus::Ok, None)
            }
        }
        Ok(None) => (ReadinessStatus::Ok, None),
        Err(_) => (
            ReadinessStatus::Degraded,
            Some("outbox lag check failed".to_string()),
        ),
    };

    ReadinessCheck {
        name: "outbox_pending_lag".to_string(),
        kind: "lag",
        criticality: DependencyCriticality::NonCritical,
        status,
        latency_ms: started_at.elapsed().as_millis(),
        reason,
    }
}

async fn check_search_index_lag(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
) -> ReadinessCheck {
    let started_at = Instant::now();
    let threshold = settings.readiness.search_max_lag_seconds;
    let backend = ctx.db().get_database_backend();
    let stmt = Statement::from_string(backend, search_index_lag_query(backend).to_string());

    let (status, reason) = match ctx.db().query_one_raw(stmt).await {
        Ok(Some(row)) => {
            let lag_seconds = match row.try_get::<i64>("", "max_lag_seconds") {
                Ok(value) => value.max(0),
                Err(_) => {
                    return ReadinessCheck {
                        name: "search_index_lag".to_string(),
                        kind: "lag",
                        criticality: DependencyCriticality::NonCritical,
                        status: ReadinessStatus::Degraded,
                        latency_ms: started_at.elapsed().as_millis(),
                        reason: Some("search lag check failed".to_string()),
                    };
                }
            };
            if lag_exceeds_threshold(lag_seconds, threshold) {
                (
                    ReadinessStatus::Degraded,
                    Some(format!(
                        "search indexing lag {lag_seconds}s exceeds threshold {threshold}s"
                    )),
                )
            } else {
                (ReadinessStatus::Ok, None)
            }
        }
        Ok(None) => (ReadinessStatus::Ok, None),
        Err(error) if is_missing_search_relation_error(&error) => (
            ReadinessStatus::Degraded,
            Some("search_documents relation is not available for lag check".to_string()),
        ),
        Err(_) => (
            ReadinessStatus::Degraded,
            Some("search lag check failed".to_string()),
        ),
    };

    ReadinessCheck {
        name: "search_index_lag".to_string(),
        kind: "lag",
        criticality: DependencyCriticality::NonCritical,
        status,
        latency_ms: started_at.elapsed().as_millis(),
        reason,
    }
}

fn search_index_lag_query(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Sqlite => {
            // SQLite parses timestamps at millisecond precision. Integer
            // milliseconds avoid julianday floating-point errors at half seconds.
            r#"
            SELECT
                CAST(
                    COALESCE(
                        MAX(
                            CASE
                                WHEN julianday(updated_at) > julianday(indexed_at) THEN (
                                    (CAST(strftime('%s', updated_at) AS INTEGER) - CAST(strftime('%s', indexed_at) AS INTEGER)) * 1000
                                    + CAST(substr(strftime('%f', updated_at), 4, 3) AS INTEGER)
                                    - CAST(substr(strftime('%f', indexed_at), 4, 3) AS INTEGER)
                                    + 500
                                ) / 1000
                                ELSE 0
                            END
                        ),
                        0
                    ) AS INTEGER
                ) AS max_lag_seconds
            FROM search_documents
            "#
        }
        _ => {
            r#"
            SELECT COALESCE(MAX(GREATEST(EXTRACT(EPOCH FROM (updated_at - indexed_at)), 0)), 0)::bigint AS max_lag_seconds
            FROM search_documents
            "#
        }
    }
}

fn is_missing_search_relation_error(error: &sea_orm::DbErr) -> bool {
    let message = error.to_string().to_ascii_lowercase();
    message.contains("no such table")
        || message.contains("undefinedtable")
        || message.contains("relation") && message.contains("does not exist")
}

fn worker_host_mode_check() -> ReadinessCheck {
    ReadinessCheck {
        name: "worker_host_mode".to_string(),
        kind: "runtime",
        criticality: DependencyCriticality::NonCritical,
        status: ReadinessStatus::Ok,
        latency_ms: 0,
        reason: Some(
            "surface host mode does not run local background workers; worker readiness is owned by the worker host"
                .to_string(),
        ),
    }
}

fn check_runtime_workers(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
) -> Vec<ReadinessCheck> {
    let event_runtime = ctx.shared_get::<std::sync::Arc<event_transport_factory::EventRuntime>>();
    let relay_required = event_runtime
        .as_ref()
        .and_then(|runtime| runtime.relay_config.clone())
        .is_some();
    let retention_required = event_runtime
        .as_ref()
        .and_then(|runtime| runtime.outbox_retention.clone())
        .is_some();
    let stop_requested = ctx
        .shared_map::<StopHandle, _>(StopHandle::is_stopping)
        .unwrap_or(false);

    let mut checks = vec![runtime_worker_check(
        "worker:outbox_relay",
        relay_required,
        ctx.shared_map::<OutboxRelayWorkerHandle, _>(OutboxRelayWorkerHandle::is_finished),
        stop_requested,
    )];

    checks.push(runtime_worker_check(
        "worker:outbox_retention",
        retention_required,
        ctx.shared_map::<OutboxRetentionWorkerHandle, _>(OutboxRetentionWorkerHandle::is_finished),
        stop_requested,
    ));

    checks.push(runtime_worker_check(
        "worker:remote_executor_reaper",
        settings.registry.remote_executor.enabled,
        ctx.shared_map::<RemoteExecutorReaperHandle, _>(RemoteExecutorReaperHandle::is_finished),
        stop_requested,
    ));

    for worker in additional_runtime_worker_snapshots(ctx) {
        checks.push(runtime_worker_check(
            &format!("worker:{}", worker.name),
            worker.required,
            worker.finished,
            stop_requested,
        ));
    }

    #[cfg(feature = "mod-seo")]
    {
        let seo_worker_finished = ctx
            .shared_map::<crate::services::app_lifecycle::SeoBulkWorkerHandle, _>(
                crate::services::app_lifecycle::SeoBulkWorkerHandle::is_finished,
            );
        checks.push(runtime_worker_check(
            "worker:seo_bulk",
            settings.runtime.background_workers.seo_bulk_enabled,
            seo_worker_finished,
            stop_requested,
        ));
        checks.push(runtime_worker_check(
            "worker:seo_sitemap",
            settings.runtime.background_workers.seo_sitemap_enabled,
            seo_worker_finished,
            stop_requested,
        ));
        checks.push(runtime_worker_check(
            "worker:seo_index_repair",
            settings.runtime.background_workers.seo_index_repair_enabled,
            seo_worker_finished,
            stop_requested,
        ));
    }

    checks
}

fn runtime_worker_check(
    name: &str,
    required: bool,
    handle_finished: Option<bool>,
    stop_requested: bool,
) -> ReadinessCheck {
    let lifecycle_state = RuntimeWorkerLifecycleState::from_worker_snapshot(
        required,
        handle_finished,
        stop_requested,
    );
    let criticality = if required {
        DependencyCriticality::Critical
    } else {
        DependencyCriticality::NonCritical
    };
    let status = match (criticality, lifecycle_state) {
        (_, RuntimeWorkerLifecycleState::Ready) => ReadinessStatus::Ok,
        (DependencyCriticality::Critical, RuntimeWorkerLifecycleState::Starting)
        | (DependencyCriticality::Critical, RuntimeWorkerLifecycleState::Stopping)
        | (DependencyCriticality::Critical, RuntimeWorkerLifecycleState::Failed) => {
            ReadinessStatus::Unhealthy
        }
        _ => ReadinessStatus::Degraded,
    };
    let reason = match (required, lifecycle_state) {
        (false, RuntimeWorkerLifecycleState::Ready) => {
            Some("worker disabled by runtime settings".to_string())
        }
        (_, RuntimeWorkerLifecycleState::Starting) => {
            Some("required worker lifecycle state is starting; handle is missing".to_string())
        }
        (_, RuntimeWorkerLifecycleState::Stopping) => {
            Some("worker lifecycle state is stopping".to_string())
        }
        (_, RuntimeWorkerLifecycleState::Failed) => {
            Some("required worker lifecycle state is failed; task has stopped".to_string())
        }
        (_, RuntimeWorkerLifecycleState::Degraded) => {
            Some("worker lifecycle state is degraded".to_string())
        }
        (_, RuntimeWorkerLifecycleState::Ready) => None,
    };

    ReadinessCheck {
        name: name.to_string(),
        kind: "worker",
        criticality,
        status,
        latency_ms: 0,
        reason,
    }
}

fn email_backend_check(settings: &RustokSettings) -> ReadinessCheck {
    let (status, reason) = match settings.email.provider {
        EmailProvider::None => (
            ReadinessStatus::Degraded,
            Some("email provider disabled by configuration".to_string()),
        ),
        EmailProvider::Smtp if !settings.email.enabled => (
            ReadinessStatus::Degraded,
            Some("smtp email provider disabled by configuration".to_string()),
        ),
        EmailProvider::Smtp => (ReadinessStatus::Ok, None),
    };

    ReadinessCheck {
        name: "email_backend".to_string(),
        kind: "dependency",
        criticality: DependencyCriticality::NonCritical,
        status,
        latency_ms: 0,
        reason,
    }
}

async fn check_rate_limit_backend(
    ctx: &ServerRuntimeContext,
    namespace: &'static str,
) -> std::result::Result<(), String> {
    match namespace {
        "api" => ctx
            .shared_get::<SharedApiRateLimiter>()
            .ok_or_else(|| "API rate limiter not initialized in shared_store".to_string())?
            .0
            .check_backend_health()
            .await
            .map_err(|_| "api rate-limit backend check failed".to_string()),
        "auth" => ctx
            .shared_get::<SharedAuthRateLimiter>()
            .ok_or_else(|| "auth rate limiter not initialized in shared_store".to_string())?
            .0
            .check_backend_health()
            .await
            .map_err(|_| "auth rate-limit backend check failed".to_string()),
        "oauth" => ctx
            .shared_get::<SharedOAuthRateLimiter>()
            .ok_or_else(|| "oauth rate limiter not initialized in shared_store".to_string())?
            .0
            .check_backend_health()
            .await
            .map_err(|_| "oauth rate-limit backend check failed".to_string()),
        _ => Err(format!("unknown rate-limit namespace: {namespace}")),
    }
}

async fn check_runtime_guardrails(ctx: &ServerRuntimeContext) -> ReadinessCheck {
    let started_at = Instant::now();
    let snapshot = collect_runtime_guardrail_snapshot(ctx).await;
    let (criticality, status) = match snapshot.status {
        RuntimeGuardrailStatus::Ok => (DependencyCriticality::NonCritical, ReadinessStatus::Ok),
        RuntimeGuardrailStatus::Degraded => (
            DependencyCriticality::NonCritical,
            ReadinessStatus::Degraded,
        ),
        RuntimeGuardrailStatus::Critical => {
            (DependencyCriticality::Critical, ReadinessStatus::Unhealthy)
        }
    };

    ReadinessCheck {
        name: "runtime_guardrails".to_string(),
        kind: "guardrail",
        criticality,
        status,
        latency_ms: started_at.elapsed().as_millis(),
        reason: (!snapshot.reasons.is_empty()).then(|| {
            format!(
                "rollout={:?}; observed={:?}; {}",
                snapshot.rollout,
                snapshot.observed_status,
                snapshot.reasons.join("; ")
            )
        }),
    }
}

async fn run_guarded_check<F, Fut>(
    name: &str,
    criticality: DependencyCriticality,
    kind: &'static str,
    check_fn: F,
) -> ReadinessCheck
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<(), String>>,
{
    let started_at = Instant::now();
    if let Some(open_for_ms) = circuit_open_for(name) {
        return ReadinessCheck {
            name: name.to_string(),
            kind,
            criticality,
            status: status_for_failure(criticality),
            latency_ms: started_at.elapsed().as_millis(),
            reason: Some(format!("circuit open for {open_for_ms}ms")),
        };
    }

    let result = tokio::time::timeout(HEALTH_CHECK_TIMEOUT, check_fn()).await;

    match result {
        Ok(Ok(())) => {
            on_check_success(name);
            ReadinessCheck {
                name: name.to_string(),
                kind,
                criticality,
                status: ReadinessStatus::Ok,
                latency_ms: started_at.elapsed().as_millis(),
                reason: None,
            }
        }
        Ok(Err(reason)) => {
            on_check_failure(name);
            ReadinessCheck {
                name: name.to_string(),
                kind,
                criticality,
                status: status_for_failure(criticality),
                latency_ms: started_at.elapsed().as_millis(),
                reason: Some(reason),
            }
        }
        Err(_) => {
            on_check_failure(name);
            ReadinessCheck {
                name: name.to_string(),
                kind,
                criticality,
                status: status_for_failure(criticality),
                latency_ms: started_at.elapsed().as_millis(),
                reason: Some("health check timed out".to_string()),
            }
        }
    }
}

fn status_for_failure(criticality: DependencyCriticality) -> ReadinessStatus {
    match criticality {
        DependencyCriticality::Critical => ReadinessStatus::Unhealthy,
        DependencyCriticality::NonCritical => ReadinessStatus::Degraded,
    }
}

fn circuit_open_for(name: &str) -> Option<u128> {
    let mut state = CIRCUITS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = Instant::now();

    if let Some(circuit) = state.get_mut(name)
        && let Some(open_until) = circuit.open_until
    {
        if open_until > now {
            return Some(open_until.duration_since(now).as_millis());
        }

        circuit.open_until = None;
        circuit.consecutive_failures = 0;
    }

    None
}

fn on_check_success(name: &str) {
    let mut state = CIRCUITS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.remove(name);
}

fn on_check_failure(name: &str) {
    let mut state = CIRCUITS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let circuit = state.entry(name.to_string()).or_default();
    circuit.consecutive_failures += 1;

    if circuit.consecutive_failures >= CIRCUIT_BREAKER_FAILURE_THRESHOLD {
        circuit.open_until = Some(Instant::now() + CIRCUIT_BREAKER_COOLDOWN);
        tracing::warn!(
            name,
            threshold = CIRCUIT_BREAKER_FAILURE_THRESHOLD,
            "Health check circuit breaker tripped open"
        );
    }
}

fn readiness_http_status(status: ReadinessStatus) -> StatusCode {
    match status {
        ReadinessStatus::Ok | ReadinessStatus::Degraded => StatusCode::OK,
        ReadinessStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    }
}

fn aggregate_status(checks: &[ReadinessCheck], modules: &[ReadinessCheck]) -> ReadinessStatus {
    let all = checks.iter().chain(modules.iter());

    if all.clone().any(|check| {
        check.criticality == DependencyCriticality::Critical
            && check.status == ReadinessStatus::Unhealthy
    }) {
        return ReadinessStatus::Unhealthy;
    }

    if all.clone().any(|check| check.status != ReadinessStatus::Ok) {
        return ReadinessStatus::Degraded;
    }

    ReadinessStatus::Ok
}

fn collect_reasons(checks: &[ReadinessCheck], modules: &[ReadinessCheck]) -> Vec<String> {
    checks
        .iter()
        .chain(modules.iter())
        .filter_map(|check| {
            check
                .reason
                .as_ref()
                .map(|reason| format!("{} ({:?}): {}", check.name, check.criticality, reason))
        })
        .collect()
}

fn parse_host_port(url: &str) -> std::result::Result<(String, u16), String> {
    let (scheme, authority_source) = match url.split_once("://") {
        Some((scheme, rest)) => (Some(scheme.trim().to_ascii_lowercase()), rest),
        None => (None, url),
    };
    let default_port = match scheme.as_deref() {
        Some("https") => 443,
        _ => 80,
    };

    let authority_trimmed = authority_source
        .split('/')
        .next()
        .unwrap_or_default()
        .trim();
    let authority = match authority_trimmed.rsplit_once('@') {
        Some((_, host)) => host,
        None => authority_trimmed,
    };

    if authority.is_empty() {
        return Err("search backend URL is empty or invalid".to_string());
    }

    if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| "search backend URL contains an invalid host".to_string())?;
        let host = &authority[1..end];
        if host.is_empty() {
            return Err("search backend URL contains an invalid host".to_string());
        }
        let remainder = &authority[end + 1..];
        let port = if remainder.is_empty() {
            default_port
        } else if let Some(port) = remainder.strip_prefix(':') {
            port.parse::<u16>()
                .map_err(|_| "search backend URL contains an invalid port".to_string())?
        } else {
            return Err("search backend URL contains an invalid host".to_string());
        };
        return Ok((host.to_string(), port));
    }

    if authority.matches(':').count() > 1 {
        return Ok((authority.to_string(), default_port));
    }

    if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port
            .parse::<u16>()
            .map_err(|_| "search backend URL contains an invalid port".to_string())?;
        if host.is_empty() {
            return Err("search backend URL contains an invalid host".to_string());
        }
        return Ok((host.to_string(), port));
    }

    Ok((authority.to_string(), default_port))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/health", get(health))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/health/runtime", get(runtime))
        .route("/health/modules", get(modules))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::tenant::TenantInvalidationListenerSnapshot;

    #[test]
    fn readiness_lag_comparison_handles_full_u64_threshold_range() {
        assert!(lag_exceeds_threshold(10, 9));
        assert!(!lag_exceeds_threshold(10, 10));
        assert!(!lag_exceeds_threshold(-1, 0));
        assert!(!lag_exceeds_threshold(i64::MAX, i64::MAX as u64 + 1));
        assert!(!lag_exceeds_threshold(i64::MAX, u64::MAX));
    }

    #[tokio::test]
    async fn sqlite_outbox_lag_accepts_large_unsigned_thresholds() {
        use sea_orm::{ActiveModelTrait, ConnectionTrait, Schema, Set};

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let backend = db.get_database_backend();
        db.execute_raw(
            backend.build(
                &Schema::new(backend).create_table_from_entity(rustok_outbox::entity::Entity),
            ),
        )
        .await
        .unwrap();
        rustok_outbox::entity::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            event_type: Set("test.event".to_string()),
            schema_version: Set(1),
            payload: Set(serde_json::json!({})),
            status: Set(SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(Utc::now() - chrono::Duration::minutes(5)),
            dispatched_at: Set(None),
        }
        .insert(&db)
        .await
        .unwrap();
        let mut settings = RustokSettings::default();
        settings.readiness.outbox_max_pending_lag_seconds = 1;
        let ctx = ServerRuntimeContext::new(db, settings.clone());
        assert_eq!(
            check_outbox_pending_lag(&ctx, &settings).await.status,
            ReadinessStatus::Degraded
        );
        settings.readiness.outbox_max_pending_lag_seconds = u64::MAX;
        assert_eq!(
            check_outbox_pending_lag(&ctx, &settings).await.status,
            ReadinessStatus::Ok
        );
    }

    #[tokio::test]
    async fn sqlite_search_lag_uses_rounded_seconds_for_readiness() {
        use sea_orm::ConnectionTrait;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE search_documents (indexed_at TEXT, updated_at TEXT)")
            .await
            .unwrap();
        db.execute_unprepared(
            "INSERT INTO search_documents VALUES ('2024-01-01 00:00:00', '2024-01-01 00:00:10')",
        )
        .await
        .unwrap();
        let mut settings = RustokSettings::default();
        settings.readiness.search_max_lag_seconds = 9;
        let ctx = ServerRuntimeContext::new(db, settings.clone());
        let lagging = check_search_index_lag(&ctx, &settings).await;
        assert_eq!(lagging.status, ReadinessStatus::Degraded);
        assert!(lagging.reason.as_deref().unwrap().contains("lag 10s"));
        settings.readiness.search_max_lag_seconds = 10;
        let within_threshold = check_search_index_lag(&ctx, &settings).await;
        assert_eq!(within_threshold.status, ReadinessStatus::Ok);
        settings.readiness.search_max_lag_seconds = u64::MAX;
        let large_threshold = check_search_index_lag(&ctx, &settings).await;
        assert_eq!(large_threshold.status, ReadinessStatus::Ok);
    }

    #[tokio::test]
    async fn sqlite_search_lag_rounds_half_second_boundaries_for_readiness() {
        use sea_orm::ConnectionTrait;

        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE search_documents (indexed_at TEXT, updated_at TEXT)")
            .await
            .unwrap();
        let mut settings = RustokSettings::default();
        settings.readiness.search_max_lag_seconds = 0;
        let ctx = ServerRuntimeContext::new(db, settings.clone());
        for (indexed, updated, expected) in [
            ("2024-01-01 00:00:04.000", "2024-01-01 00:00:04.499", 0),
            ("2024-01-01 00:00:04.000", "2024-01-01 00:00:04.500", 1),
            ("2024-01-01 00:00:04.000", "2024-01-01 00:00:05.499", 1),
            ("2024-01-01 00:00:04.000", "2024-01-01 00:00:05.500", 2),
            ("2024-01-01 00:00:59.900", "2024-01-01 00:01:00.400", 1),
            ("1969-12-31 23:59:59.750", "1970-01-01 00:00:00.250", 1),
            (
                "2024-01-01 01:00:00.000+01:00",
                "2024-01-01 00:00:01.000+00:00",
                1,
            ),
            (
                "2024-01-01 00:00:01.000+00:00",
                "2024-01-01 01:00:00.000+01:00",
                0,
            ),
        ] {
            ctx.db()
                .execute_unprepared("DELETE FROM search_documents")
                .await
                .unwrap();
            ctx.db()
                .execute_unprepared(&format!(
                    "INSERT INTO search_documents VALUES ('{indexed}', '{updated}')"
                ))
                .await
                .unwrap();
            let check = check_search_index_lag(&ctx, &settings).await;
            if expected == 0 {
                assert_eq!(check.status, ReadinessStatus::Ok, "{indexed} -> {updated}");
            } else {
                assert_eq!(
                    check.status,
                    ReadinessStatus::Degraded,
                    "{indexed} -> {updated}"
                );
                assert!(
                    check
                        .reason
                        .as_deref()
                        .unwrap()
                        .contains(&format!("lag {expected}s"))
                );
            }
        }
    }

    #[tokio::test]
    async fn readiness_checks_run_concurrently() {
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
        let pending: Vec<BoxFuture<'static, (bool, ReadinessCheck)>> = (0..2)
            .map(|_| {
                let barrier = barrier.clone();
                async move {
                    barrier.wait().await;
                    (
                        false,
                        check(
                            "parallel",
                            DependencyCriticality::Critical,
                            ReadinessStatus::Ok,
                            None,
                        ),
                    )
                }
                .boxed()
            })
            .collect();
        let checks = tokio::time::timeout(Duration::from_secs(1), run_readiness_checks(pending))
            .await
            .expect("checks should run concurrently");
        assert_eq!(checks.len(), 2);
    }

    fn check(
        name: &str,
        criticality: DependencyCriticality,
        status: ReadinessStatus,
        reason: Option<&str>,
    ) -> ReadinessCheck {
        ReadinessCheck {
            name: name.to_string(),
            kind: "dependency",
            criticality,
            status,
            latency_ms: 1,
            reason: reason.map(str::to_string),
        }
    }

    #[test]
    fn aggregate_is_unhealthy_when_critical_dependency_is_unhealthy() {
        let checks = vec![check(
            "database",
            DependencyCriticality::Critical,
            ReadinessStatus::Unhealthy,
            Some("db down"),
        )];

        let status = aggregate_status(&checks, &[]);

        assert_eq!(status, ReadinessStatus::Unhealthy);
    }

    #[test]
    fn aggregate_is_degraded_when_only_non_critical_dependency_fails() {
        let checks = vec![check(
            "search",
            DependencyCriticality::NonCritical,
            ReadinessStatus::Degraded,
            Some("timeout"),
        )];

        let status = aggregate_status(&checks, &[]);

        assert_eq!(status, ReadinessStatus::Degraded);
    }

    #[test]
    fn aggregate_is_degraded_when_non_critical_module_is_degraded() {
        let modules = vec![ReadinessCheck {
            name: "module:blog".to_string(),
            kind: "module",
            criticality: DependencyCriticality::NonCritical,
            status: ReadinessStatus::Degraded,
            latency_ms: 1,
            reason: Some("module reported degraded".to_string()),
        }];

        let status = aggregate_status(&[], &modules);

        assert_eq!(status, ReadinessStatus::Degraded);
    }

    #[tokio::test]
    async fn guarded_check_times_out_and_degrades_non_critical_dependency() {
        let result = run_guarded_check(
            "slow_non_critical",
            DependencyCriticality::NonCritical,
            "dependency",
            || async {
                tokio::time::sleep(HEALTH_CHECK_TIMEOUT + Duration::from_millis(50)).await;
                Ok(())
            },
        )
        .await;

        assert_eq!(result.status, ReadinessStatus::Degraded);
        assert!(
            result
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("timed out"))
        );
    }

    #[test]
    fn reasons_collect_context_for_degradation() {
        let checks = vec![check(
            "search",
            DependencyCriticality::NonCritical,
            ReadinessStatus::Degraded,
            Some("connect error"),
        )];

        let reasons = collect_reasons(&checks, &[]);

        assert_eq!(reasons.len(), 1);
        assert!(reasons[0].contains("search"));
        assert!(reasons[0].contains("connect error"));
    }

    #[test]
    fn parse_host_port_supports_scheme_and_explicit_port() {
        let (host, port) = parse_host_port("http://localhost:7700/search").expect("valid url");
        assert_eq!(host, "localhost");
        assert_eq!(port, 7700);
    }

    #[test]
    fn parse_host_port_uses_https_default_port() {
        let (host, port) =
            parse_host_port("https://search.example.test/search").expect("valid url");
        assert_eq!(host, "search.example.test");
        assert_eq!(port, 443);
    }

    #[test]
    fn parse_host_port_strips_userinfo_and_supports_bracketed_ipv6() {
        let (host, port) =
            parse_host_port("https://user:secret@[2001:db8::1]:9443/search").expect("valid url");
        assert_eq!(host, "2001:db8::1");
        assert_eq!(port, 9443);
    }

    #[test]
    fn tenant_invalidation_listener_status_codes_are_stable() {
        let healthy = TenantInvalidationListenerSnapshot {
            status: TenantInvalidationListenerStatus::Healthy,
            last_error: None,
            redis_required: false,
            local_ready: true,
            subscriber_ready: true,
            reconciliation_healthy: true,
        };
        let degraded = TenantInvalidationListenerSnapshot {
            status: TenantInvalidationListenerStatus::Degraded,
            last_error: Some("redis unavailable".to_string()),
            redis_required: true,
            local_ready: true,
            subscriber_ready: false,
            reconciliation_healthy: false,
        };

        assert_eq!(healthy.status.metric_value(), 2);
        assert_eq!(degraded.status.metric_value(), 3);
    }

    #[test]
    fn readiness_profile_only_requires_local_workers_in_full_and_worker_modes() {
        for (host_mode, expected) in [
            (crate::common::settings::RuntimeHostMode::Full, true),
            (crate::common::settings::RuntimeHostMode::Worker, true),
            (crate::common::settings::RuntimeHostMode::Api, false),
            (crate::common::settings::RuntimeHostMode::AdminSsr, false),
            (
                crate::common::settings::RuntimeHostMode::StorefrontSsr,
                false,
            ),
            (
                crate::common::settings::RuntimeHostMode::RegistryOnly,
                false,
            ),
        ] {
            let mut settings = RustokSettings::default();
            settings.runtime.host_mode = host_mode;
            let profile = ReadinessProfile::from_settings(&settings);
            assert_eq!(profile.includes_local_worker_checks(), expected);
        }
    }

    #[test]
    fn surface_host_mode_worker_check_is_non_critical() {
        let check = worker_host_mode_check();
        assert_eq!(check.status, ReadinessStatus::Ok);
        assert_eq!(check.criticality, DependencyCriticality::NonCritical);
        assert_eq!(aggregate_status(&[check], &[]), ReadinessStatus::Ok);
    }

    #[test]
    fn readiness_profile_skips_runtime_dependencies_for_registry_only_mode() {
        let mut settings = RustokSettings::default();
        settings.runtime.host_mode = crate::common::settings::RuntimeHostMode::RegistryOnly;

        let profile = ReadinessProfile::from_settings(&settings);

        assert!(!profile.includes_runtime_dependencies());
        assert!(!profile.includes_module_health());
    }

    #[test]
    fn required_database_schema_tables_follow_runtime_features() {
        let mut settings = RustokSettings::default();
        settings.features.search_indexing = false;

        assert_eq!(
            required_database_schema_tables(&settings),
            vec!["tenants", "users", "sys_events"]
        );

        settings.features.search_indexing = true;
        assert_eq!(
            required_database_schema_tables(&settings),
            vec!["tenants", "users", "sys_events", "search_documents"]
        );
    }

    #[tokio::test]
    async fn dispatcher_readiness_tracks_registration_and_task_liveness() {
        use crate::services::module_event_dispatcher::{
            ModuleEventDispatcherHandle, ModuleEventDispatcherRequirement,
        };
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let check = || {
            check_runtime_workers(&ctx, ctx.settings())
                .into_iter()
                .find(|check| check.name == "worker:module_event_dispatcher")
                .unwrap()
        };
        // An uninitialized listener set must not be mistaken for no listeners.
        assert_eq!(check().status, ReadinessStatus::Unhealthy);
        ctx.shared_insert(ModuleEventDispatcherRequirement(false));
        assert_eq!(check().status, ReadinessStatus::Ok);
        assert_eq!(check().criticality, DependencyCriticality::NonCritical);
        ctx.shared_insert(ModuleEventDispatcherRequirement(true));
        assert_eq!(check().status, ReadinessStatus::Unhealthy);

        ctx.shared_insert(ModuleEventDispatcherHandle::for_test());
        assert_eq!(check().status, ReadinessStatus::Ok);
        assert!(
            ctx.shared_map::<ModuleEventDispatcherHandle, _>(ModuleEventDispatcherHandle::abort)
                .is_some()
        );
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while ctx.shared_map::<ModuleEventDispatcherHandle, _>(
                ModuleEventDispatcherHandle::is_finished,
            ) != Some(true)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(check().status, ReadinessStatus::Unhealthy);
    }

    #[tokio::test]
    async fn module_work_scheduler_readiness_tracks_registration_and_liveness() {
        use crate::services::app_lifecycle::{
            ModuleWorkSchedulerHandle, ModuleWorkSchedulerRequirement, StopHandle,
        };
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let check = || {
            check_runtime_workers(&ctx, ctx.settings())
                .into_iter()
                .find(|check| check.name == "worker:module_work_scheduler")
                .unwrap()
        };
        // Unknown registration state is not equivalent to an empty set.
        assert_eq!(check().status, ReadinessStatus::Unhealthy);
        ctx.shared_insert(ModuleWorkSchedulerRequirement(false));
        assert_eq!(check().status, ReadinessStatus::Ok);
        assert_eq!(check().criticality, DependencyCriticality::NonCritical);
        ctx.shared_insert(ModuleWorkSchedulerRequirement(true));
        assert_eq!(check().status, ReadinessStatus::Unhealthy);

        let task = tokio::spawn(std::future::pending::<()>());
        let abort = task.abort_handle();
        ctx.shared_insert(ModuleWorkSchedulerHandle::new(task));
        assert_eq!(check().status, ReadinessStatus::Ok);
        abort.abort();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while ctx
                .shared_map::<ModuleWorkSchedulerHandle, _>(ModuleWorkSchedulerHandle::is_finished)
                != Some(true)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(check().status, ReadinessStatus::Unhealthy);

        // Shutdown must fail readiness even before the task has exited.
        let task = tokio::spawn(std::future::pending::<()>());
        let abort = task.abort_handle();
        ctx.shared_insert(ModuleWorkSchedulerHandle::new(task));
        assert_eq!(check().status, ReadinessStatus::Ok);
        let stop = StopHandle::ensure(&ctx);
        stop.stop().await;
        assert_eq!(check().status, ReadinessStatus::Unhealthy);
        abort.abort();
    }

    #[cfg(feature = "mod-payment")]
    #[tokio::test]
    async fn payment_recovery_readiness_tracks_a_real_worker() {
        use crate::services::payment_provider_event_worker::{
            PaymentProviderEventWorkerHandle, spawn_payment_provider_event_worker,
        };
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let check = || {
            check_runtime_workers(&ctx, ctx.settings())
                .into_iter()
                .find(|check| check.name == "worker:payment_provider_event_recovery")
                .unwrap()
        };
        assert_eq!(check().status, ReadinessStatus::Unhealthy);
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        ctx.shared_insert(spawn_payment_provider_event_worker(ctx.clone(), stop_rx));
        assert_eq!(check().status, ReadinessStatus::Ok);
        stop_tx.send(true).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while ctx.shared_map::<PaymentProviderEventWorkerHandle, _>(
                PaymentProviderEventWorkerHandle::is_finished,
            ) != Some(true)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(check().status, ReadinessStatus::Unhealthy);
    }

    #[test]
    fn required_runtime_worker_missing_is_unhealthy() {
        let check = runtime_worker_check("worker:outbox_relay", true, None, false);

        assert_eq!(check.kind, "worker");
        assert_eq!(check.criticality, DependencyCriticality::Critical);
        assert_eq!(check.status, ReadinessStatus::Unhealthy);
        assert!(
            check
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("missing"))
        );
    }

    #[test]
    fn required_runtime_worker_finished_is_unhealthy() {
        let check = runtime_worker_check("worker:outbox_relay", true, Some(true), false);

        assert_eq!(check.criticality, DependencyCriticality::Critical);
        assert_eq!(check.status, ReadinessStatus::Unhealthy);
        assert!(
            check
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("failed"))
        );
    }

    #[test]
    fn required_runtime_worker_running_is_ready() {
        let check = runtime_worker_check("worker:outbox_relay", true, Some(false), false);

        assert_eq!(check.criticality, DependencyCriticality::Critical);
        assert_eq!(check.status, ReadinessStatus::Ok);
        assert!(check.reason.is_none());
    }

    #[test]
    fn required_runtime_worker_stopping_is_unhealthy() {
        let check = runtime_worker_check("worker:outbox_relay", true, Some(false), true);

        assert_eq!(check.criticality, DependencyCriticality::Critical);
        assert_eq!(check.status, ReadinessStatus::Unhealthy);
        assert!(
            check
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("stopping"))
        );
    }

    #[test]
    fn email_backend_is_degraded_when_disabled() {
        let mut settings = RustokSettings::default();
        settings.email.provider = EmailProvider::None;

        let check = email_backend_check(&settings);

        assert_eq!(check.name, "email_backend");
        assert_eq!(check.criticality, DependencyCriticality::NonCritical);
        assert_eq!(check.status, ReadinessStatus::Degraded);
        assert!(
            check
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("disabled"))
        );
    }

    #[test]
    fn email_backend_is_ready_when_smtp_enabled() {
        let mut settings = RustokSettings::default();
        settings.email.provider = EmailProvider::Smtp;
        settings.email.enabled = true;

        let check = email_backend_check(&settings);

        assert_eq!(check.status, ReadinessStatus::Ok);
        assert!(check.reason.is_none());
    }

    #[test]
    fn email_backend_is_degraded_when_smtp_is_disabled() {
        let mut settings = RustokSettings::default();
        settings.email.provider = EmailProvider::Smtp;
        settings.email.enabled = false;

        let check = email_backend_check(&settings);

        assert_eq!(check.status, ReadinessStatus::Degraded);
        assert!(
            check
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("disabled"))
        );
    }

    #[test]
    fn readiness_http_status_maps_unhealthy_to_service_unavailable() {
        assert_eq!(
            super::readiness_http_status(ReadinessStatus::Ok),
            StatusCode::OK
        );
        assert_eq!(
            super::readiness_http_status(ReadinessStatus::Degraded),
            StatusCode::OK
        );
        assert_eq!(
            super::readiness_http_status(ReadinessStatus::Unhealthy),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn resolve_modules_health_status_distinguishes_tri_state() {
        assert_eq!(super::resolve_modules_health_status(false, false), "ok");
        assert_eq!(
            super::resolve_modules_health_status(false, true),
            "degraded"
        );
        assert_eq!(
            super::resolve_modules_health_status(true, false),
            "unhealthy"
        );
        assert_eq!(
            super::resolve_modules_health_status(true, true),
            "unhealthy"
        );
    }
}
