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
use rustok_core::tenant_validation::TenantIdentifierValidator;
use rustok_core::{CacheBackend, Error as CoreError};
use rustok_tenant::{
    PortActor, PortContext, PortError, PortErrorKind, TenantReadPort, TenantReadProjection,
    TenantReadRequest, TenantReadSelector, TenantService,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::common::{
    extract_effective_host, peer_ip_from_extensions,
    settings::{RustokSettings, TenantFallbackMode},
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

#[derive(Debug, Clone, Copy)]
pub enum TenantIdentifierKind {
    Uuid,
    Slug,
    Host,
}

#[derive(Debug, Clone)]
pub struct ResolvedTenantIdentifier {
    pub value: String,
    pub kind: TenantIdentifierKind,
    pub uuid: Uuid,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
enum CachedTenantMiss {
    NotFound,
    Disabled,
}

impl CachedTenantMiss {
    fn status_code(self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Disabled => StatusCode::FORBIDDEN,
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
    let selector = match identifier.kind {
        TenantIdentifierKind::Uuid => TenantReadSelector::Id(identifier.uuid),
        TenantIdentifierKind::Slug => TenantReadSelector::Slug(identifier.value.clone()),
        TenantIdentifierKind::Host => TenantReadSelector::Domain(identifier.value.clone()),
    };

    TenantReadRequest {
        selector,
        include_inactive: true,
    }
}

fn tenant_read_context(identifier: &ResolvedTenantIdentifier) -> PortContext {
    PortContext::new(
        identifier.uuid.to_string(),
        PortActor::service("rustok-server.tenant-resolver"),
        "und",
        format!(
            "tenant-resolver:{}:{}",
            identifier.kind.as_str(),
            identifier.value
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

impl TenantIdentifierKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TenantIdentifierKind::Uuid => "uuid",
            TenantIdentifierKind::Slug => "slug",
            TenantIdentifierKind::Host => "host",
        }
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
    ) -> Result<Option<CachedTenantMiss>, StatusCode> {
        let cached = self
            .cache_service
            .get_negative::<CachedTenantMiss>(
                Arc::clone(&self.tenant_negative_cache),
                cache_key,
                &self.negative_policy,
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
    ) -> Result<(), StatusCode> {
        self.cache_service
            .store_negative(
                Arc::clone(&self.tenant_negative_cache),
                cache_key,
                reason,
                current_unix_ms(),
                None,
                &self.negative_policy,
            )
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        self.metrics
            .incr("negative_inserts", &self.metrics.local_negative_inserts)
            .await;
        Ok(())
    }

    async fn get_or_load_with_coalescing<F, Fut>(
        &self,
        cache_key: &str,
        loader: F,
    ) -> Result<TenantContext, StatusCode>
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
                    let generated_at = current_unix_ms();
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
            .map_err(cache_load_error_to_status)?;

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

pub async fn resolve(
    State(ctx): State<ServerRuntimeContext>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if should_bypass_tenant_resolution(req.uri().path()) {
        return Ok(next.run(req).await);
    }

    let settings = ctx.settings();
    let identifier = resolve_identifier(&req, settings)?;

    let Some(infra) = tenant_infra(&ctx) else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let cache_key = infra
        .key_builder
        .kind_key(identifier.kind, &identifier.value);
    let negative_key = infra
        .key_builder
        .kind_negative_key(identifier.kind, &identifier.value);

    if let Some(reason) = infra.check_negative(&negative_key).await? {
        return Err(reason.status_code());
    }

    let tenant_service = TenantService::new(ctx.db_clone());
    let tenant_request = tenant_read_request(&identifier);
    let tenant_port_context = tenant_read_context(&identifier);
    let negative_key_clone = negative_key.clone();
    let infra_clone = infra.clone();

    let context = infra
        .get_or_load_with_coalescing(&cache_key, || async move {
            let projection = match tenant_service
                .read_tenant(tenant_port_context, tenant_request)
                .await
            {
                Ok(projection) => projection,
                Err(error) if error.kind == PortErrorKind::NotFound => {
                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                        .map_err(|_| {
                            CoreError::Cache("tenant negative cache write failed".to_string())
                        })?;
                    return Err(CoreError::NotFound(error.message));
                }
                Err(error) => return Err(tenant_port_error_to_core_error(error)),
            };

            match tenant_context_from_projection(projection) {
                Ok(context) => Ok(context),
                Err(CachedTenantMiss::Disabled) => {
                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::Disabled)
                        .await
                        .map_err(|_| {
                            CoreError::Cache("tenant negative cache write failed".to_string())
                        })?;
                    Err(CoreError::Forbidden("tenant disabled".to_string()))
                }
                Err(CachedTenantMiss::NotFound) => {
                    infra_clone
                        .set_negative(negative_key_clone.clone(), CachedTenantMiss::NotFound)
                        .await
                        .map_err(|_| {
                            CoreError::Cache("tenant negative cache write failed".to_string())
                        })?;
                    Err(CoreError::NotFound("tenant not found".to_string()))
                }
            }
        })
        .await?;

    req.extensions_mut().insert(TenantContextExtension(context));
    Ok(next.run(req).await)
}

fn should_bypass_tenant_resolution(path: &str) -> bool {
    matches!(path, "/metrics" | "/api/openapi.json" | "/api/openapi.yaml")
        || path == "/api/graphql/schema.graphql"
        || path == "/api/graphql/ws"
        || path == "/api/install"
        || path.starts_with("/api/install/")
        || path == "/v1/catalog"
        || path.starts_with("/v1/catalog/")
        || path == "/catalog"
        || path.starts_with("/catalog/")
        || path.starts_with("/health")
}

pub fn resolve_identifier(
    req: &Request<Body>,
    settings: &RustokSettings,
) -> Result<ResolvedTenantIdentifier, StatusCode> {
    if !settings.tenant.enabled {
        return Ok(ResolvedTenantIdentifier {
            value: settings.tenant.default_id.to_string(),
            kind: TenantIdentifierKind::Uuid,
            uuid: settings.tenant.default_id,
        });
    }

    match settings.tenant.resolution.as_str() {
        "header" => {
            let primary_header_value = req
                .headers()
                .get(&settings.tenant.header_name)
                .and_then(|value| value.to_str().ok());
            let slug_header_value = (settings.tenant.header_name != "X-Tenant-Slug")
                .then(|| req.headers().get("X-Tenant-Slug"))
                .flatten()
                .and_then(|value| value.to_str().ok());

            let identifier = primary_header_value
                .or(slug_header_value)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());

            let identifier = match identifier {
                Some(identifier) => identifier,
                None if matches!(
                    settings.tenant.fallback_mode,
                    TenantFallbackMode::DefaultTenant
                ) =>
                {
                    settings.tenant.default_id.to_string()
                }
                None => {
                    tracing::warn!(
                        header_name = %settings.tenant.header_name,
                        "Missing tenant header in strict header resolution mode"
                    );
                    return Err(StatusCode::BAD_REQUEST);
                }
            };

            classify_and_validate_identifier(&identifier).map_err(|error| {
                tracing::warn!(
                    identifier = %identifier,
                    error = %error,
                    "Invalid tenant identifier from header"
                );
                StatusCode::BAD_REQUEST
            })
        }
        "host" | "domain" => {
            let peer_ip = peer_ip_from_extensions(req.extensions());
            let host =
                extract_effective_host(req.headers(), peer_ip, &settings.runtime.request_trust)
                    .ok_or(StatusCode::BAD_REQUEST)?;
            let host_without_port = host.split(':').next().unwrap_or(host.as_str());

            let validated_host = TenantIdentifierValidator::validate_host(host_without_port)
                .map_err(|error| {
                    tracing::warn!(
                        host = %host_without_port,
                        error = %error,
                        "Invalid tenant hostname"
                    );
                    StatusCode::BAD_REQUEST
                })?;

            Ok(ResolvedTenantIdentifier {
                value: validated_host,
                kind: TenantIdentifierKind::Host,
                uuid: settings.tenant.default_id,
            })
        }
        "subdomain" => {
            let peer_ip = peer_ip_from_extensions(req.extensions());
            let host =
                extract_effective_host(req.headers(), peer_ip, &settings.runtime.request_trust)
                    .ok_or(StatusCode::BAD_REQUEST)?;
            let host_without_port = host.split(':').next().unwrap_or(host.as_str());
            let validated_host = TenantIdentifierValidator::validate_host(host_without_port)
                .map_err(|error| {
                    tracing::warn!(
                        host = %host_without_port,
                        error = %error,
                        "Invalid tenant hostname"
                    );
                    StatusCode::BAD_REQUEST
                })?;

            let identifier = subdomain_identifier(&validated_host, &settings.tenant.base_domains)?;
            classify_and_validate_identifier(&identifier).map_err(|error| {
                tracing::warn!(
                    host = %validated_host,
                    identifier = %identifier,
                    %error,
                    "Invalid tenant subdomain identifier"
                );
                StatusCode::BAD_REQUEST
            })
        }
        _ => Ok(ResolvedTenantIdentifier {
            value: settings.tenant.default_id.to_string(),
            kind: TenantIdentifierKind::Uuid,
            uuid: settings.tenant.default_id,
        }),
    }
}

fn subdomain_identifier(host: &str, base_domains: &[String]) -> Result<String, StatusCode> {
    for base_domain in base_domains {
        if host == base_domain {
            tracing::warn!(
                host,
                base_domain,
                "Subdomain routing requires a tenant slug"
            );
            return Err(StatusCode::BAD_REQUEST);
        }

        let suffix = format!(".{base_domain}");
        if let Some(candidate) = host.strip_suffix(&suffix) {
            if candidate.is_empty() || candidate.contains('.') {
                tracing::warn!(
                    host,
                    base_domain,
                    "Invalid nested subdomain for tenant routing"
                );
                return Err(StatusCode::BAD_REQUEST);
            }

            return Ok(candidate.to_string());
        }
    }

    tracing::warn!(
        host,
        "No configured base domain matched subdomain tenant resolution"
    );
    Err(StatusCode::NOT_FOUND)
}

fn classify_and_validate_identifier(
    value: &str,
) -> Result<ResolvedTenantIdentifier, rustok_core::tenant_validation::TenantValidationError> {
    if let Ok(uuid) = TenantIdentifierValidator::validate_uuid(value) {
        return Ok(ResolvedTenantIdentifier {
            value: uuid.to_string(),
            kind: TenantIdentifierKind::Uuid,
            uuid,
        });
    }

    let validated_slug = TenantIdentifierValidator::validate_slug(value)?;

    Ok(ResolvedTenantIdentifier {
        value: validated_slug,
        kind: TenantIdentifierKind::Slug,
        uuid: Uuid::nil(),
    })
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

fn cache_load_error_to_status(error: CoreError) -> StatusCode {
    match error {
        CoreError::NotFound(_) => StatusCode::NOT_FOUND,
        CoreError::Forbidden(_) => StatusCode::FORBIDDEN,
        CoreError::Validation(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn cache_envelope_error_to_core(error: rustok_cache::CacheEnvelopeError) -> CoreError {
    CoreError::Cache(format!("tenant cache envelope error: {error}"))
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
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
