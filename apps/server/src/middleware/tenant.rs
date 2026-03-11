use axum::{
    body::Body,
    extract::State,
    http::{
        header::{FORWARDED, HOST},
        Request, StatusCode,
    },
    middleware::Next,
    response::Response,
};
use loco_rs::app::AppContext;
#[cfg(feature = "redis-cache")]
use redis::AsyncCommands;
use rustok_core::tenant_validation::TenantIdentifierValidator;
#[cfg(feature = "redis-cache")]
use rustok_core::RedisCacheBackend;
use rustok_core::{CacheBackend, EventConsumerRuntime, InMemoryCacheBackend};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify, RwLock};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::common::settings::RustokSettings;
use crate::context::{TenantContext, TenantContextExtension};
use crate::models::tenants;

const TENANT_CACHE_VERSION: &str = "v1";
const TENANT_INVALIDATION_CHANNEL: &str = "tenant.cache.invalidate";
const TENANT_CACHE_TTL: Duration = Duration::from_secs(300);
const TENANT_NEGATIVE_CACHE_TTL: Duration = Duration::from_secs(60);
const TENANT_CACHE_MAX_CAPACITY: u64 = 1_000;
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

#[derive(Clone)]
pub struct TenantCacheInfrastructure {
    tenant_cache: Arc<dyn CacheBackend>,
    tenant_negative_cache: Arc<dyn CacheBackend>,
    metrics: Arc<TenantCacheMetricsStore>,
    key_builder: TenantCacheKeyBuilder,
    invalidation_publisher: Arc<TenantInvalidationPublisher>,
    invalidation_listener_state: Arc<TenantInvalidationListenerState>,
    in_flight: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
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
        format!("tenant:{}:{}:{}", self.version, kind.as_str(), value)
    }

    fn negative_key(&self, kind: TenantIdentifierKind, value: &str) -> String {
        format!(
            "tenant_negative:{}:{}:{}",
            self.version,
            kind.as_str(),
            value
        )
    }

    fn kind_key(&self, kind: TenantIdentifierKind, value: &str) -> String {
        match kind {
            TenantIdentifierKind::Host => self.tenant_key(kind, &value.to_lowercase()),
            _ => self.tenant_key(kind, value),
        }
    }

    fn kind_negative_key(&self, kind: TenantIdentifierKind, value: &str) -> String {
        match kind {
            TenantIdentifierKind::Host => self.negative_key(kind, &value.to_lowercase()),
            _ => self.negative_key(kind, value),
        }
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
    #[cfg(feature = "redis-cache")]
    redis_client: Option<redis::Client>,
}

impl TenantInvalidationPublisher {
    fn new() -> Self {
        Self {
            #[cfg(feature = "redis-cache")]
            redis_client: resolve_redis_client(),
        }
    }

    async fn publish(&self, cache_key: &str) {
        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
                let _: redis::RedisResult<()> = redis::cmd("PUBLISH")
                    .arg(TENANT_INVALIDATION_CHANNEL)
                    .arg(cache_key)
                    .query_async(&mut conn)
                    .await;
            }
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

    async fn mark_starting(&self) {
        self.status.store(
            TenantInvalidationListenerStatus::Starting.as_u8(),
            Ordering::Relaxed,
        );
        *self.last_error.write().await = None;
    }

    async fn mark_healthy(&self) {
        self.status.store(
            TenantInvalidationListenerStatus::Healthy.as_u8(),
            Ordering::Relaxed,
        );
        *self.last_error.write().await = None;
    }

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
    fn new() -> Self {
        Self {
            local_hits: Arc::new(AtomicU64::new(0)),
            local_misses: Arc::new(AtomicU64::new(0)),
            local_negative_hits: Arc::new(AtomicU64::new(0)),
            local_negative_misses: Arc::new(AtomicU64::new(0)),
            local_negative_inserts: Arc::new(AtomicU64::new(0)),
            coalesced_requests: Arc::new(AtomicU64::new(0)),
            #[cfg(feature = "redis-cache")]
            redis_client: resolve_redis_client(),
        }
    }

    async fn incr(&self, key: &str, local: &AtomicU64) {
        local.fetch_add(1, Ordering::Relaxed);

        #[cfg(feature = "redis-cache")]
        if let Some(client) = &self.redis_client {
            if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
                let redis_key = format!("tenant_metrics:{}:{key}", TENANT_CACHE_VERSION);
                let _: redis::RedisResult<u64> = conn.incr(redis_key, 1).await;
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
            if let Ok(mut conn) = client.get_multiplexed_async_connection().await {
                let redis_key = format!("tenant_metrics:{}:{key}", TENANT_CACHE_VERSION);
                let value: redis::RedisResult<Option<u64>> = conn.get(redis_key).await;
                if let Ok(Some(metric)) = value {
                    return metric;
                }
            }
        }

        local.load(Ordering::Relaxed)
    }
}

impl TenantCacheInfrastructure {
    async fn new() -> Self {
        Self {
            tenant_cache: build_tenant_cache_backend().await,
            tenant_negative_cache: build_negative_tenant_cache_backend().await,
            metrics: Arc::new(TenantCacheMetricsStore::new()),
            key_builder: TenantCacheKeyBuilder::new(TENANT_CACHE_VERSION),
            invalidation_publisher: Arc::new(TenantInvalidationPublisher::new()),
            invalidation_listener_state: Arc::new(TenantInvalidationListenerState::new()),
            in_flight: Arc::new(Mutex::new(HashMap::new())),
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
            Err(e) => {
                tracing::warn!("Tenant cache deserialization error: {}", e);
                let _ = self.tenant_cache.invalidate(cache_key).await;
                Ok(None)
            }
        }
    }

    async fn set_cached_tenant(
        &self,
        cache_key: String,
        context: &TenantContext,
    ) -> Result<(), StatusCode> {
        let bytes = serde_json::to_vec(context).map_err(|e| {
            tracing::error!("Tenant cache serialization error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        self.tenant_cache
            .set(cache_key, bytes)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }

    async fn check_negative(&self, cache_key: &str) -> Result<bool, StatusCode> {
        let hit = self
            .tenant_negative_cache
            .get(cache_key)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .is_some();
        if hit {
            self.metrics
                .incr("negative_hits", &self.metrics.local_negative_hits)
                .await;
        } else {
            self.metrics
                .incr("negative_misses", &self.metrics.local_negative_misses)
                .await;
        }
        Ok(hit)
    }

    async fn set_negative(&self, cache_key: String) -> Result<(), StatusCode> {
        self.tenant_negative_cache
            .set(cache_key, Vec::new())
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        self.metrics
            .incr("negative_inserts", &self.metrics.local_negative_inserts)
            .await;
        Ok(())
    }

    async fn invalidate_pair(&self, cache_key: &str, negative_key: &str) {
        let _ = self.tenant_cache.invalidate(cache_key).await;
        let _ = self.tenant_negative_cache.invalidate(negative_key).await;
    }

    async fn get_or_load_with_coalescing<F, Fut>(
        &self,
        cache_key: &str,
        loader: F,
    ) -> Result<TenantContext, StatusCode>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Option<TenantContext>, StatusCode>>,
    {
        loop {
            let notify = {
                let mut in_flight = self.in_flight.lock().await;

                if let Some(existing) = in_flight.get(cache_key) {
                    let notify = existing.clone();
                    drop(in_flight);

                    self.metrics
                        .incr("coalesced_requests", &self.metrics.coalesced_requests)
                        .await;

                    notify.notified().await;

                    if let Some(cached) = self.get_cached_tenant(cache_key).await? {
                        return Ok(cached);
                    }

                    continue;
                }

                let notify = Arc::new(Notify::new());
                in_flight.insert(cache_key.to_string(), notify.clone());
                notify
            };

            let result = loader().await;

            {
                let mut in_flight = self.in_flight.lock().await;
                in_flight.remove(cache_key);
            }

            notify.notify_waiters();

            match result {
                Ok(Some(context)) => {
                    self.set_cached_tenant(cache_key.to_string(), &context)
                        .await?;
                    return Ok(context);
                }
                Ok(None) => return Err(StatusCode::NOT_FOUND),
                Err(e) => return Err(e),
            }
        }
    }
}

async fn build_tenant_cache_backend() -> Arc<dyn CacheBackend> {
    #[cfg(feature = "redis-cache")]
    if let Some(url) = resolve_redis_url() {
        if let Ok(redis_cache) = RedisCacheBackend::new(
            &url,
            format!("tenant-cache:{}:data", TENANT_CACHE_VERSION),
            TENANT_CACHE_TTL,
        )
        .await
        {
            return Arc::new(redis_cache);
        }
    }

    Arc::new(InMemoryCacheBackend::new(
        TENANT_CACHE_TTL,
        TENANT_CACHE_MAX_CAPACITY,
    ))
}

async fn build_negative_tenant_cache_backend() -> Arc<dyn CacheBackend> {
    #[cfg(feature = "redis-cache")]
    if let Some(url) = resolve_redis_url() {
        if let Ok(redis_cache) = RedisCacheBackend::new(
            &url,
            format!("tenant-cache:{}:negative", TENANT_CACHE_VERSION),
            TENANT_NEGATIVE_CACHE_TTL,
        )
        .await
        {
            return Arc::new(redis_cache);
        }
    }

    Arc::new(InMemoryCacheBackend::new(
        TENANT_NEGATIVE_CACHE_TTL,
        TENANT_CACHE_MAX_CAPACITY,
    ))
}

#[cfg(feature = "redis-cache")]
fn resolve_redis_url() -> Option<String> {
    std::env::var("RUSTOK_REDIS_URL")
        .ok()
        .or_else(|| std::env::var("REDIS_URL").ok())
        .filter(|url| !url.trim().is_empty())
}

#[cfg(not(feature = "redis-cache"))]
fn resolve_redis_url() -> Option<String> {
    None
}

#[cfg(feature = "redis-cache")]
fn resolve_redis_client() -> Option<redis::Client> {
    resolve_redis_url().and_then(|url| redis::Client::open(url).ok())
}

pub async fn init_tenant_cache_infrastructure(ctx: &AppContext) {
    if ctx
        .shared_store
        .contains::<Arc<TenantCacheInfrastructure>>()
    {
        return;
    }

    let infra = Arc::new(TenantCacheInfrastructure::new().await);
    ctx.shared_store.insert(infra.clone());

    if let Some(task) = spawn_invalidation_listener(infra.clone()).await {
        ctx.shared_store.insert(task);
    } else {
        infra
            .invalidation_listener_state
            .mark_disabled("redis pubsub invalidation listener is disabled")
            .await;
    }
}

async fn spawn_invalidation_listener(
    infra: Arc<TenantCacheInfrastructure>,
) -> Option<JoinHandle<()>> {
    #[cfg(feature = "redis-cache")]
    {
        let client = resolve_redis_client()?;
        let listener_state = infra.invalidation_listener_state.clone();
        let task = tokio::spawn(async move {
            let runtime = EventConsumerRuntime::new("tenant_invalidation_listener");
            let mut reason = "startup";

            loop {
                runtime.restarted(reason);
                listener_state.mark_starting().await;

                if let Err(error) = consume_tenant_invalidation_messages(
                    &client,
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
    client: &redis::Client,
    infra: Arc<TenantCacheInfrastructure>,
    listener_state: Arc<TenantInvalidationListenerState>,
) -> Result<(), String> {
    let mut pubsub = client
        .get_async_pubsub()
        .await
        .map_err(|error| format!("pubsub connection failed: {error}"))?;

    pubsub
        .subscribe(TENANT_INVALIDATION_CHANNEL)
        .await
        .map_err(|error| format!("pubsub subscribe failed: {error}"))?;

    listener_state.mark_healthy().await;

    let mut messages = pubsub.on_message();
    use futures_util::StreamExt;

    while let Some(msg) = messages.next().await {
        let payload: Result<String, _> = msg.get_payload();
        let Ok(payload) = payload else {
            continue;
        };

        let Some((cache_key, negative_key)) = parse_invalidation_payload(&payload) else {
            tracing::warn!(
                channel = TENANT_INVALIDATION_CHANNEL,
                payload = %payload,
                "Ignoring malformed tenant invalidation payload"
            );
            continue;
        };

        infra.invalidate_pair(cache_key, negative_key).await;
    }

    Err("pubsub stream closed".to_string())
}

fn parse_invalidation_payload(payload: &str) -> Option<(&str, &str)> {
    let mut parts = payload.split('|');
    let cache_key = parts.next()?;
    let negative_key = parts.next()?;
    if cache_key.is_empty() || negative_key.is_empty() {
        return None;
    }
    Some((cache_key, negative_key))
}

fn tenant_infra(ctx: &AppContext) -> Option<Arc<TenantCacheInfrastructure>> {
    ctx.shared_store.get::<Arc<TenantCacheInfrastructure>>()
}

pub async fn resolve(
    State(ctx): State<AppContext>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let settings = RustokSettings::from_settings(&ctx.config.settings)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let identifier = resolve_identifier(&req, &settings)?;

    let Some(infra) = tenant_infra(&ctx) else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let cache_key = infra
        .key_builder
        .kind_key(identifier.kind, &identifier.value);
    let negative_key = infra
        .key_builder
        .kind_negative_key(identifier.kind, &identifier.value);

    if infra.check_negative(&negative_key).await? {
        return Err(StatusCode::NOT_FOUND);
    }

    if let Some(cached_context) = infra.get_cached_tenant(&cache_key).await? {
        req.extensions_mut()
            .insert(TenantContextExtension(cached_context));
        return Ok(next.run(req).await);
    }

    let db = ctx.db.clone();
    let ident_kind = identifier.kind;
    let ident_value = identifier.value.clone();
    let ident_uuid = identifier.uuid;
    let negative_key_clone = negative_key.clone();
    let infra_clone = infra.clone();

    let context = infra
        .get_or_load_with_coalescing(&cache_key, || async move {
            let tenant = match ident_kind {
                TenantIdentifierKind::Uuid => tenants::Entity::find_by_id(&db, ident_uuid)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                TenantIdentifierKind::Slug => tenants::Entity::find_by_slug(&db, &ident_value)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
                TenantIdentifierKind::Host => tenants::Entity::find_by_domain(&db, &ident_value)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
            };

            match tenant {
                Some(tenant) => Ok(Some(TenantContext::from_model(&tenant))),
                None => {
                    infra_clone.set_negative(negative_key_clone).await?;
                    Ok(None)
                }
            }
        })
        .await?;

    req.extensions_mut().insert(TenantContextExtension(context));
    Ok(next.run(req).await)
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
            let header_value = req
                .headers()
                .get(&settings.tenant.header_name)
                .and_then(|value| value.to_str().ok());

            let identifier = header_value
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| settings.tenant.default_id.to_string());

            // Validate and classify the identifier
            classify_and_validate_identifier(&identifier).map_err(|e| {
                tracing::warn!(
                    identifier = %identifier,
                    error = %e,
                    "Invalid tenant identifier from header"
                );
                StatusCode::BAD_REQUEST
            })
        }
        "host" | "domain" | "subdomain" => {
            let host = extract_host(req.headers()).ok_or(StatusCode::BAD_REQUEST)?;
            let host_without_port = host.split(':').next().unwrap_or(host);

            // Validate hostname
            let validated_host = TenantIdentifierValidator::validate_host(host_without_port)
                .map_err(|e| {
                    tracing::warn!(
                        host = %host_without_port,
                        error = %e,
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
        _ => Ok(ResolvedTenantIdentifier {
            value: settings.tenant.default_id.to_string(),
            kind: TenantIdentifierKind::Uuid,
            uuid: settings.tenant.default_id,
        }),
    }
}

fn extract_host(headers: &axum::http::HeaderMap) -> Option<&str> {
    if let Some(host) = headers
        .get("x-forwarded-host")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
    {
        return Some(host.trim());
    }

    if let Some(forwarded) = headers.get(FORWARDED).and_then(|value| value.to_str().ok()) {
        if let Some(host) = parse_forwarded_host(forwarded) {
            return Some(host);
        }
    }

    headers.get(HOST).and_then(|value| value.to_str().ok())
}

fn parse_forwarded_host(forwarded: &str) -> Option<&str> {
    forwarded
        .split(',')
        .next()
        .and_then(|entry| {
            entry
                .split(';')
                .find(|part| part.trim_start().starts_with("host="))
        })
        .and_then(|part| part.trim_start().strip_prefix("host="))
        .map(|host| host.trim_matches('"').trim())
}

/// Classifies and validates a tenant identifier with security checks
fn classify_and_validate_identifier(
    value: &str,
) -> Result<ResolvedTenantIdentifier, rustok_core::tenant_validation::TenantValidationError> {
    // Try UUID first (most specific)
    if let Ok(uuid) = TenantIdentifierValidator::validate_uuid(value) {
        return Ok(ResolvedTenantIdentifier {
            value: uuid.to_string(),
            kind: TenantIdentifierKind::Uuid,
            uuid,
        });
    }

    // Try slug (with security validation)
    let validated_slug = TenantIdentifierValidator::validate_slug(value)?;

    Ok(ResolvedTenantIdentifier {
        value: validated_slug,
        kind: TenantIdentifierKind::Slug,
        uuid: Uuid::nil(),
    })
}

/// Classifies a tenant identifier without returning errors (best-effort).
/// Falls back to UUID kind if the identifier doesn't match slug/UUID patterns.
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

pub async fn tenant_cache_stats(ctx: &AppContext) -> TenantCacheStats {
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
    ctx: &AppContext,
) -> TenantInvalidationListenerSnapshot {
    let Some(infra) = tenant_infra(ctx) else {
        return TenantInvalidationListenerSnapshot {
            status: TenantInvalidationListenerStatus::Disabled,
            last_error: Some("tenant cache infrastructure not initialized".to_string()),
        };
    };

    infra.invalidation_listener_state.snapshot().await
}

/// Invalidate cached tenant (call after tenant or domain update)
pub async fn invalidate_tenant_cache(ctx: &AppContext, identifier: &str) {
    let resolved = classify_identifier(identifier.to_string());
    invalidate_cache_keys(ctx, resolved.kind, &resolved.value).await;
}

pub async fn invalidate_tenant_cache_by_host(ctx: &AppContext, host: &str) {
    invalidate_cache_keys(ctx, TenantIdentifierKind::Host, host).await;
}

pub async fn invalidate_tenant_cache_by_uuid(ctx: &AppContext, tenant_id: Uuid) {
    invalidate_cache_keys(ctx, TenantIdentifierKind::Uuid, &tenant_id.to_string()).await;
}

pub async fn invalidate_tenant_cache_by_slug(ctx: &AppContext, slug: &str) {
    invalidate_cache_keys(ctx, TenantIdentifierKind::Slug, slug).await;
}

async fn invalidate_cache_keys(ctx: &AppContext, kind: TenantIdentifierKind, value: &str) {
    let Some(infra) = tenant_infra(ctx) else {
        return;
    };

    let cache_key = infra.key_builder.kind_key(kind, value);
    let negative_key = infra.key_builder.kind_negative_key(kind, value);
    infra.invalidate_pair(&cache_key, &negative_key).await;

    let payload = format!("{cache_key}|{negative_key}");
    infra.invalidation_publisher.publish(&payload).await;
}

#[cfg(test)]
mod invalidation_tests {
    use super::{
        parse_invalidation_payload, TenantInvalidationListenerState,
        TenantInvalidationListenerStatus,
    };

    #[test]
    fn parse_invalidation_payload_returns_both_keys() {
        let payload = "tenant:v1:slug:demo|tenant_negative:v1:slug:demo";
        let parsed = parse_invalidation_payload(payload);

        assert_eq!(
            parsed,
            Some(("tenant:v1:slug:demo", "tenant_negative:v1:slug:demo"))
        );
    }

    #[test]
    fn parse_invalidation_payload_rejects_malformed_payload() {
        assert_eq!(parse_invalidation_payload("tenant:v1:slug:demo"), None);
        assert_eq!(
            parse_invalidation_payload("|tenant_negative:v1:slug:demo"),
            None
        );
        assert_eq!(parse_invalidation_payload("tenant:v1:slug:demo|"), None);
    }

    #[tokio::test]
    async fn listener_state_snapshot_reflects_degraded_status() {
        let state = TenantInvalidationListenerState::new();
        state.mark_degraded("redis unavailable").await;

        let snapshot = state.snapshot().await;

        assert_eq!(snapshot.status, TenantInvalidationListenerStatus::Degraded);
        assert_eq!(snapshot.last_error.as_deref(), Some("redis unavailable"));
    }
}
