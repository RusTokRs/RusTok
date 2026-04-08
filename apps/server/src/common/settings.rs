use ipnet::IpNet;
use rustok_core::tenant_validation::TenantIdentifierValidator;
use rustok_iggy::IggyConfig;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

#[cfg(feature = "mod-media")]
use rustok_storage::StorageConfig;

use crate::services::redis_runtime::resolve_redis_url;

const DEFAULT_TENANT_ID: Uuid = Uuid::from_u128(1);

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RustokSettings {
    #[serde(default)]
    pub tenant: TenantSettings,
    #[serde(default)]
    pub build: BuildRuntimeSettings,
    #[serde(default)]
    pub search: SearchSettings,
    #[serde(default)]
    pub features: FeatureSettings,
    #[serde(default)]
    pub rate_limit: RateLimitSettings,
    #[serde(default)]
    pub events: EventSettings,
    #[serde(default)]
    pub email: EmailSettings,
    #[serde(default)]
    pub cache: CacheSettings,
    #[serde(default)]
    pub registry: RegistrySettings,
    #[serde(default)]
    pub runtime: RuntimeSettings,
    #[cfg(feature = "mod-media")]
    #[serde(default)]
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistrySettings {
    #[serde(default)]
    pub validation_runner: RegistryValidationRunnerSettings,
    #[serde(default)]
    pub remote_executor: RegistryRemoteExecutorSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryValidationRunnerSettings {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegistryRemoteExecutorSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub shared_token: Option<String>,
    #[serde(default = "default_registry_remote_executor_lease_ttl_ms")]
    pub lease_ttl_ms: u64,
    #[serde(default = "default_registry_remote_executor_requeue_scan_interval_ms")]
    pub requeue_scan_interval_ms: u64,
}

/// Cache configuration.
///
/// `redis_url` overrides `RUSTOK_REDIS_URL` / `REDIS_URL` env vars when set.
/// This lets ops teams set Redis URL via YAML config instead of env, useful in
/// containerised deployments where config files are preferred over env injection.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CacheSettings {
    /// Explicit Redis URL. When absent, falls back to `RUSTOK_REDIS_URL` → `REDIS_URL` env vars.
    pub redis_url: Option<String>,
}

/// Email transport provider selector.
///
/// - `smtp` (default): sends via lettre directly using the `[email.smtp]` config
/// - `loco`: sends via Loco Mailer (`ctx.mailer`) + Tera templates
/// - `none`: email sending is disabled
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmailProvider {
    #[default]
    Smtp,
    Loco,
    None,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmailSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: EmailProvider,
    #[serde(default)]
    pub smtp: SmtpSettings,
    #[serde(default = "default_email_from")]
    pub from: String,
    #[serde(default = "default_reset_base_url")]
    pub reset_base_url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SmtpSettings {
    #[serde(default = "default_smtp_host")]
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

impl Default for SmtpSettings {
    fn default() -> Self {
        Self {
            host: default_smtp_host(),
            port: default_smtp_port(),
            username: String::new(),
            password: String::new(),
        }
    }
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: EmailProvider::Smtp,
            smtp: SmtpSettings::default(),
            from: default_email_from(),
            reset_base_url: default_reset_base_url(),
        }
    }
}

impl Default for RegistrySettings {
    fn default() -> Self {
        Self {
            validation_runner: RegistryValidationRunnerSettings::default(),
            remote_executor: RegistryRemoteExecutorSettings::default(),
        }
    }
}

impl Default for RegistryValidationRunnerSettings {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl Default for RegistryRemoteExecutorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            shared_token: None,
            lease_ttl_ms: default_registry_remote_executor_lease_ttl_ms(),
            requeue_scan_interval_ms: default_registry_remote_executor_requeue_scan_interval_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventSettings {
    #[serde(default)]
    pub transport: EventTransportKind,
    #[serde(default)]
    pub relay_target: RelayTargetKind,
    #[serde(default)]
    pub allow_relay_target_fallback: bool,
    #[serde(default = "default_relay_interval_ms")]
    pub relay_interval_ms: u64,
    #[serde(default = "default_event_channel_capacity")]
    pub channel_capacity: usize,
    #[serde(default)]
    pub relay_retry_policy: RelayRetryPolicy,
    #[serde(default)]
    pub dlq: DlqSettings,
    #[serde(default)]
    pub backpressure: EventBackpressureSettings,
    #[serde(default)]
    pub iggy: IggyConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EventBackpressureSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_backpressure_max_queue_depth")]
    pub max_queue_depth: usize,
    #[serde(default = "default_backpressure_warning_threshold")]
    pub warning_threshold: f64,
    #[serde(default = "default_backpressure_critical_threshold")]
    pub critical_threshold: f64,
}

impl Default for EventBackpressureSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            max_queue_depth: default_backpressure_max_queue_depth(),
            warning_threshold: default_backpressure_warning_threshold(),
            critical_threshold: default_backpressure_critical_threshold(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelayRetryPolicy {
    #[serde(default = "default_relay_max_attempts")]
    pub max_attempts: i32,
    #[serde(default = "default_relay_backoff_base_ms")]
    pub base_backoff_ms: u64,
    #[serde(default = "default_relay_backoff_max_ms")]
    pub max_backoff_ms: u64,
}

impl Default for RelayRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: default_relay_max_attempts(),
            base_backoff_ms: default_relay_backoff_base_ms(),
            max_backoff_ms: default_relay_backoff_max_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DlqSettings {
    #[serde(default = "default_dlq_enabled")]
    pub enabled: bool,
    #[serde(default = "default_dlq_max_attempts")]
    pub max_attempts: i32,
}

impl Default for DlqSettings {
    fn default() -> Self {
        Self {
            enabled: default_dlq_enabled(),
            max_attempts: default_dlq_max_attempts(),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RelayTargetKind {
    #[default]
    Memory,
    Iggy,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EventTransportKind {
    #[default]
    Memory,
    Outbox,
    Iggy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TenantSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_resolution")]
    pub resolution: String,
    #[serde(default = "default_header_name")]
    pub header_name: String,
    #[serde(default = "default_tenant_id")]
    pub default_id: Uuid,
    #[serde(default)]
    pub fallback_mode: TenantFallbackMode,
    #[serde(default)]
    pub base_domains: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureSettings {
    #[serde(default = "default_true")]
    pub registration_enabled: bool,
    #[serde(default)]
    pub email_verification: bool,
    #[serde(default = "default_true")]
    pub multi_tenant: bool,
    #[serde(default = "default_true")]
    pub search_indexing: bool,
    #[serde(default)]
    pub oauth_enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_search_driver")]
    pub driver: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_index_prefix")]
    pub index_prefix: String,
    #[serde(default)]
    pub reindex: SearchReindexSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchReindexSettings {
    #[serde(default = "default_search_reindex_parallelism")]
    pub parallelism: usize,
    #[serde(default = "default_search_reindex_entity_budget")]
    pub entity_budget: usize,
    #[serde(default = "default_search_reindex_yield_every")]
    pub yield_every: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BuildRuntimeSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_build_poll_interval_ms")]
    pub poll_interval_ms: u64,
    #[serde(default)]
    pub auto_release_environment: Option<String>,
    #[serde(default)]
    pub auto_activate_release: bool,
    #[serde(default)]
    pub deployment: BuildDeploymentSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BuildDeploymentSettings {
    #[serde(default)]
    pub backend: BuildDeploymentBackendKind,
    #[serde(default = "default_build_deployment_filesystem_root_dir")]
    pub filesystem_root_dir: String,
    #[serde(default)]
    pub public_base_url: Option<String>,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub bearer_token: Option<String>,
    #[serde(default = "default_build_deployment_docker_bin")]
    pub docker_bin: String,
    #[serde(default)]
    pub image_repository: Option<String>,
    #[serde(default)]
    pub rollout_command: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BuildDeploymentBackendKind {
    #[default]
    RecordOnly,
    Filesystem,
    Http,
    Container,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub backend: RateLimitBackendKind,
    #[serde(default = "default_rate_limit_redis_key_prefix")]
    pub redis_key_prefix: String,
    #[serde(default = "default_requests_per_minute")]
    pub requests_per_minute: u32,
    #[serde(default = "default_burst")]
    pub burst: u32,
    #[serde(default = "default_auth_requests_per_minute")]
    pub auth_requests_per_minute: u32,
    #[serde(default = "default_auth_burst")]
    pub auth_burst: u32,
    #[serde(default = "default_oauth_requests_per_minute")]
    pub oauth_requests_per_minute: u32,
    #[serde(default = "default_oauth_burst")]
    pub oauth_burst: u32,
    #[serde(default = "default_trusted_auth_dimensions")]
    pub trusted_auth_dimensions: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeSettings {
    #[serde(default)]
    pub host_mode: RuntimeHostMode,
    #[serde(default)]
    pub guardrails: RuntimeGuardrailSettings,
    #[serde(default)]
    pub request_trust: RequestTrustSettings,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TenantFallbackMode {
    #[default]
    Disabled,
    DefaultTenant,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeHostMode {
    #[default]
    Full,
    RegistryOnly,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestTrustSettings {
    #[serde(default)]
    pub forwarded_headers_mode: ForwardedHeadersMode,
    #[serde(default)]
    pub trusted_proxy_cidrs: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ForwardedHeadersMode {
    #[default]
    Ignore,
    TrustedOnly,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeGuardrailSettings {
    #[serde(default)]
    pub rollout: GuardrailRolloutMode,
    #[serde(default)]
    pub rate_limit_memory_thresholds: RateLimitMemoryGuardrailSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitMemoryGuardrailSettings {
    #[serde(default = "default_runtime_guardrail_api_warning_entries")]
    pub api_warning_entries: usize,
    #[serde(default = "default_runtime_guardrail_api_critical_entries")]
    pub api_critical_entries: usize,
    #[serde(default = "default_runtime_guardrail_auth_warning_entries")]
    pub auth_warning_entries: usize,
    #[serde(default = "default_runtime_guardrail_auth_critical_entries")]
    pub auth_critical_entries: usize,
    #[serde(default = "default_runtime_guardrail_oauth_warning_entries")]
    pub oauth_warning_entries: usize,
    #[serde(default = "default_runtime_guardrail_oauth_critical_entries")]
    pub oauth_critical_entries: usize,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitBackendKind {
    #[default]
    Memory,
    Redis,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GuardrailRolloutMode {
    Observe,
    #[default]
    Enforce,
}

impl Default for TenantSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            resolution: default_resolution(),
            header_name: default_header_name(),
            default_id: default_tenant_id(),
            fallback_mode: TenantFallbackMode::Disabled,
            base_domains: Vec::new(),
        }
    }
}

impl Default for FeatureSettings {
    fn default() -> Self {
        Self {
            registration_enabled: true,
            email_verification: false,
            multi_tenant: true,
            search_indexing: true,
            oauth_enabled: false,
        }
    }
}

impl Default for SearchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            driver: default_search_driver(),
            url: String::new(),
            api_key: None,
            index_prefix: default_index_prefix(),
            reindex: SearchReindexSettings::default(),
        }
    }
}

impl Default for BuildRuntimeSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_ms: default_build_poll_interval_ms(),
            auto_release_environment: None,
            auto_activate_release: false,
            deployment: BuildDeploymentSettings::default(),
        }
    }
}

impl Default for BuildDeploymentSettings {
    fn default() -> Self {
        Self {
            backend: BuildDeploymentBackendKind::RecordOnly,
            filesystem_root_dir: default_build_deployment_filesystem_root_dir(),
            public_base_url: None,
            endpoint_url: None,
            bearer_token: None,
            docker_bin: default_build_deployment_docker_bin(),
            image_repository: None,
            rollout_command: None,
        }
    }
}

impl Default for EventSettings {
    fn default() -> Self {
        Self {
            transport: EventTransportKind::default(),
            relay_target: RelayTargetKind::default(),
            allow_relay_target_fallback: false,
            relay_interval_ms: default_relay_interval_ms(),
            channel_capacity: default_event_channel_capacity(),
            relay_retry_policy: RelayRetryPolicy::default(),
            dlq: DlqSettings::default(),
            backpressure: EventBackpressureSettings::default(),
            iggy: IggyConfig::default(),
        }
    }
}

impl Default for SearchReindexSettings {
    fn default() -> Self {
        Self {
            parallelism: default_search_reindex_parallelism(),
            entity_budget: default_search_reindex_entity_budget(),
            yield_every: default_search_reindex_yield_every(),
        }
    }
}

impl Default for RateLimitSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            backend: RateLimitBackendKind::Memory,
            redis_key_prefix: default_rate_limit_redis_key_prefix(),
            requests_per_minute: default_requests_per_minute(),
            burst: default_burst(),
            auth_requests_per_minute: default_auth_requests_per_minute(),
            auth_burst: default_auth_burst(),
            oauth_requests_per_minute: default_oauth_requests_per_minute(),
            oauth_burst: default_oauth_burst(),
            trusted_auth_dimensions: default_trusted_auth_dimensions(),
        }
    }
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            host_mode: RuntimeHostMode::Full,
            guardrails: RuntimeGuardrailSettings::default(),
            request_trust: RequestTrustSettings::default(),
        }
    }
}

impl Default for RequestTrustSettings {
    fn default() -> Self {
        Self {
            forwarded_headers_mode: ForwardedHeadersMode::Ignore,
            trusted_proxy_cidrs: Vec::new(),
        }
    }
}

impl Default for RuntimeGuardrailSettings {
    fn default() -> Self {
        Self {
            rollout: GuardrailRolloutMode::Enforce,
            rate_limit_memory_thresholds: RateLimitMemoryGuardrailSettings::default(),
        }
    }
}

impl Default for RateLimitMemoryGuardrailSettings {
    fn default() -> Self {
        Self {
            api_warning_entries: default_runtime_guardrail_api_warning_entries(),
            api_critical_entries: default_runtime_guardrail_api_critical_entries(),
            auth_warning_entries: default_runtime_guardrail_auth_warning_entries(),
            auth_critical_entries: default_runtime_guardrail_auth_critical_entries(),
            oauth_warning_entries: default_runtime_guardrail_oauth_warning_entries(),
            oauth_critical_entries: default_runtime_guardrail_oauth_critical_entries(),
        }
    }
}

impl RustokSettings {
    pub fn from_settings(settings: &Option<serde_json::Value>) -> Result<Self, serde_json::Error> {
        let root = settings.clone().unwrap_or_else(|| serde_json::json!({}));
        let rustok = root
            .get("rustok")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let mut parsed: Self = serde_json::from_value(rustok)?;

        if let Ok(raw_transport) = std::env::var("RUSTOK_EVENT_TRANSPORT") {
            parsed.events.transport = parse_event_transport(&raw_transport)?;
        }

        if let Ok(raw_host_mode) = std::env::var("RUSTOK_RUNTIME_HOST_MODE") {
            parsed.runtime.host_mode = parse_runtime_host_mode(&raw_host_mode)?;
        }

        parsed.tenant.header_name = parsed.tenant.header_name.trim().to_string();
        if parsed.tenant.header_name.is_empty() {
            parsed.tenant.header_name = default_header_name();
        }

        parsed.tenant.base_domains = parsed
            .tenant
            .base_domains
            .iter()
            .map(|value| value.trim().trim_end_matches('.').to_ascii_lowercase())
            .filter(|value| !value.is_empty())
            .collect();

        for base_domain in &parsed.tenant.base_domains {
            TenantIdentifierValidator::validate_host(base_domain).map_err(|error| {
                serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid rustok.tenant.base_domains entry `{base_domain}`: {error}"),
                ))
            })?;
        }

        for cidr in &parsed.runtime.request_trust.trusted_proxy_cidrs {
            IpNet::from_str(cidr.trim()).map_err(|error| {
                serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "invalid rustok.runtime.request_trust.trusted_proxy_cidrs entry `{cidr}`: {error}"
                    ),
                ))
            })?;
        }

        if parsed.events.relay_retry_policy.max_attempts <= 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.events.relay_retry_policy.max_attempts must be > 0",
            )));
        }

        if parsed.events.dlq.max_attempts <= 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.events.dlq.max_attempts must be > 0",
            )));
        }

        if parsed.events.channel_capacity == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.events.channel_capacity must be > 0",
            )));
        }

        let backpressure = &parsed.events.backpressure;
        if backpressure.enabled {
            if backpressure.max_queue_depth == 0 {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "rustok.events.backpressure.max_queue_depth must be > 0",
                )));
            }

            if !(0.0..1.0).contains(&backpressure.warning_threshold) {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "rustok.events.backpressure.warning_threshold must be in range (0, 1)",
                )));
            }

            if !(backpressure.warning_threshold..=1.0).contains(&backpressure.critical_threshold)
                || backpressure.critical_threshold <= backpressure.warning_threshold
            {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "rustok.events.backpressure.critical_threshold must be in range (warning_threshold, 1]",
                )));
            }
        }

        if parsed.search.reindex.parallelism == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.search.reindex.parallelism must be > 0",
            )));
        }

        if parsed.search.reindex.entity_budget == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.search.reindex.entity_budget must be > 0",
            )));
        }

        if parsed.search.reindex.yield_every == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.search.reindex.yield_every must be > 0",
            )));
        }

        if parsed.build.poll_interval_ms == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.build.poll_interval_ms must be > 0",
            )));
        }

        if let Some(environment) = parsed
            .build
            .auto_release_environment
            .as_ref()
            .map(|value| value.trim().to_string())
        {
            if environment.is_empty() {
                parsed.build.auto_release_environment = None;
            } else {
                parsed.build.auto_release_environment = Some(environment);
            }
        }

        if parsed.build.auto_activate_release && parsed.build.auto_release_environment.is_none() {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.build.auto_activate_release requires rustok.build.auto_release_environment",
            )));
        }

        if parsed.build.deployment.backend == BuildDeploymentBackendKind::Filesystem
            && parsed
                .build
                .deployment
                .filesystem_root_dir
                .trim()
                .is_empty()
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.build.deployment.filesystem_root_dir must not be empty when backend=filesystem",
            )));
        }

        if parsed.build.deployment.backend == BuildDeploymentBackendKind::Http {
            let endpoint_url = parsed
                .build
                .deployment
                .endpoint_url
                .as_ref()
                .map(|value| value.trim().to_string())
                .unwrap_or_default();

            if endpoint_url.is_empty() {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "rustok.build.deployment.endpoint_url must not be empty when backend=http",
                )));
            }

            parsed.build.deployment.endpoint_url = Some(endpoint_url);
        }

        let docker_bin = parsed.build.deployment.docker_bin.trim();
        if docker_bin.is_empty() {
            parsed.build.deployment.docker_bin = default_build_deployment_docker_bin();
        } else {
            parsed.build.deployment.docker_bin = docker_bin.to_string();
        }

        if parsed.build.deployment.backend == BuildDeploymentBackendKind::Container {
            let image_repository = parsed
                .build
                .deployment
                .image_repository
                .as_ref()
                .map(|value| value.trim().to_string())
                .unwrap_or_default();

            if image_repository.is_empty() {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "rustok.build.deployment.image_repository must not be empty when backend=container",
                )));
            }

            parsed.build.deployment.image_repository = Some(image_repository);
        }

        if let Some(public_base_url) = parsed
            .build
            .deployment
            .public_base_url
            .as_ref()
            .map(|value| value.trim().trim_end_matches('/').to_string())
        {
            if public_base_url.is_empty() {
                parsed.build.deployment.public_base_url = None;
            } else {
                parsed.build.deployment.public_base_url = Some(public_base_url);
            }
        }

        if let Some(bearer_token) = parsed
            .build
            .deployment
            .bearer_token
            .as_ref()
            .map(|value| value.trim().to_string())
        {
            if bearer_token.is_empty() {
                parsed.build.deployment.bearer_token = None;
            } else {
                parsed.build.deployment.bearer_token = Some(bearer_token);
            }
        }

        if let Some(rollout_command) = parsed
            .build
            .deployment
            .rollout_command
            .as_ref()
            .map(|value| value.trim().to_string())
        {
            if rollout_command.is_empty() {
                parsed.build.deployment.rollout_command = None;
            } else {
                parsed.build.deployment.rollout_command = Some(rollout_command);
            }
        }

        if parsed.rate_limit.enabled && parsed.rate_limit.backend == RateLimitBackendKind::Redis {
            if resolve_redis_url().is_none() {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "rustok.rate_limit.backend=redis requires RUSTOK_REDIS_URL or REDIS_URL",
                )));
            }

            if parsed.rate_limit.redis_key_prefix.trim().is_empty() {
                return Err(serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "rustok.rate_limit.redis_key_prefix must not be empty when backend=redis",
                )));
            }
        }

        validate_guardrail_threshold(
            "rustok.runtime.guardrails.rate_limit_memory_thresholds.api",
            parsed
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .api_warning_entries,
            parsed
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .api_critical_entries,
        )?;
        validate_guardrail_threshold(
            "rustok.runtime.guardrails.rate_limit_memory_thresholds.auth",
            parsed
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .auth_warning_entries,
            parsed
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .auth_critical_entries,
        )?;
        validate_guardrail_threshold(
            "rustok.runtime.guardrails.rate_limit_memory_thresholds.oauth",
            parsed
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .oauth_warning_entries,
            parsed
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .oauth_critical_entries,
        )?;

        Ok(parsed)
    }
}

impl RuntimeSettings {
    pub fn is_registry_only(&self) -> bool {
        self.host_mode == RuntimeHostMode::RegistryOnly
    }
}

fn parse_event_transport(value: &str) -> Result<EventTransportKind, serde_json::Error> {
    match value.trim().to_ascii_lowercase().as_str() {
        "memory" => Ok(EventTransportKind::Memory),
        "outbox" => Ok(EventTransportKind::Outbox),
        "iggy" => Ok(EventTransportKind::Iggy),
        _ => Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "Invalid RUSTOK_EVENT_TRANSPORT='{value}'. Expected one of: memory, outbox, iggy"
            ),
        ))),
    }
}

fn parse_runtime_host_mode(value: &str) -> Result<RuntimeHostMode, serde_json::Error> {
    match value.trim().to_ascii_lowercase().as_str() {
        "full" => Ok(RuntimeHostMode::Full),
        "registry_only" => Ok(RuntimeHostMode::RegistryOnly),
        other => Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "invalid RUSTOK_RUNTIME_HOST_MODE `{other}`; expected `full` or `registry_only`"
            ),
        ))),
    }
}

fn default_tenant_id() -> Uuid {
    DEFAULT_TENANT_ID
}

fn default_resolution() -> String {
    "header".to_string()
}

fn default_header_name() -> String {
    "X-Tenant-ID".to_string()
}

fn default_true() -> bool {
    true
}

fn default_search_driver() -> String {
    "meilisearch".to_string()
}

fn default_index_prefix() -> String {
    "rustok_".to_string()
}

fn default_search_reindex_parallelism() -> usize {
    4
}

fn default_search_reindex_entity_budget() -> usize {
    500
}

fn default_search_reindex_yield_every() -> u64 {
    50
}

fn default_build_poll_interval_ms() -> u64 {
    5_000
}

fn default_registry_remote_executor_lease_ttl_ms() -> u64 {
    120_000
}

fn default_registry_remote_executor_requeue_scan_interval_ms() -> u64 {
    15_000
}

fn default_build_deployment_filesystem_root_dir() -> String {
    "artifacts/releases".to_string()
}

fn default_build_deployment_docker_bin() -> String {
    "docker".to_string()
}

fn default_requests_per_minute() -> u32 {
    60
}

fn default_rate_limit_redis_key_prefix() -> String {
    "rate-limit:v1".to_string()
}

fn default_burst() -> u32 {
    10
}

fn default_auth_requests_per_minute() -> u32 {
    20
}

fn default_auth_burst() -> u32 {
    0
}

fn default_oauth_requests_per_minute() -> u32 {
    30
}

fn default_oauth_burst() -> u32 {
    5
}

fn default_trusted_auth_dimensions() -> bool {
    true
}

fn default_relay_interval_ms() -> u64 {
    1_000
}

fn default_event_channel_capacity() -> usize {
    128
}

fn default_relay_max_attempts() -> i32 {
    5
}

fn default_relay_backoff_base_ms() -> u64 {
    1_000
}

fn default_relay_backoff_max_ms() -> u64 {
    60_000
}

fn default_dlq_enabled() -> bool {
    true
}

fn default_dlq_max_attempts() -> i32 {
    10
}

fn default_backpressure_max_queue_depth() -> usize {
    10_000
}

fn default_backpressure_warning_threshold() -> f64 {
    0.7
}

fn default_backpressure_critical_threshold() -> f64 {
    0.9
}

fn default_email_from() -> String {
    "no-reply@rustok.local".to_string()
}

fn default_runtime_guardrail_api_warning_entries() -> usize {
    5_000
}

fn default_runtime_guardrail_api_critical_entries() -> usize {
    20_000
}

fn default_runtime_guardrail_auth_warning_entries() -> usize {
    1_000
}

fn default_runtime_guardrail_auth_critical_entries() -> usize {
    5_000
}

fn default_runtime_guardrail_oauth_warning_entries() -> usize {
    1_000
}

fn default_runtime_guardrail_oauth_critical_entries() -> usize {
    5_000
}

fn default_reset_base_url() -> String {
    "http://localhost:3000/reset-password".to_string()
}

fn validate_guardrail_threshold(
    namespace: &str,
    warning_entries: usize,
    critical_entries: usize,
) -> Result<(), serde_json::Error> {
    if warning_entries == 0 {
        return Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{namespace}.warning_entries must be > 0"),
        )));
    }

    if critical_entries <= warning_entries {
        return Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{namespace}.critical_entries must be > warning_entries"),
        )));
    }

    Ok(())
}

fn default_smtp_host() -> String {
    "localhost".to_string()
}

fn default_smtp_port() -> u16 {
    1025
}

#[cfg(test)]
mod tests {
    use super::{
        BuildDeploymentBackendKind, EventTransportKind, GuardrailRolloutMode, RateLimitBackendKind,
        RelayTargetKind, RustokSettings,
    };
    use std::sync::{Mutex, OnceLock};

    const EVENT_TRANSPORT_ENV: &str = "RUSTOK_EVENT_TRANSPORT";
    const RUNTIME_HOST_MODE_ENV: &str = "RUSTOK_RUNTIME_HOST_MODE";
    const RUSTOK_REDIS_URL_ENV: &str = "RUSTOK_REDIS_URL";
    const REDIS_URL_ENV: &str = "REDIS_URL";

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvVarGuard {
        fn clear(key: &'static str) -> Self {
            let original = std::env::var(key).ok();
            unsafe {
                std::env::remove_var(key);
            }
            Self { key, original }
        }

        fn set(key: &'static str, value: &str) -> Self {
            let original = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.original {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    #[test]
    fn reads_transport_from_config() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "events": {
                    "transport": "outbox"
                }
            }
        });

        let settings = RustokSettings::from_settings(&Some(raw)).expect("settings parsed");
        assert_eq!(settings.events.transport, EventTransportKind::Outbox);
    }

    #[test]
    fn rejects_invalid_env_transport() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::set(EVENT_TRANSPORT_ENV, "broken");
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let err = RustokSettings::from_settings(&Some(serde_json::json!({ "rustok": {} })))
            .expect_err("transport should fail");
        assert!(err
            .to_string()
            .contains("Invalid RUSTOK_EVENT_TRANSPORT='broken'"));
    }

    #[test]
    fn reads_relay_defaults_from_config() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "events": {
                    "transport": "outbox",
                    "relay_target": "iggy"
                }
            }
        });

        let settings = RustokSettings::from_settings(&Some(raw)).expect("settings parsed");
        assert_eq!(settings.events.transport, EventTransportKind::Outbox);
        assert_eq!(settings.events.relay_target, RelayTargetKind::Iggy);
        assert!(!settings.events.allow_relay_target_fallback);
        assert_eq!(settings.events.channel_capacity, 128);
        assert_eq!(settings.events.relay_retry_policy.max_attempts, 5);
        assert_eq!(settings.events.relay_retry_policy.base_backoff_ms, 1_000);
        assert_eq!(settings.events.relay_retry_policy.max_backoff_ms, 60_000);
        assert!(settings.events.dlq.enabled);
        assert_eq!(settings.events.dlq.max_attempts, 10);
    }

    #[test]
    fn rejects_non_positive_retry_and_dlq_attempts() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let bad_retry = serde_json::json!({
            "rustok": {
                "events": {
                    "relay_retry_policy": { "max_attempts": 0 }
                }
            }
        });

        let err =
            RustokSettings::from_settings(&Some(bad_retry)).expect_err("retry validation expected");
        assert!(err
            .to_string()
            .contains("relay_retry_policy.max_attempts must be > 0"));

        let bad_dlq = serde_json::json!({
            "rustok": {
                "events": {
                    "dlq": { "max_attempts": 0 }
                }
            }
        });

        let err =
            RustokSettings::from_settings(&Some(bad_dlq)).expect_err("dlq validation expected");
        assert!(err.to_string().contains("dlq.max_attempts must be > 0"));
    }

    #[test]
    fn rejects_zero_event_channel_capacity() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "events": {
                    "channel_capacity": 0
                }
            }
        });

        let err = RustokSettings::from_settings(&Some(raw)).expect_err("capacity validation");
        assert!(err
            .to_string()
            .contains("rustok.events.channel_capacity must be > 0"));
    }

    #[test]
    fn reads_rate_limit_backend_defaults() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let settings =
            RustokSettings::from_settings(&Some(serde_json::json!({ "rustok": {} }))).unwrap();

        assert_eq!(settings.rate_limit.backend, RateLimitBackendKind::Memory);
        assert_eq!(settings.rate_limit.redis_key_prefix, "rate-limit:v1");
        assert_eq!(settings.rate_limit.oauth_requests_per_minute, 30);
        assert_eq!(settings.rate_limit.oauth_burst, 5);
        assert!(settings.rate_limit.trusted_auth_dimensions);
        assert_eq!(settings.events.channel_capacity, 128);
        assert_eq!(settings.events.relay_interval_ms, 1_000);
        assert_eq!(settings.email.from, "no-reply@rustok.local");
        assert_eq!(
            settings.email.reset_base_url,
            "http://localhost:3000/reset-password"
        );
        assert_eq!(
            settings.runtime.guardrails.rollout,
            GuardrailRolloutMode::Enforce
        );
        assert_eq!(
            settings
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .api_warning_entries,
            5_000
        );
        assert_eq!(
            settings
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .api_critical_entries,
            20_000
        );
        assert_eq!(
            settings
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .auth_warning_entries,
            1_000
        );
        assert_eq!(
            settings
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .auth_critical_entries,
            5_000
        );
        assert_eq!(
            settings
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .oauth_warning_entries,
            1_000
        );
        assert_eq!(
            settings
                .runtime
                .guardrails
                .rate_limit_memory_thresholds
                .oauth_critical_entries,
            5_000
        );
        assert_eq!(settings.search.reindex.parallelism, 4);
        assert_eq!(settings.search.reindex.entity_budget, 500);
        assert_eq!(settings.search.reindex.yield_every, 50);
    }

    #[test]
    fn rejects_zero_search_reindex_budget_values() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "search": {
                    "reindex": {
                        "parallelism": 0
                    }
                }
            }
        });

        let err = RustokSettings::from_settings(&Some(raw)).expect_err("search reindex validation");
        assert!(err
            .to_string()
            .contains("rustok.search.reindex.parallelism must be > 0"));
    }

    #[test]
    fn reads_build_runtime_defaults_and_rejects_invalid_activation_policy() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let settings =
            RustokSettings::from_settings(&Some(serde_json::json!({ "rustok": {} }))).unwrap();
        assert!(!settings.build.enabled);
        assert_eq!(settings.build.poll_interval_ms, 5_000);
        assert!(settings.build.auto_release_environment.is_none());
        assert!(!settings.build.auto_activate_release);
        assert_eq!(
            settings.build.deployment.backend,
            BuildDeploymentBackendKind::RecordOnly
        );
        assert_eq!(
            settings.build.deployment.filesystem_root_dir,
            "artifacts/releases"
        );
        assert!(settings.build.deployment.public_base_url.is_none());
        assert!(settings.build.deployment.endpoint_url.is_none());
        assert!(settings.build.deployment.bearer_token.is_none());
        assert_eq!(settings.build.deployment.docker_bin, "docker");
        assert!(settings.build.deployment.image_repository.is_none());
        assert!(settings.build.deployment.rollout_command.is_none());

        let raw = serde_json::json!({
            "rustok": {
                "build": {
                    "enabled": true,
                    "poll_interval_ms": 1000,
                    "auto_activate_release": true
                }
            }
        });
        let err =
            RustokSettings::from_settings(&Some(raw)).expect_err("build activation validation");
        assert!(err.to_string().contains(
            "rustok.build.auto_activate_release requires rustok.build.auto_release_environment"
        ));

        let raw = serde_json::json!({
            "rustok": {
                "build": {
                    "deployment": {
                        "backend": "filesystem",
                        "filesystem_root_dir": ""
                    }
                }
            }
        });
        let err = RustokSettings::from_settings(&Some(raw))
            .expect_err("filesystem deployment validation");
        assert!(err
            .to_string()
            .contains("rustok.build.deployment.filesystem_root_dir must not be empty"));

        let raw = serde_json::json!({
            "rustok": {
                "build": {
                    "deployment": {
                        "backend": "http"
                    }
                }
            }
        });
        let err =
            RustokSettings::from_settings(&Some(raw)).expect_err("http deployment validation");
        assert!(err
            .to_string()
            .contains("rustok.build.deployment.endpoint_url must not be empty"));

        let raw = serde_json::json!({
            "rustok": {
                "build": {
                    "deployment": {
                        "backend": "http",
                        "endpoint_url": " https://deploy.example.com/releases ",
                        "bearer_token": " secret-token "
                    }
                }
            }
        });
        let settings =
            RustokSettings::from_settings(&Some(raw)).expect("http deployment settings parse");
        assert_eq!(
            settings.build.deployment.backend,
            BuildDeploymentBackendKind::Http
        );
        assert_eq!(
            settings.build.deployment.endpoint_url.as_deref(),
            Some("https://deploy.example.com/releases")
        );
        assert_eq!(
            settings.build.deployment.bearer_token.as_deref(),
            Some("secret-token")
        );

        let raw = serde_json::json!({
            "rustok": {
                "build": {
                    "deployment": {
                        "backend": "container"
                    }
                }
            }
        });
        let err =
            RustokSettings::from_settings(&Some(raw)).expect_err("container deployment validation");
        assert!(err
            .to_string()
            .contains("rustok.build.deployment.image_repository must not be empty"));

        let raw = serde_json::json!({
            "rustok": {
                "build": {
                    "deployment": {
                        "backend": "container",
                        "docker_bin": " docker ",
                        "image_repository": " registry.example.com/rustok/server ",
                        "rollout_command": " ./scripts/deploy.sh {image} "
                    }
                }
            }
        });
        let settings =
            RustokSettings::from_settings(&Some(raw)).expect("container deployment settings parse");
        assert_eq!(
            settings.build.deployment.backend,
            BuildDeploymentBackendKind::Container
        );
        assert_eq!(settings.build.deployment.docker_bin, "docker");
        assert_eq!(
            settings.build.deployment.image_repository.as_deref(),
            Some("registry.example.com/rustok/server")
        );
        assert_eq!(
            settings.build.deployment.rollout_command.as_deref(),
            Some("./scripts/deploy.sh {image}")
        );
    }

    #[test]
    fn rejects_invalid_runtime_guardrail_thresholds() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "runtime": {
                    "guardrails": {
                        "rate_limit_memory_thresholds": {
                            "auth_warning_entries": 100,
                            "auth_critical_entries": 100
                        }
                    }
                }
            }
        });

        let err =
            RustokSettings::from_settings(&Some(raw)).expect_err("guardrail validation expected");
        assert!(err.to_string().contains(
            "rustok.runtime.guardrails.rate_limit_memory_thresholds.auth.critical_entries must be > warning_entries"
        ));
    }

    #[test]
    fn parses_registry_only_runtime_host_mode() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _host_mode_guard = EnvVarGuard::clear(RUNTIME_HOST_MODE_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "runtime": {
                    "host_mode": "registry_only"
                }
            }
        });

        let settings =
            RustokSettings::from_settings(&Some(raw)).expect("registry-only settings parse");
        assert!(settings.runtime.is_registry_only());
    }

    #[test]
    fn env_overrides_runtime_host_mode() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _host_mode_guard = EnvVarGuard::set(RUNTIME_HOST_MODE_ENV, "registry_only");
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let settings = RustokSettings::from_settings(&Some(serde_json::json!({ "rustok": {} })))
            .expect("runtime host mode env override parse");

        assert!(settings.runtime.is_registry_only());
    }

    #[test]
    fn rejects_invalid_runtime_host_mode_env_override() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _host_mode_guard = EnvVarGuard::set(RUNTIME_HOST_MODE_ENV, "broken");
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let err = RustokSettings::from_settings(&Some(serde_json::json!({ "rustok": {} })))
            .expect_err("invalid host mode env override expected");
        assert!(err.to_string().contains(
            "invalid RUSTOK_RUNTIME_HOST_MODE `broken`; expected `full` or `registry_only`"
        ));
    }

    #[test]
    fn rejects_enabled_redis_rate_limit_without_redis_url() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "rate_limit": {
                    "enabled": true,
                    "backend": "redis"
                }
            }
        });

        let err =
            RustokSettings::from_settings(&Some(raw)).expect_err("redis URL validation expected");
        assert!(err
            .to_string()
            .contains("rustok.rate_limit.backend=redis requires RUSTOK_REDIS_URL or REDIS_URL"));
    }

    #[test]
    fn allows_enabled_redis_rate_limit_with_redis_url() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::set(RUSTOK_REDIS_URL_ENV, "redis://localhost:6379/0");
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "rate_limit": {
                    "enabled": true,
                    "backend": "redis",
                    "redis_key_prefix": "rate-limit:v1"
                }
            }
        });

        let settings = RustokSettings::from_settings(&Some(raw)).expect("redis settings parse");
        assert_eq!(settings.rate_limit.backend, RateLimitBackendKind::Redis);
    }
}
