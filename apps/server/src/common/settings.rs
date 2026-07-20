use ipnet::IpNet;
use rustok_core::tenant_validation::TenantIdentifierValidator;
use rustok_iggy::IggyConfig;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

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
    #[serde(default)]
    pub readiness: ReadinessSettings,
    #[serde(default)]
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct RegistrySettings {
    #[serde(default)]
    pub remote_executor: RegistryRemoteExecutorSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadinessSettings {
    #[serde(default = "default_readiness_outbox_max_pending_lag_seconds")]
    pub outbox_max_pending_lag_seconds: u64,
    #[serde(default = "default_readiness_search_max_lag_seconds")]
    pub search_max_lag_seconds: u64,
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
/// - `none`: email sending is disabled
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmailProvider {
    #[default]
    Smtp,
    None,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmailSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub provider: EmailProvider,
    #[serde(default)]
    pub allow_disabled_in_production: bool,
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
            allow_disabled_in_production: false,
            smtp: SmtpSettings::default(),
            from: default_email_from(),
            reset_base_url: default_reset_base_url(),
        }
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

impl Default for ReadinessSettings {
    fn default() -> Self {
        Self {
            outbox_max_pending_lag_seconds: default_readiness_outbox_max_pending_lag_seconds(),
            search_max_lag_seconds: default_readiness_search_max_lag_seconds(),
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
    #[serde(default = "default_relay_batch_size")]
    pub relay_batch_size: u64,
    #[serde(default = "default_relay_max_concurrency")]
    pub relay_max_concurrency: usize,
    #[serde(default = "default_relay_claim_ttl_ms")]
    pub relay_claim_ttl_ms: u64,
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TenantResolutionMode {
    #[default]
    Header,
    Host,
    Domain,
    Subdomain,
}

impl TenantResolutionMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Host => "host",
            Self::Domain => "domain",
            Self::Subdomain => "subdomain",
        }
    }
}

impl std::fmt::Display for TenantResolutionMode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TenantRuntimeProfile {
    #[default]
    MultiTenant,
    SingleTenant,
    Development,
}

impl TenantRuntimeProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MultiTenant => "multi_tenant",
            Self::SingleTenant => "single_tenant",
            Self::Development => "development",
        }
    }

    pub const fn derives_tenant_from_request(self) -> bool {
        !matches!(self, Self::SingleTenant)
    }
}

impl std::fmt::Display for TenantRuntimeProfile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TenantSettings {
    #[serde(default)]
    pub profile: TenantRuntimeProfile,
    /// Compatibility switch validated against `profile`.
    /// `single_tenant` requires false; all request-derived profiles require true.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub resolution: TenantResolutionMode,
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
    /// Enables the trusted static deployment adapter used by installer flows.
    /// It does not start a server-local build worker.
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub deployment: BuildDeploymentSettings,
}

pub use rustok_build::{
    DeploymentBackend as BuildDeploymentBackendKind, DeploymentSettings as BuildDeploymentSettings,
};

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
    pub background_workers: RuntimeBackgroundWorkerSettings,
    #[serde(default)]
    pub guardrails: RuntimeGuardrailSettings,
    #[serde(default)]
    pub request_trust: RequestTrustSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuntimeBackgroundWorkerSettings {
    #[serde(default = "default_true")]
    pub workflow_cron_enabled: bool,
    #[serde(default = "default_true")]
    pub seo_bulk_enabled: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TenantFallbackMode {
    #[default]
    Disabled,
    DefaultTenant,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TenantSettingsError {
    ProfileEnabledMismatch {
        profile: TenantRuntimeProfile,
        enabled: bool,
    },
    InvalidHeaderName(String),
    MissingSubdomainBaseDomain,
    FallbackRequiresDevelopmentProfile,
    FallbackRequiresHeaderMode,
    DevelopmentProfileForbiddenInProduction,
}

impl std::fmt::Display for TenantSettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProfileEnabledMismatch { profile, enabled } => write!(
                formatter,
                "rustok.tenant.profile={profile} is inconsistent with rustok.tenant.enabled={enabled}; single_tenant requires enabled=false and request-derived profiles require enabled=true"
            ),
            Self::InvalidHeaderName(value) => {
                write!(formatter, "rustok.tenant.header_name `{value}` is not a valid HTTP header name")
            }
            Self::MissingSubdomainBaseDomain => formatter.write_str(
                "rustok.tenant.base_domains must contain at least one domain when resolution=subdomain",
            ),
            Self::FallbackRequiresDevelopmentProfile => formatter.write_str(
                "rustok.tenant.fallback_mode=default_tenant requires rustok.tenant.profile=development",
            ),
            Self::FallbackRequiresHeaderMode => formatter.write_str(
                "rustok.tenant.fallback_mode=default_tenant is only valid with resolution=header",
            ),
            Self::DevelopmentProfileForbiddenInProduction => formatter.write_str(
                "rustok.tenant.profile=development is forbidden in production; use multi_tenant or single_tenant",
            ),
        }
    }
}

impl std::error::Error for TenantSettingsError {}

impl TenantSettings {
    pub fn validate(&self) -> Result<(), TenantSettingsError> {
        let expected_enabled = self.profile.derives_tenant_from_request();
        if self.enabled != expected_enabled {
            return Err(TenantSettingsError::ProfileEnabledMismatch {
                profile: self.profile,
                enabled: self.enabled,
            });
        }

        if self.fallback_mode == TenantFallbackMode::DefaultTenant
            && self.profile != TenantRuntimeProfile::Development
        {
            return Err(TenantSettingsError::FallbackRequiresDevelopmentProfile);
        }

        if self.profile == TenantRuntimeProfile::SingleTenant {
            return Ok(());
        }

        axum::http::HeaderName::from_bytes(self.header_name.as_bytes())
            .map_err(|_| TenantSettingsError::InvalidHeaderName(self.header_name.clone()))?;

        if self.resolution == TenantResolutionMode::Subdomain && self.base_domains.is_empty() {
            return Err(TenantSettingsError::MissingSubdomainBaseDomain);
        }

        if self.fallback_mode == TenantFallbackMode::DefaultTenant
            && self.resolution != TenantResolutionMode::Header
        {
            return Err(TenantSettingsError::FallbackRequiresHeaderMode);
        }

        Ok(())
    }

    pub fn validate_for_environment(&self, production: bool) -> Result<(), TenantSettingsError> {
        self.validate()?;
        if production && self.profile == TenantRuntimeProfile::Development {
            return Err(TenantSettingsError::DevelopmentProfileForbiddenInProduction);
        }
        Ok(())
    }
}

pub use rustok_build::BuildRuntimeMode as RuntimeHostMode;

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
            profile: TenantRuntimeProfile::MultiTenant,
            enabled: true,
            resolution: TenantResolutionMode::Header,
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
            deployment: BuildDeploymentSettings::default(),
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
            relay_batch_size: default_relay_batch_size(),
            relay_max_concurrency: default_relay_max_concurrency(),
            relay_claim_ttl_ms: default_relay_claim_ttl_ms(),
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
            background_workers: RuntimeBackgroundWorkerSettings::default(),
            guardrails: RuntimeGuardrailSettings::default(),
            request_trust: RequestTrustSettings::default(),
        }
    }
}

impl Default for RuntimeBackgroundWorkerSettings {
    fn default() -> Self {
        Self {
            workflow_cron_enabled: true,
            seo_bulk_enabled: true,
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

        parsed
            .tenant
            .base_domains
            .sort_by_key(|domain| std::cmp::Reverse(domain.len()));
        parsed.tenant.base_domains.dedup();

        for base_domain in &parsed.tenant.base_domains {
            TenantIdentifierValidator::validate_host(base_domain).map_err(|error| {
                serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid rustok.tenant.base_domains entry `{base_domain}`: {error}"),
                ))
            })?;
        }

        parsed
            .tenant
            .validate_for_environment(is_production_environment())
            .map_err(|error| {
                serde_json::Error::io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    error.to_string(),
                ))
            })?;

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

        if parsed.events.relay_batch_size == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.events.relay_batch_size must be > 0",
            )));
        }

        if parsed.events.relay_max_concurrency == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.events.relay_max_concurrency must be > 0",
            )));
        }

        if parsed.events.relay_claim_ttl_ms == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.events.relay_claim_ttl_ms must be > 0",
            )));
        }

        if parsed.readiness.outbox_max_pending_lag_seconds == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.readiness.outbox_max_pending_lag_seconds must be > 0",
            )));
        }

        if parsed.readiness.search_max_lag_seconds == 0 {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.readiness.search_max_lag_seconds must be > 0",
            )));
        }

        if is_production_environment()
            && email_delivery_is_disabled(&parsed.email)
            && !parsed.email.allow_disabled_in_production
            && !email_disabled_production_override_enabled()
        {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rustok.email is disabled in production; configure an email provider or set rustok.email.allow_disabled_in_production=true / RUSTOK_EMAIL_ALLOW_DISABLED_IN_PRODUCTION=true as an explicit emergency override",
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

    pub fn is_worker_only(&self) -> bool {
        self.host_mode == RuntimeHostMode::Worker
    }

    pub fn is_api_only(&self) -> bool {
        self.host_mode == RuntimeHostMode::Api
    }

    pub fn is_admin_ssr(&self) -> bool {
        self.host_mode == RuntimeHostMode::AdminSsr
    }

    pub fn is_storefront_ssr(&self) -> bool {
        self.host_mode == RuntimeHostMode::StorefrontSsr
    }

    pub fn runs_background_workers(&self) -> bool {
        matches!(
            self.host_mode,
            RuntimeHostMode::Full | RuntimeHostMode::Worker
        )
    }
}

/// Cached, shared reference to the parsed [`RustokSettings`].
///
/// Stored in the server runtime at bootstrap time so that per-request
/// middleware (tenant resolution, channel resolution, etc.) can read
/// configuration without re-parsing `ctx.config.settings` (JSON
/// deserialisation + env-var overrides) on every HTTP request.
#[derive(Clone)]
pub struct SharedRustokSettings(pub std::sync::Arc<RustokSettings>);

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
        "api" => Ok(RuntimeHostMode::Api),
        "admin_ssr" => Ok(RuntimeHostMode::AdminSsr),
        "storefront_ssr" => Ok(RuntimeHostMode::StorefrontSsr),
        "worker" => Ok(RuntimeHostMode::Worker),
        other => Err(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "invalid RUSTOK_RUNTIME_HOST_MODE `{other}`; expected `full`, `registry_only`, `api`, `admin_ssr`, `storefront_ssr`, or `worker`"
            ),
        ))),
    }
}

fn default_tenant_id() -> Uuid {
    DEFAULT_TENANT_ID
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

fn default_registry_remote_executor_lease_ttl_ms() -> u64 {
    120_000
}

fn default_registry_remote_executor_requeue_scan_interval_ms() -> u64 {
    15_000
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

fn default_relay_batch_size() -> u64 {
    100
}

fn default_relay_max_concurrency() -> usize {
    8
}

fn default_relay_claim_ttl_ms() -> u64 {
    60_000
}

fn default_readiness_outbox_max_pending_lag_seconds() -> u64 {
    300
}

fn default_readiness_search_max_lag_seconds() -> u64 {
    300
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

fn email_delivery_is_disabled(settings: &EmailSettings) -> bool {
    matches!(settings.provider, EmailProvider::None)
        || matches!(settings.provider, EmailProvider::Smtp) && !settings.enabled
}

fn is_production_environment() -> bool {
    ["RUSTOK_ENV", "RUST_ENV", "APP_ENV"].iter().any(|key| {
        std::env::var(key)
            .map(|value| {
                matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "prod" | "production"
                )
            })
            .unwrap_or(false)
    })
}

fn email_disabled_production_override_enabled() -> bool {
    std::env::var("RUSTOK_EMAIL_ALLOW_DISABLED_IN_PRODUCTION")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{
        BuildDeploymentBackendKind, EmailProvider, EventTransportKind, GuardrailRolloutMode,
        RateLimitBackendKind, RelayTargetKind, RustokSettings, TenantFallbackMode,
        TenantResolutionMode, TenantRuntimeProfile, TenantSettingsError,
    };
    use std::sync::{Mutex, OnceLock};

    const EVENT_TRANSPORT_ENV: &str = "RUSTOK_EVENT_TRANSPORT";
    const RUNTIME_HOST_MODE_ENV: &str = "RUSTOK_RUNTIME_HOST_MODE";
    const RUSTOK_REDIS_URL_ENV: &str = "RUSTOK_REDIS_URL";
    const REDIS_URL_ENV: &str = "REDIS_URL";
    const RUST_ENV_ENV: &str = "RUST_ENV";
    const APP_ENV_ENV: &str = "APP_ENV";
    const EMAIL_DISABLED_PROD_OVERRIDE_ENV: &str = "RUSTOK_EMAIL_ALLOW_DISABLED_IN_PRODUCTION";

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
        assert!(
            err.to_string()
                .contains("Invalid RUSTOK_EVENT_TRANSPORT='broken'")
        );
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
        assert!(
            err.to_string()
                .contains("relay_retry_policy.max_attempts must be > 0")
        );

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
    fn rejects_disabled_smtp_email_in_production_without_override() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _transport_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);
        let _app_env_guard = EnvVarGuard::clear(APP_ENV_ENV);
        let _override_guard = EnvVarGuard::clear(EMAIL_DISABLED_PROD_OVERRIDE_ENV);
        let _rust_env_guard = EnvVarGuard::set(RUST_ENV_ENV, "production");

        let raw = serde_json::json!({
            "rustok": {
                "email": {
                    "provider": "smtp",
                    "enabled": false
                }
            }
        });

        let err = RustokSettings::from_settings(&Some(raw)).expect_err("email validation");
        assert!(
            err.to_string()
                .contains("rustok.email is disabled in production")
        );
    }

    #[test]
    fn rejects_none_email_provider_in_production_without_override() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _transport_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);
        let _rust_env_guard = EnvVarGuard::clear(RUST_ENV_ENV);
        let _override_guard = EnvVarGuard::clear(EMAIL_DISABLED_PROD_OVERRIDE_ENV);
        let _app_env_guard = EnvVarGuard::set(APP_ENV_ENV, "prod");

        let raw = serde_json::json!({
            "rustok": {
                "email": {
                    "provider": "none"
                }
            }
        });

        let err = RustokSettings::from_settings(&Some(raw)).expect_err("email validation");
        assert!(
            err.to_string()
                .contains("rustok.email is disabled in production")
        );
    }

    #[test]
    fn accepts_enabled_smtp_email_in_production() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _transport_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);
        let _rust_env_guard = EnvVarGuard::clear(RUST_ENV_ENV);
        let _app_env_guard = EnvVarGuard::set(APP_ENV_ENV, "production");
        let _override_guard = EnvVarGuard::clear(EMAIL_DISABLED_PROD_OVERRIDE_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "email": {
                    "enabled": true,
                    "provider": "smtp",
                    "from": "no-reply@example.com",
                    "reset_base_url": "https://example.com/reset-password"
                }
            }
        });

        let settings = RustokSettings::from_settings(&Some(raw)).expect("settings parsed");
        assert_eq!(settings.email.provider, EmailProvider::Smtp);
        assert!(settings.email.enabled);
    }

    #[test]
    fn accepts_disabled_email_in_production_with_config_override() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _transport_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);
        let _app_env_guard = EnvVarGuard::clear(APP_ENV_ENV);
        let _override_guard = EnvVarGuard::clear(EMAIL_DISABLED_PROD_OVERRIDE_ENV);
        let _rust_env_guard = EnvVarGuard::set(RUST_ENV_ENV, "production");

        let raw = serde_json::json!({
            "rustok": {
                "email": {
                    "provider": "none",
                    "allow_disabled_in_production": true
                }
            }
        });

        let settings = RustokSettings::from_settings(&Some(raw)).expect("settings parsed");
        assert_eq!(settings.email.provider, EmailProvider::None);
        assert!(settings.email.allow_disabled_in_production);
    }

    #[test]
    fn accepts_disabled_email_in_production_with_env_override() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _transport_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);
        let _app_env_guard = EnvVarGuard::clear(APP_ENV_ENV);
        let _rust_env_guard = EnvVarGuard::set(RUST_ENV_ENV, "production");
        let _override_guard = EnvVarGuard::set(EMAIL_DISABLED_PROD_OVERRIDE_ENV, "true");

        let raw = serde_json::json!({
            "rustok": {
                "email": {
                    "provider": "none"
                }
            }
        });

        let settings = RustokSettings::from_settings(&Some(raw)).expect("settings parsed");
        assert_eq!(settings.email.provider, EmailProvider::None);
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
        assert!(
            err.to_string()
                .contains("rustok.events.channel_capacity must be > 0")
        );
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
        assert_eq!(settings.readiness.outbox_max_pending_lag_seconds, 300);
        assert_eq!(settings.readiness.search_max_lag_seconds, 300);
    }

    #[test]
    fn rejects_zero_readiness_lag_thresholds() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "readiness": {
                    "outbox_max_pending_lag_seconds": 0
                }
            }
        });

        let err =
            RustokSettings::from_settings(&Some(raw)).expect_err("readiness validation expected");
        assert!(
            err.to_string()
                .contains("rustok.readiness.outbox_max_pending_lag_seconds must be > 0")
        );

        let raw = serde_json::json!({
            "rustok": {
                "readiness": {
                    "search_max_lag_seconds": 0
                }
            }
        });

        let err =
            RustokSettings::from_settings(&Some(raw)).expect_err("readiness validation expected");
        assert!(
            err.to_string()
                .contains("rustok.readiness.search_max_lag_seconds must be > 0")
        );
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
        assert!(
            err.to_string()
                .contains("rustok.search.reindex.parallelism must be > 0")
        );
    }

    #[test]
    fn reads_build_deployment_defaults_and_validates_deployment_configuration() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let settings =
            RustokSettings::from_settings(&Some(serde_json::json!({ "rustok": {} }))).unwrap();
        assert!(!settings.build.enabled);
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
                    "deployment": {
                        "backend": "filesystem",
                        "filesystem_root_dir": ""
                    }
                }
            }
        });
        let err = RustokSettings::from_settings(&Some(raw))
            .expect_err("filesystem deployment validation");
        assert!(
            err.to_string()
                .contains("rustok.build.deployment.filesystem_root_dir must not be empty")
        );

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
        assert!(
            err.to_string()
                .contains("rustok.build.deployment.endpoint_url must not be empty")
        );

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
        assert!(
            err.to_string()
                .contains("rustok.build.deployment.image_repository must not be empty")
        );

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
    fn parses_worker_runtime_host_mode() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _host_mode_guard = EnvVarGuard::clear(RUNTIME_HOST_MODE_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        let raw = serde_json::json!({
            "rustok": {
                "runtime": {
                    "host_mode": "worker"
                }
            }
        });

        let settings = RustokSettings::from_settings(&Some(raw)).expect("worker settings parse");
        assert!(settings.runtime.is_worker_only());
        assert!(!settings.runtime.is_registry_only());
    }

    #[test]
    fn surface_runtime_modes_skip_background_workers() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        let _env_guard = EnvVarGuard::clear(EVENT_TRANSPORT_ENV);
        let _host_mode_guard = EnvVarGuard::clear(RUNTIME_HOST_MODE_ENV);
        let _redis_guard = EnvVarGuard::clear(RUSTOK_REDIS_URL_ENV);
        let _redis_url_guard = EnvVarGuard::clear(REDIS_URL_ENV);

        for host_mode in ["api", "admin_ssr", "storefront_ssr"] {
            let settings = RustokSettings::from_settings(&Some(serde_json::json!({
                "rustok": { "runtime": { "host_mode": host_mode } }
            })))
            .expect("surface host mode parse");

            assert!(!settings.runtime.runs_background_workers());
        }
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
            "invalid RUSTOK_RUNTIME_HOST_MODE `broken`; expected `full`, `registry_only`, `api`, `admin_ssr`, `storefront_ssr`, or `worker`"
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
        assert!(
            err.to_string()
                .contains("rustok.rate_limit.backend=redis requires RUSTOK_REDIS_URL or REDIS_URL")
        );
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
    #[test]
    fn tenant_resolution_mode_is_deserialized_strictly() {
        let raw = serde_json::json!({
            "rustok": { "tenant": { "resolution": "automatic" } }
        });
        let error = RustokSettings::from_settings(&Some(raw)).expect_err("unknown mode");
        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn tenant_policy_requires_profile_and_enabled_to_agree() {
        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::SingleTenant;
        assert_eq!(
            settings.tenant.validate(),
            Err(TenantSettingsError::ProfileEnabledMismatch {
                profile: TenantRuntimeProfile::SingleTenant,
                enabled: true,
            })
        );
        settings.tenant.enabled = false;
        assert!(settings.tenant.validate().is_ok());
    }

    #[test]
    fn tenant_policy_rejects_fallback_outside_development_profile() {
        let mut settings = RustokSettings::default();
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
        assert_eq!(
            settings.tenant.validate(),
            Err(TenantSettingsError::FallbackRequiresDevelopmentProfile)
        );
    }

    #[test]
    fn tenant_policy_rejects_invalid_development_fallback_combination() {
        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::Development;
        settings.tenant.resolution = TenantResolutionMode::Host;
        settings.tenant.fallback_mode = TenantFallbackMode::DefaultTenant;
        assert_eq!(
            settings.tenant.validate(),
            Err(TenantSettingsError::FallbackRequiresHeaderMode)
        );
    }

    #[test]
    fn tenant_policy_rejects_development_profile_in_production() {
        let mut settings = RustokSettings::default();
        settings.tenant.profile = TenantRuntimeProfile::Development;
        assert_eq!(
            settings.tenant.validate_for_environment(true),
            Err(TenantSettingsError::DevelopmentProfileForbiddenInProduction)
        );
    }

    #[test]
    fn tenant_policy_requires_subdomain_base_domains() {
        let mut settings = RustokSettings::default();
        settings.tenant.resolution = TenantResolutionMode::Subdomain;
        settings.tenant.base_domains.clear();
        assert_eq!(
            settings.tenant.validate(),
            Err(TenantSettingsError::MissingSubdomainBaseDomain)
        );
    }
}
