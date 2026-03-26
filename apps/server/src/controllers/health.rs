//! Health check endpoints for K8s probes and module health aggregation

use crate::error::Result;
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Extension;
use loco_rs::app::AppContext;
use loco_rs::controller::format;
use loco_rs::controller::Routes;
use once_cell::sync::Lazy;
use rustok_core::{HealthStatus, ModuleRegistry};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use utoipa::ToSchema;

use crate::common::settings::RustokSettings;
use crate::middleware::rate_limit::{
    SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter,
};
use crate::middleware::tenant::{
    tenant_invalidation_listener_snapshot, TenantInvalidationListenerStatus,
};
use crate::services::runtime_guardrails::{
    collect_runtime_guardrail_snapshot, RuntimeGuardrailSnapshot, RuntimeGuardrailStatus,
};

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
    format::json(HealthResponse {
        status: "ok",
        app: "rustok",
        version: env!("CARGO_PKG_VERSION"),
    })
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
    format::json(serde_json::json!({ "status": "ok" }))
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
    State(ctx): State<AppContext>,
    Extension(registry): Extension<ModuleRegistry>,
) -> Result<Response> {
    let settings = RustokSettings::from_settings(&ctx.config.settings).unwrap_or_default();

    let mut checks = vec![
        run_guarded_check(
            "database",
            DependencyCriticality::Critical,
            "dependency",
            || check_database(&ctx.db),
        )
        .await,
        run_guarded_check(
            "cache_backend",
            DependencyCriticality::NonCritical,
            "dependency",
            || check_cache_backend(&ctx),
        )
        .await,
        run_guarded_check(
            "tenant_cache_invalidation",
            DependencyCriticality::NonCritical,
            "dependency",
            || check_tenant_invalidation_listener(&ctx),
        )
        .await,
        run_guarded_check(
            "event_transport",
            DependencyCriticality::Critical,
            "dependency",
            || check_event_transport(&ctx),
        )
        .await,
    ];

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

    checks.push(check_runtime_guardrails(&ctx).await);
    checks.push(check_search_backend(&settings.search).await);

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

    let mut module_checks = Vec::new();
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

    let status = aggregate_status(&checks, &module_checks);
    let degraded_reasons = collect_reasons(&checks, &module_checks);

    format::json(ReadinessResponse {
        status,
        checks,
        modules: module_checks,
        degraded_reasons,
    })
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
pub async fn runtime(State(ctx): State<AppContext>) -> Result<Response> {
    let snapshot = collect_runtime_guardrail_snapshot(&ctx).await;
    format::json(snapshot)
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

    format::json(ModulesHealthResponse {
        status: if overall_healthy { "ok" } else { "degraded" },
        modules: modules_health,
    })
}

#[cfg(feature = "mod-media")]
async fn check_storage_backend(ctx: &AppContext) -> std::result::Result<(), String> {
    use rustok_storage::StorageService;

    let Some(storage) = ctx.shared_store.get::<StorageService>() else {
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
    use sea_orm::ConnectionTrait;

    db.execute_unprepared("SELECT 1")
        .await
        .map(|_| ())
        .map_err(|error| format!("database check failed: {error}"))
}

async fn check_cache_backend(ctx: &AppContext) -> std::result::Result<(), String> {
    use rustok_cache::CacheService;

    let Some(cache) = ctx.shared_store.get::<CacheService>() else {
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

async fn check_tenant_invalidation_listener(ctx: &AppContext) -> std::result::Result<(), String> {
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

async fn check_event_transport(ctx: &AppContext) -> std::result::Result<(), String> {
    use rustok_core::events::EventTransport;
    use std::sync::Arc;

    if let Some(runtime) = ctx
        .shared_store
        .get::<Arc<crate::services::event_transport_factory::EventRuntime>>()
    {
        if runtime.relay_fallback_active {
            return Err(
                "event relay target is degraded: fallback-to-memory mode is active".to_string(),
            );
        }
    }

    ctx.shared_store
        .get::<Arc<dyn EventTransport>>()
        .map(|_| ())
        .ok_or_else(|| "event transport not initialized in shared_store".to_string())
}

async fn check_rate_limit_backend(
    ctx: &AppContext,
    namespace: &'static str,
) -> std::result::Result<(), String> {
    match namespace {
        "api" => ctx
            .shared_store
            .get::<SharedApiRateLimiter>()
            .ok_or_else(|| "API rate limiter not initialized in shared_store".to_string())?
            .0
            .check_backend_health()
            .await
            .map_err(|error| format!("api rate-limit backend check failed: {error}")),
        "auth" => ctx
            .shared_store
            .get::<SharedAuthRateLimiter>()
            .ok_or_else(|| "auth rate limiter not initialized in shared_store".to_string())?
            .0
            .check_backend_health()
            .await
            .map_err(|error| format!("auth rate-limit backend check failed: {error}")),
        "oauth" => ctx
            .shared_store
            .get::<SharedOAuthRateLimiter>()
            .ok_or_else(|| "oauth rate limiter not initialized in shared_store".to_string())?
            .0
            .check_backend_health()
            .await
            .map_err(|error| format!("oauth rate-limit backend check failed: {error}")),
        _ => Err(format!("unknown rate-limit namespace: {namespace}")),
    }
}

async fn check_search_backend(search: &crate::common::settings::SearchSettings) -> ReadinessCheck {
    if !search.enabled {
        return ReadinessCheck {
            name: "search_backend".to_string(),
            kind: "dependency",
            criticality: DependencyCriticality::NonCritical,
            status: ReadinessStatus::Ok,
            latency_ms: 0,
            reason: Some("search disabled".to_string()),
        };
    }

    run_guarded_check(
        "search_backend",
        DependencyCriticality::NonCritical,
        "dependency",
        || async {
            let (host, port) = parse_host_port(&search.url)?;
            TcpStream::connect((host.as_str(), port))
                .await
                .map(|_| ())
                .map_err(|error| format!("search connect error: {error}"))
        },
    )
    .await
}

async fn check_runtime_guardrails(ctx: &AppContext) -> ReadinessCheck {
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

pub fn routes() -> Routes {
    Routes::new()
        .prefix("health")
        .add("/", get(health))
        .add("/live", get(live))
        .add("/ready", get(ready))
        .add("/runtime", get(runtime))
        .add("/modules", get(modules))
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
        assert!(result
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("timed out")));
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
        };
        let degraded = TenantInvalidationListenerSnapshot {
            status: TenantInvalidationListenerStatus::Degraded,
            last_error: Some("redis unavailable".to_string()),
        };

        assert_eq!(healthy.status.metric_value(), 2);
        assert_eq!(degraded.status.metric_value(), 3);
    }
}
