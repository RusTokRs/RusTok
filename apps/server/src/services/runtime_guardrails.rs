use loco_rs::app::AppContext;
use rustok_core::events::BackpressureState;
use serde::Serialize;
use utoipa::ToSchema;

use crate::common::settings::{GuardrailRolloutMode, RustokSettings};
use crate::middleware::rate_limit::{
    SharedApiRateLimiter, SharedAuthRateLimiter, SharedOAuthRateLimiter,
};
use crate::services::app_lifecycle::{
    RegistryRemoteExecutorReaperWorkerHandle, RegistryValidationStageWorkerHandle,
};
use crate::services::event_bus::SharedEventBus;
use crate::services::event_transport_factory::EventRuntime;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGuardrailStatus {
    Ok,
    Degraded,
    Critical,
}

impl RuntimeGuardrailStatus {
    pub fn metric_value(self) -> i64 {
        match self {
            Self::Ok => 0,
            Self::Degraded => 1,
            Self::Critical => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeGuardrailRollout {
    Observe,
    Enforce,
}

impl RuntimeGuardrailRollout {
    pub fn metric_value(self) -> i64 {
        match self {
            Self::Observe => 0,
            Self::Enforce => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RuntimeGuardrailSnapshot {
    pub status: RuntimeGuardrailStatus,
    pub observed_status: RuntimeGuardrailStatus,
    pub rollout: RuntimeGuardrailRollout,
    pub host_mode: String,
    pub runtime_dependencies_enabled: bool,
    pub reasons: Vec<String>,
    pub rate_limits: Vec<RateLimitGuardrailSnapshot>,
    pub event_bus: EventBusGuardrailSnapshot,
    pub event_transport: EventTransportGuardrailSnapshot,
    pub validation_runner: ValidationRunnerGuardrailSnapshot,
    pub remote_executor: RemoteExecutorGuardrailSnapshot,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RateLimitGuardrailSnapshot {
    pub namespace: &'static str,
    pub backend: &'static str,
    pub distributed: bool,
    pub policy: RateLimitPolicySnapshot,
    pub active_clients: usize,
    pub total_entries: usize,
    pub healthy: bool,
    pub state: RuntimeGuardrailStatus,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RateLimitPolicySnapshot {
    pub enabled: bool,
    pub max_requests: usize,
    pub window_seconds: u64,
    pub trusted_auth_dimensions: bool,
    pub memory_warning_entries: usize,
    pub memory_critical_entries: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EventBusGuardrailSnapshot {
    pub backpressure_enabled: bool,
    pub current_depth: usize,
    pub max_depth: usize,
    pub state: RuntimeGuardrailStatus,
    pub events_rejected: u64,
    pub warning_count: u64,
    pub critical_count: u64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EventTransportGuardrailSnapshot {
    pub relay_fallback_active: bool,
    pub channel_capacity: usize,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ValidationRunnerGuardrailSnapshot {
    pub configured_enabled: bool,
    pub active: bool,
    pub worker_attached: bool,
    pub instance_id: Option<u64>,
    pub auto_confirm_manual_review: bool,
    pub poll_interval_ms: u64,
    pub actor: String,
    pub supported_stages: Vec<String>,
    pub state: RuntimeGuardrailStatus,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RemoteExecutorGuardrailSnapshot {
    pub configured_enabled: bool,
    pub active: bool,
    pub token_configured: bool,
    pub reaper_attached: bool,
    pub reaper_instance_id: Option<u64>,
    pub lease_ttl_ms: u64,
    pub requeue_scan_interval_ms: u64,
    pub state: RuntimeGuardrailStatus,
}

pub async fn collect_runtime_guardrail_snapshot(ctx: &AppContext) -> RuntimeGuardrailSnapshot {
    let settings = RustokSettings::from_settings(&ctx.config.settings).unwrap_or_default();
    let policy = runtime_guardrail_policy_from_settings(&settings);
    let mut reasons = Vec::new();
    let mut observed_status = RuntimeGuardrailStatus::Ok;

    let mut rate_limits = Vec::new();
    if let Some(shared) = ctx.shared_store.get::<SharedApiRateLimiter>() {
        let snapshot = collect_rate_limit_snapshot("api", &shared.0, &policy).await;
        if !snapshot.healthy {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Critical,
                &mut reasons,
                format!(
                    "api rate-limit backend `{}` is unavailable",
                    snapshot.backend
                ),
            );
        }
        if snapshot.state == RuntimeGuardrailStatus::Degraded {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Degraded,
                &mut reasons,
                format!(
                    "{} rate-limit memory saturation warning: {} entries",
                    snapshot.namespace, snapshot.total_entries
                ),
            );
        } else if snapshot.state == RuntimeGuardrailStatus::Critical {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Critical,
                &mut reasons,
                format!(
                    "{} rate-limit memory saturation critical: {} entries",
                    snapshot.namespace, snapshot.total_entries
                ),
            );
        }
        rate_limits.push(snapshot);
    }

    if let Some(shared) = ctx.shared_store.get::<SharedAuthRateLimiter>() {
        let snapshot = collect_rate_limit_snapshot("auth", &shared.0, &policy).await;
        if !snapshot.healthy {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Critical,
                &mut reasons,
                format!(
                    "auth rate-limit backend `{}` is unavailable",
                    snapshot.backend
                ),
            );
        }
        if snapshot.state == RuntimeGuardrailStatus::Degraded {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Degraded,
                &mut reasons,
                format!(
                    "{} rate-limit memory saturation warning: {} entries",
                    snapshot.namespace, snapshot.total_entries
                ),
            );
        } else if snapshot.state == RuntimeGuardrailStatus::Critical {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Critical,
                &mut reasons,
                format!(
                    "{} rate-limit memory saturation critical: {} entries",
                    snapshot.namespace, snapshot.total_entries
                ),
            );
        }
        rate_limits.push(snapshot);
    }

    if let Some(shared) = ctx.shared_store.get::<SharedOAuthRateLimiter>() {
        let snapshot = collect_rate_limit_snapshot("oauth", &shared.0, &policy).await;
        if !snapshot.healthy {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Critical,
                &mut reasons,
                format!(
                    "oauth rate-limit backend `{}` is unavailable",
                    snapshot.backend
                ),
            );
        }
        if snapshot.state == RuntimeGuardrailStatus::Degraded {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Degraded,
                &mut reasons,
                format!(
                    "{} rate-limit memory saturation warning: {} entries",
                    snapshot.namespace, snapshot.total_entries
                ),
            );
        } else if snapshot.state == RuntimeGuardrailStatus::Critical {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Critical,
                &mut reasons,
                format!(
                    "{} rate-limit memory saturation critical: {} entries",
                    snapshot.namespace, snapshot.total_entries
                ),
            );
        }
        rate_limits.push(snapshot);
    }

    let event_transport = ctx
        .shared_store
        .get::<std::sync::Arc<EventRuntime>>()
        .map(|runtime| EventTransportGuardrailSnapshot {
            relay_fallback_active: runtime.relay_fallback_active,
            channel_capacity: runtime.channel_capacity,
        })
        .unwrap_or(EventTransportGuardrailSnapshot {
            relay_fallback_active: false,
            channel_capacity: 0,
        });

    if event_transport.relay_fallback_active {
        escalate(
            &mut observed_status,
            RuntimeGuardrailStatus::Critical,
            &mut reasons,
            "event relay target is running in fallback mode".to_string(),
        );
    }

    let validation_runner = collect_validation_runner_snapshot(ctx, &settings);
    if validation_runner.state == RuntimeGuardrailStatus::Degraded {
        if validation_runner.configured_enabled && !validation_runner.worker_attached {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Degraded,
                &mut reasons,
                "registry validation runner is enabled but no worker is attached".to_string(),
            );
        } else if !validation_runner.configured_enabled && validation_runner.worker_attached {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Degraded,
                &mut reasons,
                "registry validation runner worker is attached while config is disabled"
                    .to_string(),
            );
        }
    }

    let remote_executor = collect_remote_executor_snapshot(ctx, &settings);
    if remote_executor.state == RuntimeGuardrailStatus::Degraded {
        if remote_executor.configured_enabled && !remote_executor.token_configured {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Degraded,
                &mut reasons,
                "registry remote executor is enabled but no shared token is configured".to_string(),
            );
        } else if remote_executor.active && !remote_executor.reaper_attached {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Degraded,
                &mut reasons,
                "registry remote executor is enabled but no lease reaper worker is attached"
                    .to_string(),
            );
        } else if !remote_executor.configured_enabled && remote_executor.reaper_attached {
            escalate(
                &mut observed_status,
                RuntimeGuardrailStatus::Degraded,
                &mut reasons,
                "registry remote executor reaper worker is attached while config is disabled"
                    .to_string(),
            );
        }
    }

    let event_bus = ctx
        .shared_store
        .get::<SharedEventBus>()
        .and_then(|shared| shared.0.backpressure().map(|bp| bp.metrics()))
        .map(|metrics| {
            let state = match metrics.state {
                BackpressureState::Normal => RuntimeGuardrailStatus::Ok,
                BackpressureState::Warning => RuntimeGuardrailStatus::Degraded,
                BackpressureState::Critical => RuntimeGuardrailStatus::Critical,
            };

            if state == RuntimeGuardrailStatus::Degraded {
                escalate(
                    &mut observed_status,
                    RuntimeGuardrailStatus::Degraded,
                    &mut reasons,
                    format!(
                        "event bus backpressure warning: depth {}/{}",
                        metrics.current_depth, metrics.max_depth
                    ),
                );
            } else if state == RuntimeGuardrailStatus::Critical {
                escalate(
                    &mut observed_status,
                    RuntimeGuardrailStatus::Critical,
                    &mut reasons,
                    format!(
                        "event bus backpressure critical: depth {}/{}",
                        metrics.current_depth, metrics.max_depth
                    ),
                );
            }

            EventBusGuardrailSnapshot {
                backpressure_enabled: true,
                current_depth: metrics.current_depth,
                max_depth: metrics.max_depth,
                state,
                events_rejected: metrics.events_rejected,
                warning_count: metrics.warning_count,
                critical_count: metrics.critical_count,
            }
        })
        .unwrap_or(EventBusGuardrailSnapshot {
            backpressure_enabled: false,
            current_depth: 0,
            max_depth: 0,
            state: RuntimeGuardrailStatus::Ok,
            events_rejected: 0,
            warning_count: 0,
            critical_count: 0,
        });

    let status = match policy.rollout {
        RuntimeGuardrailRollout::Enforce => observed_status,
        RuntimeGuardrailRollout::Observe => {
            if observed_status == RuntimeGuardrailStatus::Ok {
                RuntimeGuardrailStatus::Ok
            } else {
                RuntimeGuardrailStatus::Degraded
            }
        }
    };

    RuntimeGuardrailSnapshot {
        status,
        observed_status,
        rollout: policy.rollout,
        host_mode: if settings.runtime.is_registry_only() {
            "registry_only".to_string()
        } else {
            "full".to_string()
        },
        runtime_dependencies_enabled: !settings.runtime.is_registry_only(),
        reasons,
        rate_limits,
        event_bus,
        event_transport,
        validation_runner,
        remote_executor,
    }
}

fn collect_validation_runner_snapshot(
    ctx: &AppContext,
    settings: &RustokSettings,
) -> ValidationRunnerGuardrailSnapshot {
    let configured_enabled = settings.registry.validation_runner.enabled;
    let active = configured_enabled && !settings.runtime.is_registry_only();
    let worker_handle = ctx
        .shared_store
        .get_ref::<RegistryValidationStageWorkerHandle>();
    let worker_attached = worker_handle.is_some();
    let instance_id = worker_handle.map(|handle| handle.instance_id());
    let supported_stages = supported_validation_runner_stages(
        settings
            .registry
            .validation_runner
            .auto_confirm_manual_review,
    );
    let state = if active && !worker_attached {
        RuntimeGuardrailStatus::Degraded
    } else if !configured_enabled && worker_attached {
        RuntimeGuardrailStatus::Degraded
    } else {
        RuntimeGuardrailStatus::Ok
    };

    ValidationRunnerGuardrailSnapshot {
        configured_enabled,
        active,
        worker_attached,
        instance_id,
        auto_confirm_manual_review: settings
            .registry
            .validation_runner
            .auto_confirm_manual_review,
        poll_interval_ms: settings.registry.validation_runner.poll_interval_ms,
        actor: settings.registry.validation_runner.actor.clone(),
        supported_stages,
        state,
    }
}

fn collect_remote_executor_snapshot(
    ctx: &AppContext,
    settings: &RustokSettings,
) -> RemoteExecutorGuardrailSnapshot {
    let configured_enabled = settings.registry.remote_executor.enabled;
    let token_configured = !settings
        .registry
        .remote_executor
        .shared_token
        .trim()
        .is_empty();
    let active = configured_enabled && token_configured && !settings.runtime.is_registry_only();
    let reaper_handle = ctx
        .shared_store
        .get_ref::<RegistryRemoteExecutorReaperWorkerHandle>();
    let reaper_attached = reaper_handle.is_some();
    let reaper_instance_id = reaper_handle.map(|handle| handle.instance_id());
    let state = if configured_enabled && !token_configured {
        RuntimeGuardrailStatus::Degraded
    } else if active && !reaper_attached {
        RuntimeGuardrailStatus::Degraded
    } else if !configured_enabled && reaper_attached {
        RuntimeGuardrailStatus::Degraded
    } else {
        RuntimeGuardrailStatus::Ok
    };

    RemoteExecutorGuardrailSnapshot {
        configured_enabled,
        active,
        token_configured,
        reaper_attached,
        reaper_instance_id,
        lease_ttl_ms: settings.registry.remote_executor.lease_ttl_ms,
        requeue_scan_interval_ms: settings.registry.remote_executor.requeue_scan_interval_ms,
        state,
    }
}

fn supported_validation_runner_stages(auto_confirm_manual_review: bool) -> Vec<String> {
    let mut stages = vec!["compile_smoke".to_string(), "targeted_tests".to_string()];
    if auto_confirm_manual_review {
        stages.push("security_policy_review".to_string());
    }
    stages
}

async fn collect_rate_limit_snapshot(
    namespace: &'static str,
    limiter: &crate::middleware::rate_limit::RateLimiter,
    policy: &RuntimeGuardrailPolicy,
) -> RateLimitGuardrailSnapshot {
    let stats = limiter.get_stats().await;
    let healthy = limiter.check_backend_health().await.is_ok();
    let namespace_policy = policy.namespace_policy(namespace);
    let state = if !stats.distributed
        && stats.total_entries >= namespace_policy.memory_critical_entries
    {
        RuntimeGuardrailStatus::Critical
    } else if !stats.distributed && stats.total_entries >= namespace_policy.memory_warning_entries {
        RuntimeGuardrailStatus::Degraded
    } else {
        RuntimeGuardrailStatus::Ok
    };

    RateLimitGuardrailSnapshot {
        namespace,
        backend: limiter.backend_kind(),
        distributed: stats.distributed,
        policy: RateLimitPolicySnapshot {
            enabled: limiter.enabled(),
            max_requests: limiter.max_requests(),
            window_seconds: limiter.window_secs(),
            trusted_auth_dimensions: namespace_policy.trusted_auth_dimensions,
            memory_warning_entries: namespace_policy.memory_warning_entries,
            memory_critical_entries: namespace_policy.memory_critical_entries,
        },
        active_clients: stats.active_clients,
        total_entries: stats.total_entries,
        healthy,
        state,
    }
}

#[derive(Debug, Clone)]
struct RuntimeGuardrailPolicy {
    rollout: RuntimeGuardrailRollout,
    api_policy: RateLimitNamespacePolicy,
    auth_policy: RateLimitNamespacePolicy,
    oauth_policy: RateLimitNamespacePolicy,
}

#[derive(Debug, Clone, Copy)]
struct RateLimitNamespacePolicy {
    trusted_auth_dimensions: bool,
    memory_warning_entries: usize,
    memory_critical_entries: usize,
}

impl RuntimeGuardrailPolicy {
    fn namespace_policy(&self, namespace: &str) -> RateLimitNamespacePolicy {
        match namespace {
            "auth" => self.auth_policy,
            "oauth" => self.oauth_policy,
            _ => self.api_policy,
        }
    }
}

fn runtime_guardrail_policy_from_settings(settings: &RustokSettings) -> RuntimeGuardrailPolicy {
    let guardrails = settings.runtime.guardrails.clone();
    let thresholds = guardrails.rate_limit_memory_thresholds;

    RuntimeGuardrailPolicy {
        rollout: match guardrails.rollout {
            GuardrailRolloutMode::Observe => RuntimeGuardrailRollout::Observe,
            GuardrailRolloutMode::Enforce => RuntimeGuardrailRollout::Enforce,
        },
        api_policy: RateLimitNamespacePolicy {
            trusted_auth_dimensions: settings.rate_limit.trusted_auth_dimensions,
            memory_warning_entries: thresholds.api_warning_entries,
            memory_critical_entries: thresholds.api_critical_entries,
        },
        auth_policy: RateLimitNamespacePolicy {
            trusted_auth_dimensions: settings.rate_limit.trusted_auth_dimensions,
            memory_warning_entries: thresholds.auth_warning_entries,
            memory_critical_entries: thresholds.auth_critical_entries,
        },
        oauth_policy: RateLimitNamespacePolicy {
            trusted_auth_dimensions: settings.rate_limit.trusted_auth_dimensions,
            memory_warning_entries: thresholds.oauth_warning_entries,
            memory_critical_entries: thresholds.oauth_critical_entries,
        },
    }
}

fn escalate(
    current: &mut RuntimeGuardrailStatus,
    next: RuntimeGuardrailStatus,
    reasons: &mut Vec<String>,
    reason: String,
) {
    if !reasons.iter().any(|existing| existing == &reason) {
        reasons.push(reason);
    }

    if severity_rank(next) > severity_rank(*current) {
        *current = next;
    }
}

fn severity_rank(status: RuntimeGuardrailStatus) -> u8 {
    match status {
        RuntimeGuardrailStatus::Ok => 0,
        RuntimeGuardrailStatus::Degraded => 1,
        RuntimeGuardrailStatus::Critical => 2,
    }
}
