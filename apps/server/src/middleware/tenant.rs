use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use rustok_cache::{
    CacheInvalidationMessage, CacheKeyBuilder as CanonicalCacheKeyBuilder, CacheLoadPolicy,
    CacheLoadSource, CacheService, CacheTtlPolicy,
};
use rustok_core::tenant_validation::TenantIdentifierValidator;
#[cfg(feature = "redis-cache")]
use rustok_core::EventConsumerRuntime;
use rustok_core::{CacheBackend, Error as CoreError};
use rustok_tenant::{
    PortActor, PortContext, PortError, PortErrorKind, TenantReadPort, TenantReadProjection,
    TenantReadRequest, TenantReadSelector, TenantService,
};
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::common::{
    extract_effective_host, peer_ip_from_extensions,
    settings::{RustokSettings, TenantFallbackMode},
};
use crate::context::{TenantContext, TenantContextExtension};
use crate::services::server_runtime_context::ServerRuntimeContext;

const TENANT_CACHE_VERSION: &str = "v2";
const TENANT_INVALIDATION_CHANNEL: &str = "tenant.cache.invalidate";
const TENANT_CACHE_TTL: Duration = Duration::from_secs(300);
const TENANT_NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(60);
const TENANT_CACHE_MAX_WEIGHT_BYTES: u64 = 16 * 1024 * 1024;
const TENANT_NEGATIVE_CACHE_MAX_WEIGHT_BYTES: u64 = 1024 * 1024;
const TENANT_CACHE_LOADER_TIMEOUT: Duration = Duration::from_secs(10);
const TENANT_CACHE_JITTER_PERCENT: u8 = 10;
#[cfg(feature = "redis-cache")]
const TENANT_CACHE_REDIS_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(feature = "redis-cache")]
const TENANT_INVALIDATION_RETRY_DELAY: Duration = Duration::from_secs(5);

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
    invalidation_publisher: Arc<TenantInvalidationPublisher>,
    invalidation_listener_state: Arc<TenantInvalidationListenerState>,
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
struct TenantInvalidationPublisher {
    cache_service: CacheService,
}

impl TenantInvalidationPublisher {
    fn new(cache_service: &CacheService) -> Self {
        Self {
            cache_service: cache_service.clone(),
        }
    }

    async fn publish(&self, cache_key: &str) {
        let outcome = self
            .cache_service
            .publish_invalidation(CacheInvalidationMessage::new(
                TENANT_INVALIDATION_CHANNEL,
                cache_key,
            ))
            .await;
        if self.cache_service.has_redis() && !outcome.redis_published {
            tracing::warn!(
                channel = TENANT_INVALIDATION_CHANNEL,
                "Tenant cache invalidation was not published to Redis"
            );
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantInvalidationListenerStatus {
    Disabled,
    Starting,
    Healthy,
    Degraded,
}

impl TenantInvalidationListenerStatus {
    fn as_u8(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Starting => 1,
            Self::Healthy => 2,
            Self::Degraded => 3,
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Starting,
            2 => Self::Healthy,
            3 => Self::Degraded,
            _ => Self::Disabled,
        }
    }

    pub fn metric_value(self) -> i64 {
        i64::from(self.as_u8())
    }
}

#[derive(Debug, Clone)]
pub struct TenantInvalidationListenerSnapshot {
    pub status: TenantInvalidationListenerStatus,
    pub last_error: Option<String>,
}

#[derive(Debug)]
struct TenantInvalidationListenerState {
    status: AtomicU8,
    last_error: RwLock<Option<String>>,
}

impl TenantInvalidationListenerState {
    fn new() -> Self {
        Self {
            status: AtomicU8::new(TenantInvalidationListenerStatus::Disabled.as_u8()),
            last_error: RwLock::new(None),
        }
    }

    async fn mark_disabled(&self, reason: impl Into<String>) {
        self.status.store(
            TenantInvalidationListenerStatus::Disabled.as_u8(),
            Ordering::Relaxed,
        );
        *self.last_error.write().await = Some(reason.into());
    }

    #[cfg(feature = "redis-cache")]
    async fn mark_starting(&self) {
        self.status.store(
            TenantInvalidationListenerStatus::Starting.as_u8(),
            Ordering::Relaxed,
        );
        *self.last_error.write().await = None;
    }

    #[cfg(feature = "redis-cache")]
    async fn mark_healthy(&self) {
        self.status.store(
            TenantInvalidationListenerStatus::Healthy.as_u8(),
            Ordering::Relaxed,
        );
        *self.last_error.write().await = None;
    }

    #[cfg(feature = "redis-cache")]
    async fn mark_degraded(&self, reason: impl Into<String>) {
        self.status.store(
            TenantInvalidationListenerStatus::Degraded.as_u8(),
            Ordering::Relaxed,
        );
        *self.last_error.write().await = Some(reason.into());
    }

    async fn snapshot(&self) -> TenantInvalidationListenerSnapshot {
        TenantInvalidationListenerSnapshot {
            status: TenantInvalidationListenerStatus::from_u8(self.status.load(Ordering::Relaxed)),
            last_error: self.last_error.read().await.clone(),
        }
    }
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
            invalidation_listener_status: TenantInvalidationListenerStatus::Disabled.metric_value(),
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
            invalidation_publisher: Arc::new(TenantInvalidationPublisher::new(cache_service)),
            invalidation_listener_state: Arc::new(TenantInvalidationListenerState::new()),
            cache_service: cache_service.clone(),
        }
    }

    async fn get_cached_tenant(
        &self,
        cache_key: &str,
    ) -> Result<Option<TenantContext>, StatusCode> {
        let cached = self
            .tenant_cache
            .get(cache_key)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let Some(bytes) = cached else {
            self.metrics
                .incr("misses", &self.metrics.local_misses)
                .await;
            return Ok(None);
        };

        self.metrics.incr("hits", &self.metrics.local_hits).await;
        match serde_json::from_slice::<TenantContext>(&bytes) {
            Ok(context) => Ok(Some(context)),
            Err(error) => {
                tracing::warn!(%error, "Tenant cache deserialization error");
                self.tenant_cache
                    .invalidate(cache_key)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                Ok(None)
            }
        }
    }

    async fn check_negative(
        &self,
        cache_key: &str,
    ) -> Result<Option<CachedTenantMiss>, StatusCode> {
        let cached = self
            .tenant_negative_cache
            .get(cache_key)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if let Some(bytes) = cached {
            self.metrics
                .incr("negative_hits", &self.metrics.local_negative_hits)
                .await;
            match serde_json::from_slice::<CachedTenantMiss>(&bytes) {
                Ok(miss) => return Ok(Some(miss)),
                Err(error) => {
                    tracing::warn!(%error, "Tenant negative cache deserialization error");
                    self.tenant_negative_cache
                        .invalidate(cache_key)
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                    return Ok(None);
                }
            }
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
        let payload = serde_json::to_vec(&reason).map_err(|error| {
            tracing::error!(%error, "Tenant negative cache serialization error");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        self.tenant_negative_cache
            .set(cache_key, payload)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        self.metrics
            .incr("negative_inserts", &self.metrics.local_negative_inserts)
            .await;
        Ok(())
    }

    async fn invalidate_pair(&self, cache_key: &str, negative_key: &str) {
        if let Err(error) = self.tenant_cache.invalidate(cache_key).await {
            tracing::warn!(%error, cache_key, "Tenant data cache invalidation failed");
        }
        if let Err(error) = self.tenant_negative_cache.invalidate(negative_key).await {
            tracing::warn!(%error, negative_key, "Tenant negative cache invalidation failed");
        }
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
        let ttl = CacheTtlPolicy::deterministic_jitter(
            TENANT_CACHE_TTL,
            TENANT_CACHE_JITTER_PERCENT,
            "tenant-resolution-v2",
        )
        .expect("tenant cache jitter policy is valid");
        let policy = CacheLoadPolicy::new(ttl)
            .with_loader_timeout(TENANT_CACHE_LOADER_TIMEOUT)
            .expect("tenant loader timeout is positive");
        let result = self
            .cache_service
            .load_or_fill_with_policy(
                Arc::clone(&self.tenant_cache),
                cache_key,
                policy,
                || async move {
                    let context = loader().await?;
                    serde_json::to_vec(&context).map_err(CoreError::Serialization)
                },
            )
            .await
            .map_err(cache_load_error_to_status)?;

        if result.source == CacheLoadSource::Coalesced {
            self.metrics
                .incr("coalesced_requests", &self.metrics.coalesced_requests)
                .await;
        }

        serde_json::from_slice::<TenantContext>(&result.value).map_err(|error| {
            tracing::warn!(%error, "Tenant cache load_or_fill deserialization error");
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }
}

pub async fn init_tenant_cache_infrastructure(
    ctx: &ServerRuntimeContext,
    cache_service: &CacheService,
) {
    if ctx.shared_contains::<Arc<TenantCacheInfrastructure>>() {
        return;
    }

    let infra = Arc::new(TenantCacheInfrastructure::new(cache_service).await);
    ctx.shared_insert(infra.clone());

    if let Some(task) = spawn_invalidation_listener(infra.clone(), cache_service).await {
        ctx.shared_insert(task);
    } else {
        infra
            .invalidation_listener_state
            .mark_disabled("redis pubsub invalidation listener is disabled")
            .await;
    }
}

async fn spawn_invalidation_listener(
    infra: Arc<TenantCacheInfrastructure>,
    cache_service: &CacheService,
) -> Option<JoinHandle<()>> {
    #[cfg(feature = "redis-cache")]
    {
        if !cache_service.has_redis() {
            return None;
        }
        let invalidations = cache_service.invalidations();
        let listener_state = infra.invalidation_listener_state.clone();
        let task = tokio::spawn(async move {
            let runtime = EventConsumerRuntime::new("tenant_invalidation_listener");
            let mut reason = "startup";

            loop {
                runtime.restarted(reason);
                listener_state.mark_starting().await;

                if let Err(error) = consume_tenant_invalidation_messages(
                    invalidations.clone(),
                    infra.clone(),
                    listener_state.clone(),
                )
                .await
                {
                    listener_state.mark_degraded(error.clone()).await;
                    runtime.closed();
                    tracing::warn!(
                        consumer = runtime.consumer(),
                        channel = TENANT_INVALIDATION_CHANNEL,
                        retry_delay_secs = TENANT_INVALIDATION_RETRY_DELAY.as_secs(),
                        error = %error,
                        "Tenant invalidation listener stopped unexpectedly; scheduling resubscribe"
                    );
                } else {
                    runtime.closed();
                    tracing::warn!(
                        consumer = runtime.consumer(),
                        channel = TENANT_INVALIDATION_CHANNEL,
                        retry_delay_secs = TENANT_INVALIDATION_RETRY_DELAY.as_secs(),
                        "Tenant invalidation listener stopped without error; scheduling resubscribe"
                    );
                }

                reason = "retry";
                tokio::time::sleep(TENANT_INVALIDATION_RETRY_DELAY).await;
            }
        });

        return Some(task);
    }

    #[allow(unreachable_code)]
    None
}

#[cfg(feature = "redis-cache")]
async fn consume_tenant_invalidation_messages(
    invalidations: rustok_cache::CacheInvalidationService,
    infra: Arc<TenantCacheInfrastructure>,
    listener_state: Arc<TenantInvalidationListenerState>,
) -> Result<(), String> {
    invalidations
        .consume_subscription_with_ready(
            TENANT_INVALIDATION_CHANNEL,
            {
                let listener_state = listener_state.clone();
                move || async move {
                    listener_state.mark_healthy().await;
                }
            },
            move |message| {
                let infra = infra.clone();
                async move {
                    let Some((cache_key, negative_key)) = parse_invalidation_payload(&message.key)
                    else {
                        tracing::warn!(
                            channel = %message.channel,
                            payload = %message.key,
                            "Ignoring malformed tenant invalidation payload"
                        );
                        return;
                    };

                    infra.invalidate_pair(cache_key, negative_key).await;
                }
            },
        )
        .await
}

#[cfg(feature = "redis-cache")]
fn parse_invalidation_payload(payload: &str) -> Option<(&str, &str)> {
    let mut parts = payload.split('|');
    let cache_key = parts.next()?;
    let negative_key = parts.next()?;
    if cache_key.is_empty() || negative_key.is_empty() || parts.next().is_some() {
        return None;
    }
    Some((cache_key, negative_key))
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

    if let Some(cached_context) = infra.get_cached_tenant(&cache_key).await? {
        req.extensions_mut()
            .insert(TenantContextExtension(cached_context));
        return Ok(next.run(req).await);
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
                ) => settings.tenant.default_id.to_string(),
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

fn classify_identifier(value: String) -> ResolvedTenantIdentifier {
    match classify_and_validate_identifier(&value) {
        Ok(resolved) => resolved,
        Err(_) => ResolvedTenantIdentifier {
            value: value.clone(),
            kind: TenantIdentifierKind::Uuid,
            uuid: value.parse::<Uuid>().unwrap_or(Uuid::nil()),
        },
    }
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
            invalidation_listener_status: TenantInvalidationListenerStatus::Disabled.metric_value(),
        };
    };

    let stats = infra.tenant_cache.stats();
    let negative_stats = infra.tenant_negative_cache.stats();
    let listener_snapshot = infra.invalidation_listener_state.snapshot().await;
    let mut snapshot = infra.metrics.snapshot(stats, negative_stats).await;
    snapshot.invalidation_listener_status = listener_snapshot.status.metric_value();
    snapshot
}

pub async fn tenant_invalidation_listener_snapshot(
    ctx: &ServerRuntimeContext,
) -> TenantInvalidationListenerSnapshot {
    let Some(infra) = tenant_infra(ctx) else {
        return TenantInvalidationListenerSnapshot {
            status: TenantInvalidationListenerStatus::Disabled,
            last_error: Some("tenant cache infrastructure not initialized".to_string()),
        };
    };

    infra.invalidation_listener_state.snapshot().await
}

pub async fn invalidate_tenant_cache(ctx: &ServerRuntimeContext, identifier: &str) {
    let resolved = classify_identifier(identifier.to_string());
    invalidate_cache_keys(ctx, resolved.kind, &resolved.value).await;
}

pub async fn invalidate_tenant_cache_by_host(ctx: &ServerRuntimeContext, host: &str) {
    invalidate_cache_keys(ctx, TenantIdentifierKind::Host, host).await;
}

pub async fn invalidate_tenant_cache_by_uuid(ctx: &ServerRuntimeContext, tenant_id: Uuid) {
    invalidate_cache_keys(ctx, TenantIdentifierKind::Uuid, &tenant_id.to_string()).await;
}

pub async fn invalidate_tenant_cache_by_slug(ctx: &ServerRuntimeContext, slug: &str) {
    invalidate_cache_keys(ctx, TenantIdentifierKind::Slug, slug).await;
}

async fn invalidate_cache_keys(
    ctx: &ServerRuntimeContext,
    kind: TenantIdentifierKind,
    value: &str,
) {
    let Some(infra) = tenant_infra(ctx) else {
        return;
    };

    let cache_key = infra.key_builder.kind_key(kind, value);
    let negative_key = infra.key_builder.kind_negative_key(kind, value);
    infra.invalidate_pair(&cache_key, &negative_key).await;

    let payload = format!("{cache_key}|{negative_key}");
    infra.invalidation_publisher.publish(&payload).await;
}

fn cache_load_error_to_status(error: CoreError) -> StatusCode {
    match error {
        CoreError::NotFound(_) => StatusCode::NOT_FOUND,
        CoreError::Forbidden(_) => StatusCode::FORBIDDEN,
        CoreError::Validation(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
#[path = "tenant_tests.rs"]
mod invalidation_tests;
