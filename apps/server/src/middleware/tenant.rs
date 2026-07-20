use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use rustok_cache::{
    CacheEnvelope, CacheKeyBuilder as CanonicalCacheKeyBuilder, CacheLoadPolicy, CacheLoadSource,
    CacheService, CacheTtlPolicy, NegativeCachePolicy,
};
use rustok_core::{CacheBackend, Error as CoreError};
use rustok_tenant::{
    PortActor, PortContext, PortError, PortErrorKind, TenantReadPort, TenantReadProjection,
    TenantReadRequest, TenantReadSelector, TenantService,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fmt, sync::Arc};

use super::{
    tenant_resolution::{
        resolve_explicit_slug, resolve_request, ResolvedTenantIdentifier, TenantIdentifierKind,
        TenantResolution, TenantResolutionSource,
    },
    tenant_route_policy::{tenant_route_scope, TenantRouteScope},
};
use crate::context::{TenantContext, TenantContextExtension};
use crate::services::server_runtime_context::ServerRuntimeContext;

const TENANT_CACHE_VERSION: &str = "v2";
const TENANT_CONTEXT_SCHEMA_VERSION: u32 = 1;
const TENANT_NEGATIVE_SCHEMA_VERSION: u32 = 1;
const TENANT_CACHE_TTL: Duration = Duration::from_secs(300);
const TENANT_NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(60);
const TENANT_CACHE_MAX_WEIGHT_BYTES: u64 = 16 * 1024 * 1024;
const TENANT_NEGATIVE_CACHE_MAX_WEIGHT_BYTES: u64 = 1024 * 1024;
const TENANT_NEGATIVE_MAX_ENCODED_BYTES: usize = 64 * 1024;
const TENANT_CACHE_LOADER_TIMEOUT: Duration = Duration::from_secs(10);
const TENANT_CACHE_JITTER_PERCENT: u8 = 10;
#[cfg(feature = "redis-cache")]
const TENANT_CACHE_REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
enum CachedTenantMiss {
    NotFound,
    Disabled,
}

#[derive(Debug, Clone, Copy)]
enum TenantResolutionTransport {
    Http,
    GraphqlWebSocket,
}

impl TenantResolutionTransport {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::GraphqlWebSocket => "graphql_ws",
        }
    }
}

#[derive(Debug)]
pub(crate) enum TenantContextLoadError {
    InvalidIdentifier(String),
    InvalidAssertion(String),
    InfrastructureUnavailable,
    NotFound,
    Disabled,
    CacheUnavailable(String),
    ClockUnavailable(String),
    BackendUnavailable(String),
}

impl TenantContextLoadError {
    pub(crate) const fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidIdentifier(_) | Self::InvalidAssertion(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Disabled => StatusCode::FORBIDDEN,
            Self::InfrastructureUnavailable
            | Self::CacheUnavailable(_)
            | Self::ClockUnavailable(_)
            | Self::BackendUnavailable(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub(crate) const fn client_message(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier(_) => "Invalid tenant identifier",
            Self::InvalidAssertion(_) => "Conflicting tenant assertions",
            Self::NotFound => "Tenant not found",
            Self::Disabled => "Tenant is disabled",
            Self::InfrastructureUnavailable
            | Self::CacheUnavailable(_)
            | Self::ClockUnavailable(_)
            | Self::BackendUnavailable(_) => "Failed to resolve tenant",
        }
    }

    const fn metric_outcome(&self) -> &'static str {
        match self {
            Self::InvalidIdentifier(_) => "invalid_identifier",
            Self::InvalidAssertion(_) => "invalid_assertion",
            Self::InfrastructureUnavailable => "infrastructure_unavailable",
            Self::NotFound => "not_found",
            Self::Disabled => "disabled",
            Self::CacheUnavailable(_) => "cache_unavailable",
            Self::ClockUnavailable(_) => "clock_unavailable",
            Self::BackendUnavailable(_) => "backend_unavailable",
        }
    }
}

impl fmt::Display for TenantContextLoadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier(reason) => {
                write!(formatter, "invalid tenant identifier: {reason}")
            }
            Self::InvalidAssertion(reason) => {
                write!(formatter, "invalid tenant assertion: {reason}")
            }
            Self::InfrastructureUnavailable => {
                formatter.write_str("tenant cache infrastructure is unavailable")
            }
            Self::NotFound => formatter.write_str("tenant not found"),
            Self::Disabled => formatter.write_str("tenant is disabled"),
            Self::CacheUnavailable(reason) => {
                write!(formatter, "tenant cache unavailable: {reason}")
            }
            Self::ClockUnavailable(reason) => {
                write!(formatter, "tenant clock unavailable: {reason}")
            }
            Self::BackendUnavailable(reason) => {
                write!(formatter, "tenant backend unavailable: {reason}")
            }
        }
    }
}

impl std::error::Error for TenantContextLoadError {}

impl From<CachedTenantMiss> for TenantContextLoadError {
    fn from(value: CachedTenantMiss) -> Self {
        match value {
            CachedTenantMiss::NotFound => Self::NotFound,
            CachedTenantMiss::Disabled => Self::Disabled,
        }
    }
}

fn tenant_context_from_projection(
    projection: TenantReadProjection,
) -> Result<TenantContext, CachedTenantMiss> {
    if !projection.is_active {
        tracing::warn!(
            tenant_id = %projection.id,
            slug = %projection.slug,
            "Rejecting request for disabled tenant"
        );
        return Err(CachedTenantMiss::Disabled);
    }

    Ok(TenantContext {
        id: projection.id,
        name: projection.name,
        slug: projection.slug,
        domain: projection.domain,
        settings: projection.settings,
        default_locale: projection.default_locale,
        is_active: projection.is_active,
    })
}

fn tenant_read_request(identifier: &ResolvedTenantIdentifier) -> TenantReadRequest {
    let selector = match identifier {
        ResolvedTenantIdentifier::Uuid(value) => TenantReadSelector::Id(*value),
        ResolvedTenantIdentifier::Slug(value) => TenantReadSelector::Slug(value.clone()),
        ResolvedTenantIdentifier::Host(value) => TenantReadSelector::Domain(value.clone()),
    };

    TenantReadRequest {
        selector,
        include_inactive: true,
    }
}

fn tenant_read_context(identifier: &ResolvedTenantIdentifier) -> PortContext {
    let identity = identifier.value();
    PortContext::new(
        identity.clone(),
        PortActor::service("rustok-server.tenant-resolver"),
        "und",
        format!(
            "tenant-resolver:{}:{}",
            identifier.kind().as_str(),
            identity
        ),
    )
    .with_deadline(TENANT_CACHE_LOADER_TIMEOUT)
}

fn tenant_port_error_to_core_error(error: PortError) -> CoreError {
    match error.kind {
        PortErrorKind::NotFound => CoreError::NotFound(error.message),
        PortErrorKind::Validation => CoreError::Validation(error.message),
        PortErrorKind::Timeout | PortErrorKind::Unavailable | PortErrorKind::InvariantViolation => {
            CoreError::Database(sea_orm::DbErr::Custom(format!(
                "tenant read port failed: {}",
                error.message
            )))
        }
        PortErrorKind::Conflict | PortErrorKind::Forbidden => CoreError::Forbidden(error.message),
    }
}

#[derive(Clone)]
pub struct TenantCacheInfrastructure {
    tenant_cache: Arc<dyn CacheBackend>,
    tenant_negative_cache: Arc<dyn CacheBackend>,
    metrics: Arc<TenantCacheMetricsStore>,
    key_builder: TenantCacheKeyBuilder,
    load_policy: CacheLoadPolicy,
    negative_policy: NegativeCachePolicy,
    cache_service: CacheService,
}

#[derive(Debug, Clone)]
struct TenantCacheKeyBuilder {
    version: &'static str,
}

impl TenantCacheKeyBuilder {
    fn new(version: &'static str) -> Self {
        Self { version }
    }

    fn tenant_key(&self, kind: TenantIdentifierKind, value: &str) -> String {
        self.build("resolution", kind, value)
    }

    fn negative_key(&self, kind: TenantIdentifierKind, value: &str) -> String {
        self.build("negative", kind, value)
    }

    fn kind_key(&self, kind: TenantIdentifierKind, value: &str) -> String {
        self.tenant_key(kind, normalize_identifier_value(kind, value).as_str())
    }

    fn kind_negative_key(&self, kind: TenantIdentifierKind, value: &str) -> String {
        self.negative_key(kind, normalize_identifier_value(kind, value).as_str())
    }

    fn build(&self, resource: &str, kind: TenantIdentifierKind, value: &str) -> String {
        CanonicalCacheKeyBuilder::new(
            "rustok-server",
            "runtime",
            "global",
            "tenant",
            self.version,
            resource,
        )
        .expect("tenant cache fixed key components are valid")
        .named_identity("kind", kind.as_str())
        .expect("tenant identifier kind is non-empty")
        .named_identity("value", value)
        .expect("validated tenant identifier is non-empty")
        .build()
    }
}

fn normalize_identifier_value(kind: TenantIdentifierKind, value: &str) -> String {
    match kind {
        TenantIdentifierKind::Host => value.to_lowercase(),
        _ => value.to_string(),
    }
}

#[derive(Clone)]
struct TenantCacheMetricsStore {
    local_hits: Arc<AtomicU64>,
    local_misses: Arc<AtomicU64>,
    local_negative_hits: Arc<AtomicU64>,
    local_negative_misses: Arc<AtomicU64>,
    local_negative_inserts: Arc<AtomicU64>,
    coalesced_requests: Arc<AtomicU64>,
    #[cfg(feature = "redis-cache")]
    redis_client: Option<redis::Client>,
}

impl TenantCacheMetricsStore {
    fn new(cache_service: &CacheService) -> Self {
        Self {
            local_hits: Arc::new(AtomicU64::new(0)),
            local_misses: Arc::new(AtomicU64::new(0)),
            local_negative_hits: Arc::new(AtomicU64::new(0)),
            local_negative_misses: Arc::new(AtomicU64::new(0)),
            local_negative_inserts: Arc::new(AtomicU64::new(0)),
            coalesced_requests: Arc::new(AtomicU64::new(0)),
            #[cfg(feature = "redis-cache")]
            redis_client: cache_service.redis_client().cloned(),
        }
    }

    async fn incr(&self, key: &str, local: &AtomicU64) {
        local.fetch_add(1, Ordering::Relaxed);

        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            let result = tenant_cache_redis_timeout(
                "metrics connection",
                client.get_multiplexed_async_connection(),
            )
            .await;
            if let Ok(mut conn) = result {
                let redis_key = format!("tenant_metrics:{}:{key}", TENANT_CACHE_VERSION);
                let _ = tenant_cache_redis_timeout(
                    "metrics INCR",
                    redis::cmd("INCR")
                        .arg(redis_key)
                        .query_async::<u64>(&mut conn),
                )
                .await;
            }
        }
    }

    async fn snapshot(
        &self,
        base: rustok_core::CacheStats,
        negative: rustok_core::CacheStats,
    ) -> TenantCacheStats {
        TenantCacheStats {
            hits: self.read_metric("hits", &self.local_hits).await,
            misses: self.read_metric("misses", &self.local_misses).await,
            evictions: base.evictions,
            negative_hits: self
                .read_metric("negative_hits", &self.local_negative_hits)
                .await,
            negative_misses: self
                .read_metric("negative_misses", &self.local_negative_misses)
                .await,
            negative_evictions: negative.evictions,
            entries: base.entries,
            negative_entries: negative.entries,
            negative_inserts: self
                .read_metric("negative_inserts", &self.local_negative_inserts)
                .await,
            coalesced_requests: self
                .read_metric("coalesced_requests", &self.coalesced_requests)
                .await,
            invalidation_listener_status: 0,
        }
    }

    async fn read_metric(&self, key: &str, local: &AtomicU64) -> u64 {
        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            if let Ok(mut conn) = tenant_cache_redis_timeout(
                "metrics connection",
                client.get_multiplexed_async_connection(),
            )
            .await
            {
                let redis_key = format!("tenant_metrics:{}:{key}", TENANT_CACHE_VERSION);
                if let Ok(Some(metric)) = tenant_cache_redis_timeout(
                    "metrics GET",
                    redis::cmd("GET")
                        .arg(redis_key)
                        .query_async::<Option<u64>>(&mut conn),
                )
                .await
                {
                    return metric;
                }
            }
        }

        local.load(Ordering::Relaxed)
    }
}

#[cfg(feature = "redis-cache")]
async fn tenant_cache_redis_timeout<T, E, F>(operation: &str, future: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio::time::timeout(TENANT_CACHE_REDIS_OPERATION_TIMEOUT, future)
        .await
        .map_err(|_| {
            format!(
                "tenant cache {operation} timed out after {} ms",
                TENANT_CACHE_REDIS_OPERATION_TIMEOUT.as_millis()
            )
        })?
        .map_err(|error| format!("tenant cache {operation} failed: {error}"))
}

impl TenantCacheInfrastructure {
    async fn new(cache_service: &CacheService) -> Self {
        let ttl = CacheTtlPolicy::deterministic_jitter(
            TENANT_CACHE_TTL,
            TENANT_CACHE_JITTER_PERCENT,
            "tenant-resolution-v2",
        )
        .expect("tenant cache jitter policy is valid");
        let load_policy = CacheLoadPolicy::new(ttl)
            .with_loader_timeout(TENANT_CACHE_LOADER_TIMEOUT)
            .expect("tenant loader timeout is positive");
        let negative_policy = NegativeCachePolicy::deterministic_jittered(
            TENANT_NEGATIVE_SCHEMA_VERSION,
            TENANT_NEGATIVE_CACHE_TTL,
            TENANT_CACHE_JITTER_PERCENT,
            "tenant-negative-v2",
        )
        .expect("tenant negative cache policy is valid")
        .with_max_encoded_bytes(TENANT_NEGATIVE_MAX_ENCODED_BYTES)
        .expect("tenant negative cache size limit is positive");

        Self {
            tenant_cache: cache_service
                .backend_weighted(
                    &format!("tenant-cache:{}:data", TENANT_CACHE_VERSION),
                    TENANT_CACHE_TTL,
                    TENANT_CACHE_MAX_WEIGHT_BYTES,
                )
                .await,
            tenant_negative_cache: cache_service
                .backend_weighted(
                    &format!("tenant-cache:{}:negative", TENANT_CACHE_VERSION),
                    TENANT_NEGATIVE_CACHE_TTL,
                    TENANT_NEGATIVE_CACHE_MAX_WEIGHT_BYTES,
                )
                .await,
            metrics: Arc::new(TenantCacheMetricsStore::new(cache_service)),
            key_builder: TenantCacheKeyBuilder::new(TENANT_CACHE_VERSION),
            load_policy,
            negative_policy,
            cache_service: cache_service.clone(),
        }
    }

    async fn check_negative(
        &self,
        cache_key: &str,
    ) -> Result<Option<CachedTenantMiss>, TenantContextLoadError> {
        let cached = self
            .cache_service
            .get_negative::<CachedTenantMiss>(
                Arc::clone(&self.tenant_negative_cache),
                cache_key,
                &self.negative_policy,
            )
            .await
            .map_err(|error| TenantContextLoadError::CacheUnavailable(error.to_string()))?;

        if let Some(hit) = cached {
            self.metrics
                .incr("negative_hits", &self.metrics.local_negative_hits)
                .await;
            return Ok(Some(hit.reason));
        }

        self.metrics
            .incr("negative_misses", &self.metrics.local_negative_misses)
            .await;
        Ok(None)
    }

    async fn set_negative(
        &self,
        cache_key: String,
        reason: CachedTenantMiss,
    ) -> Result<(), TenantContextLoadError> {
        self.cache_service
            .store_negative(
                Arc::clone(&self.tenant_negative_cache),
                cache_key,
                reason,
                current_unix_ms()
                    .map_err(|error| TenantContextLoadError::ClockUnavailable(error.to_string()))?,
                None,
                &self.negative_policy,
            )
            .await
            .map_err(|error| TenantContextLoadError::CacheUnavailable(error.to_string()))?;
        self.metrics
            .incr("negative_inserts", &self.metrics.local_negative_inserts)
            .await;
        Ok(())
    }

    async fn get_or_load_with_coalescing<F, Fut>(
        &self,
        cache_key: &str,
        loader: F,
    ) -> Result<TenantContext, TenantContextLoadError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = rustok_core::Result<TenantContext>>,
    {
        let cache_ttl = self
            .load_policy
            .ttl
            .ttl_for(cache_key)
            .unwrap_or(TENANT_CACHE_TTL);
        let result = self
            .cache_service
            .load_enveloped_or_fill(
                Arc::clone(&self.tenant_cache),
                cache_key,
                TENANT_CONTEXT_SCHEMA_VERSION,
                self.load_policy.clone(),
                || async move {
                    let context = loader().await?;
                    let generated_at = current_unix_ms().map_err(|error| {
                        CoreError::Cache(format!("tenant cache timestamp creation failed: {error}"))
                    })?;
                    CacheEnvelope::new(TENANT_CONTEXT_SCHEMA_VERSION, generated_at, context)
                        .and_then(|envelope| {
                            envelope.with_expirations(
                                None,
                                Some(generated_at.saturating_add(duration_millis_ceil(cache_ttl))),
                            )
                        })
                        .map_err(cache_envelope_error_to_core)
                },
            )
            .await
            .map_err(core_error_to_load_error)?;

        match result.source {
            CacheLoadSource::Hit => {
                self.metrics.incr("hits", &self.metrics.local_hits).await;
            }
            CacheLoadSource::Filled | CacheLoadSource::Coalesced => {
                self.metrics
                    .incr("misses", &self.metrics.local_misses)
                    .await;
            }
        }
        if result.source == CacheLoadSource::Coalesced {
            self.metrics
                .incr("coalesced_requests", &self.metrics.coalesced_requests)
                .await;
        }

        Ok(result.value)
    }
}

pub async fn init_tenant_cache_infrastructure(
    ctx: &ServerRuntimeContext,
    cache_service: &CacheService,
) {
    if ctx.shared_contains::<Arc<TenantCacheInfrastructure>>() {
        return;
    }

    ctx.shared_insert(Arc::new(
        TenantCacheInfrastructure::new(cache_service).await,
    ));
}

fn tenant_infra(ctx: &ServerRuntimeContext) -> Option<Arc<TenantCacheInfrastructure>> {
    ctx.shared_get::<Arc<TenantCacheInfrastructure>>()
}

pub(crate) async fn load_tenant_context(
    ctx: &ServerRuntimeContext,
    identifier: &ResolvedTenantIdentifier,
) -> Result<TenantContext, TenantContextLoadError> {
    let Some(infra) = tenant_infra(ctx) else {
        return Err(TenantContextLoadError::InfrastructureUnavailable);
    };

    let identifier_value = identifier.value();
    let cache_key = infra
        .key_builder
        .kind_key(identifier.kind(), &identifier_value);
    let negative_key = infra
        .key_builder
        .kind_negative_key(identifier.kind(), &identifier_value);

    if let Some(reason) = infra.check_negative(&negative_key).await? {
        return Err(reason.into());
    }

    let tenant_service = TenantService::new(ctx.db_clone());
    let tenant_request = tenant_read_request(identifier);
    let tenant_port_context = tenant_read_context(identifier);
    let negative_key_clone = negative_key.clone();
    let infra_clone = infra.clone();

    infra
        .get_or_load_with_coalescing(&cache_key, || async move {
            let projection = match tenant_service
                .read_tenant(tenant_port_context, tenant_request)
                .await
            {
                Ok(projection) => projection,
                Err(error) if error.kind == PortErrorKind::NotFound => {
                    if let Err(cache_error) = infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                    {
                        tracing::warn!(%cache_error, "Tenant not-found negative cache write failed");
                    }
                    return Err(CoreError::NotFound(error.message));
                }
                Err(error) => return Err(tenant_port_error_to_core_error(error)),
            };

            match tenant_context_from_projection(projection) {
                Ok(context) => Ok(context),
                Err(CachedTenantMiss::Disabled) => {
                    if let Err(cache_error) = infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::Disabled)
                        .await
                    {
                        tracing::warn!(%cache_error, "Disabled-tenant negative cache write failed");
                    }
                    Err(CoreError::Forbidden("tenant disabled".to_string()))
                }
                Err(CachedTenantMiss::NotFound) => {
                    if let Err(cache_error) = infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                    {
                        tracing::warn!(%cache_error, "Tenant projection negative cache write failed");
                    }
                    Err(CoreError::NotFound("tenant not found".to_string()))
                }
            }
        })
        .await
}

fn record_resolution_outcome(
    transport: TenantResolutionTransport,
    source: TenantResolutionSource,
    outcome: &str,
) {
    rustok_telemetry::metrics::record_tenant_resolution(
        transport.as_str(),
        source.as_str(),
        outcome,
    );
}

async fn load_resolved_tenant_context(
    ctx: &ServerRuntimeContext,
    resolution: &TenantResolution,
    transport: TenantResolutionTransport,
) -> Result<TenantContext, TenantContextLoadError> {
    let result: Result<TenantContext, TenantContextLoadError> = async {
        let context = load_tenant_context(ctx, &resolution.identifier).await?;
        resolution
            .validate_resolved_slug(&context.slug)
            .map_err(|error| TenantContextLoadError::InvalidAssertion(error.to_string()))?;
        Ok(context)
    }
    .await;

    let outcome = match &result {
        Ok(_) => "success",
        Err(error) => error.metric_outcome(),
    };
    record_resolution_outcome(transport, resolution.source, outcome);
    result
}

pub(crate) async fn resolve_tenant_context_by_slug(
    ctx: &ServerRuntimeContext,
    slug: &str,
) -> Result<TenantContext, TenantContextLoadError> {
    let resolution = resolve_explicit_slug(slug)
        .map_err(|error| TenantContextLoadError::InvalidIdentifier(error.to_string()))?;
    load_resolved_tenant_context(
        ctx,
        &resolution,
        TenantResolutionTransport::GraphqlWebSocket,
    )
    .await
}

pub async fn resolve(
    State(ctx): State<ServerRuntimeContext>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    match tenant_route_scope(req.uri().path()) {
        TenantRouteScope::TenantBound => {}
        TenantRouteScope::GlobalOperator | TenantRouteScope::SelfResolvingHandshake => {
            return Ok(next.run(req).await);
        }
    }

    let settings = ctx.settings();
    let resolution = resolve_request(&req, settings).map_err(|error| {
        tracing::warn!(
            path = req.uri().path(),
            error = %error,
            "Tenant resolution failed"
        );
        error.status_code()
    })?;

    if resolution.source == TenantResolutionSource::DevelopmentFallback {
        tracing::warn!(
            path = req.uri().path(),
            header_name = %settings.tenant.header_name,
            tenant_id = %settings.tenant.default_id,
            "Using explicitly configured development default-tenant fallback"
        );
    }

    let context = load_resolved_tenant_context(&ctx, &resolution, TenantResolutionTransport::Http)
        .await
        .map_err(|error| {
            tracing::warn!(
                path = req.uri().path(),
                error = %error,
                "Tenant context loading failed"
            );
            error.status_code()
        })?;

    req.extensions_mut().insert(TenantContextExtension(context));
    Ok(next.run(req).await)
}

#[derive(Debug, Clone, Copy)]
pub struct TenantCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub negative_hits: u64,
    pub negative_misses: u64,
    pub negative_evictions: u64,
    pub entries: u64,
    pub negative_entries: u64,
    pub negative_inserts: u64,
    pub coalesced_requests: u64,
    pub invalidation_listener_status: i64,
}

pub async fn tenant_cache_stats(ctx: &ServerRuntimeContext) -> TenantCacheStats {
    let Some(infra) = tenant_infra(ctx) else {
        return TenantCacheStats {
            hits: 0,
            misses: 0,
            evictions: 0,
            negative_hits: 0,
            negative_misses: 0,
            negative_evictions: 0,
            entries: 0,
            negative_entries: 0,
            negative_inserts: 0,
            coalesced_requests: 0,
            invalidation_listener_status: 0,
        };
    };

    let stats = infra.tenant_cache.stats();
    let negative_stats = infra.tenant_negative_cache.stats();
    infra.metrics.snapshot(stats, negative_stats).await
}

fn core_error_to_load_error(error: CoreError) -> TenantContextLoadError {
    match error {
        CoreError::NotFound(_) => TenantContextLoadError::NotFound,
        CoreError::Forbidden(_) => TenantContextLoadError::Disabled,
        CoreError::Validation(reason) => TenantContextLoadError::InvalidIdentifier(reason),
        CoreError::Cache(reason) => TenantContextLoadError::CacheUnavailable(reason),
        other => TenantContextLoadError::BackendUnavailable(other.to_string()),
    }
}

fn cache_envelope_error_to_core(error: rustok_cache::CacheEnvelopeError) -> CoreError {
    CoreError::Cache(format!("tenant cache envelope error: {error}"))
}

fn current_unix_ms() -> Result<u64, std::time::SystemTimeError> {
    unix_ms_at(SystemTime::now())
}

pub(crate) fn unix_ms_at(time: SystemTime) -> Result<u64, std::time::SystemTimeError> {
    Ok(time
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .min(u128::from(u64::MAX)) as u64)
}

fn duration_millis_ceil(duration: Duration) -> u64 {
    if duration.is_zero() {
        return 0;
    }
    duration
        .as_nanos()
        .saturating_add(999_999)
        .checked_div(1_000_000)
        .unwrap_or(u128::MAX)
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
#[path = "tenant_tests.rs"]
mod invalidation_tests;
