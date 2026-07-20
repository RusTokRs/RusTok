//! Health check endpoints for K8s probes and module health aggregation

use crate::error::Result;
use axum::Extension;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use chrono::Utc;
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
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use utoipa::ToSchema;

use crate::common::settings::{EmailProvider, EventTransportKind, RustokSettings};
use crate::middleware::rate_limit::{
    SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter,
};
use crate::middleware::tenant::{
    TenantInvalidationListenerStatus, tenant_invalidation_listener_snapshot,
};
use crate::services::app_lifecycle::{
    OutboxRelayWorkerHandle, RemoteExecutorReaperHandle, RuntimeWorkerLifecycleState, StopHandle,
};
use crate::services::event_transport_factory;
use crate::services::runtime_guardrails::{
    RuntimeGuardrailSnapshot, RuntimeGuardrailStatus, collect_runtime_guardrail_snapshot,
};
use crate::services::server_runtime_context::ServerRuntimeContext;

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);
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
}

impl ReadinessProfile {
    fn from_settings(settings: &RustokSettings) -> Self {
        Self {
            registry_only: settings.runtime.is_registry_only(),
        }
    }

    fn includes_runtime_dependencies(self) -> bool {
        !self.registry_only
    }

    fn includes_module_health(self) -> bool {
        !self.registry_only
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
    Ok(json_response(serde_json::json!({ "status": "ok" })))
}

/// GET /health/ready - K8s readiness probe
/// Checks critical and non-critical infrastructure dependencies and module health.
#[utoipa::path(
    get,
    path = "/health/ready",
    tag = "health",
    responses(
        (status = 200, description = "Readiness status with detailed dependency checks")
    )
)]
pub async fn ready(
    State(ctx): State<ServerRuntimeContext>,
    Extension(registry): Extension<ModuleRegistry>,
) -> Result<Response> {
    let settings = ctx.settings();
    let profile = ReadinessProfile::from_settings(settings);

    let mut checks = vec![
        run_guarded_check(
            "database",
            DependencyCriticality::Critical,
            "dependency",
            || check_database(ctx.db()),
        )
        .await,
        run_guarded_check(
            "database_schema",
            DependencyCriticality::Critical,
            "dependency",
            || check_required_database_schema(&ctx, settings),
        )
        .await,
        run_guarded_check(
            "cache_backend",
            DependencyCriticality::NonCritical,
            "dependency",
            || check_cache_backend(&ctx),
        )
        .await,
    ];

    if profile.includes_runtime_dependencies() {
        checks.push(
            run_guarded_check(
                "tenant_cache_invalidation",
                DependencyCriticality::NonCritical,
                "dependency",
                || check_tenant_invalidation_listener(&ctx),
            )
            .await,
        );
        checks.push(
            run_guarded_check(
                "event_transport",
                DependencyCriticality::Critical,
                "dependency",
                || check_event_transport(&ctx),
            )
            .await,
        );

        if settings.rate_limit.enabled {
            checks.push(
                run_guarded_check(
                    "rate_limit:api",
                    DependencyCriticality::Critical,
                    "dependency",
                    || check_rate_limit_backend(&ctx, "api"),
                )
                .await,
            );
            checks.push(
                run_guarded_check(
                    "rate_limit:auth",
                    DependencyCriticality::Critical,
                    "dependency",
                    || check_rate_limit_backend(&ctx, "auth"),
                )
                .await,
            );
            checks.push(
                run_guarded_check(
                    "rate_limit:oauth",
                    DependencyCriticality::Critical,
                    "dependency",
                    || check_rate_limit_backend(&ctx, "oauth"),
                )
                .await,
            );
        }

        checks.push(
            run_guarded_check(
                "search_backend",
                DependencyCriticality::NonCritical,
                "dependency",
                || async {
                    let (host, port) = parse_host_port(&settings.search.url)?;
                    TcpStream::connect((host.as_str(), port))
                        .await
                        .map(|_| ())
                        .map_err(|error| format!("search connect error: {error}"))
                },
            )
            .await,
        );
        checks.push(check_runtime_guardrails(&ctx).await);
        if settings.events.transport == EventTransportKind::Outbox {
            checks.push(check_outbox_pending_lag(&ctx, settings).await);
        }
        checks.push(check_search_index_lag(&ctx, settings).await);
        checks.push(email_backend_check(settings));
        checks.extend(check_runtime_workers(&ctx, settings));

        #[cfg(feature = "mod-media")]
        checks.push(
            run_guarded_check(
                "storage",
                DependencyCriticality::NonCritical,
                "dependency",
                || check_storage_backend(&ctx),
            )
            .await,
        );
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

    let mut module_checks = Vec::new();
    if profile.includes_module_health() {
        for module in registry.modules() {
            let criticality = if CRITICAL_MODULES.contains(&module.slug()) {
                DependencyCriticality::Critical
            } else {
                DependencyCriticality::NonCritical
            };

            let slug = module.slug().to_string();
            let module_name = format!("module:{slug}");
            let module_health = run_guarded_check(&module_name, criticality, "module", || async {
                match module.health().await {
                    HealthStatus::Healthy => Ok(()),
                    HealthStatus::Degraded => Err("module reported degraded".to_string()),
                    HealthStatus::Unhealthy => Err("module reported unhealthy".to_string()),
                }
            })
            .await;
            module_checks.push(module_health);
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

    let status = aggregate_status(&checks, &module_checks);
    let degraded_reasons = collect_reasons(&checks, &module_checks);

    Ok(json_response(ReadinessResponse {
        status,
        checks,
        modules: module_checks,
        degraded_reasons,
    }))
}

/// GET /health/runtime - Runtime guardrail snapshot for operators
/// Returns the current rollout-aware guardrail state plus component-level details.
#[utoipa::path(
    get,
    path = "/health/runtime",
    tag = "health",
    responses(
        (status = 200, description = "Runtime guardrail snapshot", body = RuntimeGuardrailSnapshot)
    )
)]
pub async fn runtime(State(ctx): State<ServerRuntimeContext>) -> Result<Response> {
    let snapshot = collect_runtime_guardrail_snapshot(&ctx).await;
    Ok(json_response(snapshot))
}

/// GET /health/modules - Module health aggregation
/// Reports health status of all registered modules
#[utoipa::path(
    get,
    path = "/health/modules",
    tag = "health",
    responses(
        (status = 200, description = "Module health statuses", body = ModulesHealthResponse)
    )
)]
pub async fn modules(Extension(registry): Extension<ModuleRegistry>) -> Result<Response> {
    let mut modules_health = Vec::new();
    let mut overall_healthy = true;

    for module in registry.modules() {
        let health = module.health().await;
        let status_str = match health {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => {
                overall_healthy = false;
                "degraded"
            }
            HealthStatus::Unhealthy => {
                overall_healthy = false;
                "unhealthy"
            }
        };

        modules_health.push(ModuleHealth {
            slug: module.slug().to_string(),
            name: module.name().to_string(),
            status: status_str.to_string(),
        });
    }

    Ok(json_response(ModulesHealthResponse {
        status: if overall_healthy { "ok" } else { "degraded" },
        modules: modules_health,
    }))
}

#[cfg(feature = "mod-media")]
async fn check_storage_backend(ctx: &ServerRuntimeContext) -> std::result::Result<(), String> {
    use rustok_storage::StorageService;

    let Some(storage) = ctx.shared_get::<StorageService>() else {
        return Ok(()); // not configured — skip
    };

    let probe = ".health-probe";
    let data = bytes::Bytes::from_static(b"ok");
    storage
        .store(probe, data, "text/plain")
        .await
        .map_err(|e| format!("storage write failed: {e}"))?;
    storage
        .delete(probe)
        .await
        .map_err(|e| format!("storage delete failed: {e}"))?;

    rustok_telemetry::metrics::update_storage_health(storage.backend_name(), true);
    Ok(())
}

async fn check_database(db: &DatabaseConnection) -> std::result::Result<(), String> {
    db.execute_unprepared("SELECT 1")
        .await
        .map(|_| ())
        .map_err(|error| format!("database check failed: {error}"))
}

async fn check_required_database_schema(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
) -> std::result::Result<(), String> {
    for table in required_database_schema_tables(settings) {
        let query = format!("SELECT 1 FROM {table} LIMIT 1");
        if let Err(error) = ctx.db().execute_unprepared(&query).await {
            return Err(format!("required table `{table}` is unavailable: {error}"));
        }
    }

    Ok(())
}

fn required_database_schema_tables(settings: &RustokSettings) -> Vec<&'static str> {
    let mut required_tables = vec!["tenants", "users"];

    if settings.events.transport == EventTransportKind::Outbox {
        required_tables.push("sys_events");
    }

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
    } else {
        Err(report
            .redis_error
            .unwrap_or_else(|| "redis unhealthy".to_string()))
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
        TenantInvalidationListenerStatus::Degraded => Err(snapshot
            .last_error
            .unwrap_or_else(|| "tenant invalidation listener is degraded".to_string())),
    }
}

async fn check_event_transport(ctx: &ServerRuntimeContext) -> std::result::Result<(), String> {
    use rustok_core::events::EventTransport;
    use std::sync::Arc;

    if let Some(runtime) =
        ctx.shared_get::<Arc<crate::services::event_transport_factory::EventRuntime>>()
    {
        if runtime.relay_fallback_active {
            return Err(
                "event relay target is degraded: fallback-to-memory mode is active".to_string(),
            );
        }
    }

    ctx.shared_get::<Arc<dyn EventTransport>>()
        .map(|_| ())
        .ok_or_else(|| "event transport not initialized in shared_store".to_string())
}

async fn check_outbox_pending_lag(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
) -> ReadinessCheck {
    let started_at = Instant::now();
    let threshold = settings.readiness.outbox_max_pending_lag_seconds as i64;
    let result = SysEventsEntity::find()
        .filter(SysEventsColumn::Status.eq(SysEventStatus::Pending))
        .order_by_asc(SysEventsColumn::CreatedAt)
        .one(ctx.db())
        .await;

    let (status, reason) = match result {
        Ok(Some(event)) => {
            let lag_seconds = (Utc::now() - event.created_at).num_seconds().max(0);
            if lag_seconds > threshold {
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
        Err(error) => (
            ReadinessStatus::Degraded,
            Some(format!("outbox lag check failed: {error}")),
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
    let threshold = settings.readiness.search_max_lag_seconds as i64;
    let backend = ctx.db().get_database_backend();
    let stmt = Statement::from_string(backend, search_index_lag_query(backend).to_string());

    let (status, reason) = match ctx.db().query_one(stmt).await {
        Ok(Some(row)) => {
            let lag_seconds = row
                .try_get::<i64>("", "max_lag_seconds")
                .unwrap_or(0)
                .max(0);
            if lag_seconds > threshold {
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
        Err(error) => (
            ReadinessStatus::Degraded,
            Some(format!("search lag check failed: {error}")),
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
            r#"
            SELECT
                CAST(
                    COALESCE(
                        MAX(
                            CASE
                                WHEN updated_at > indexed_at THEN CAST((julianday(updated_at) - julianday(indexed_at)) * 86400 AS INTEGER)
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

fn check_runtime_workers(
    ctx: &ServerRuntimeContext,
    settings: &RustokSettings,
) -> Vec<ReadinessCheck> {
    let relay_required = settings.events.transport == EventTransportKind::Outbox
        && ctx
            .shared_get::<std::sync::Arc<event_transport_factory::EventRuntime>>()
            .and_then(|runtime| runtime.relay_config.clone())
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
        "worker:remote_executor_reaper",
        settings.registry.remote_executor.enabled,
        ctx.shared_map::<RemoteExecutorReaperHandle, _>(RemoteExecutorReaperHandle::is_finished),
        stop_requested,
    ));

    #[cfg(feature = "mod-seo")]
    checks.push(runtime_worker_check(
        "worker:seo_bulk",
        settings.runtime.background_workers.seo_bulk_enabled,
        ctx.shared_map::<crate::services::app_lifecycle::SeoBulkWorkerHandle, _>(
            crate::services::app_lifecycle::SeoBulkWorkerHandle::is_finished,
        ),
        stop_requested,
    ));

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
            .map_err(|error| format!("api rate-limit backend check failed: {error}")),
        "auth" => ctx
            .shared_get::<SharedAuthRateLimiter>()
            .ok_or_else(|| "auth rate limiter not initialized in shared_store".to_string())?
            .0
            .check_backend_health()
            .await
            .map_err(|error| format!("auth rate-limit backend check failed: {error}")),
        "oauth" => ctx
            .shared_get::<SharedOAuthRateLimiter>()
            .ok_or_else(|| "oauth rate limiter not initialized in shared_store".to_string())?
            .0
            .check_backend_health()
            .await
            .map_err(|error| format!("oauth rate-limit backend check failed: {error}")),
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
    if let Some(open_for_ms) = circuit_open_for(name).await {
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
            on_check_success(name).await;
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
            on_check_failure(name).await;
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
            on_check_failure(name).await;
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

async fn circuit_open_for(name: &str) -> Option<u128> {
    let mut state = CIRCUITS.lock().await;
    let now = Instant::now();

    if let Some(circuit) = state.get_mut(name) {
        if let Some(open_until) = circuit.open_until {
            if open_until > now {
                return Some(open_until.duration_since(now).as_millis());
            }

            circuit.open_until = None;
            circuit.consecutive_failures = 0;
        }
    }

    None
}

async fn on_check_success(name: &str) {
    let mut state = CIRCUITS.lock().await;
    state.remove(name);
}

async fn on_check_failure(name: &str) {
    let mut state = CIRCUITS.lock().await;
    let circuit = state.entry(name.to_string()).or_default();
    circuit.consecutive_failures += 1;

    if circuit.consecutive_failures >= CIRCUIT_BREAKER_FAILURE_THRESHOLD {
        circuit.open_until = Some(Instant::now() + CIRCUIT_BREAKER_COOLDOWN);
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
    let without_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);

    let authority = without_scheme.split('/').next().unwrap_or_default().trim();

    if authority.is_empty() {
        return Err("search URL is empty".to_string());
    }

    if let Some((host, port)) = authority.rsplit_once(':') {
        let port = port
            .parse::<u16>()
            .map_err(|_| format!("invalid search port: {port}"))?;
        return Ok((host.to_string(), port));
    }

    Ok((authority.to_string(), 80))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/health/", get(health))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/health/runtime", get(runtime))
        .route("/health/modules", get(modules))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::tenant::TenantInvalidationListenerSnapshot;

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
            vec!["tenants", "users"]
        );

        settings.events.transport = EventTransportKind::Outbox;
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
}
