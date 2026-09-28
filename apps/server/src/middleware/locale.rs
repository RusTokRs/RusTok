use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use axum::{
    extract::{Request, State},
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use moka::future::Cache;
use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, PortActor, PortContext,
    request::{ResolvedRequestLocale, resolve_request_locale},
};
use rustok_core::i18n::Locale;
use rustok_tenant::{TenantLocalePolicyPort, TenantService};
use uuid::Uuid;

use crate::context::TenantContextExt;
use crate::services::server_runtime_context::ServerRuntimeContext;

const TENANT_LOCALE_CACHE_TTL: Duration = Duration::from_secs(60);
const TENANT_LOCALE_CACHE_MAX_WEIGHT_BYTES: u64 = 8 * 1024 * 1024;
const TENANT_LOCALE_PORT_TIMEOUT: Duration = Duration::from_secs(2);
const TENANT_LOCALE_CACHE_MAX_TENANT_VERSIONS: usize = 16 * 1024;

#[derive(Debug, Clone)]
struct TenantLocaleRecord {
    locale: String,
    is_enabled: bool,
    is_default: bool,
    fallback_locale: Option<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TenantLocaleCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub db_queries: u64,
    pub invalidations: u64,
    pub entries: u64,
}

#[derive(Clone)]
struct TenantLocaleCacheVersionState {
    next_version: u64,
    default_version: u64,
    tenant_versions: HashMap<Uuid, u64>,
    exhausted: bool,
}

impl Default for TenantLocaleCacheVersionState {
    fn default() -> Self {
        Self {
            next_version: 1,
            default_version: 1,
            tenant_versions: HashMap::new(),
            exhausted: false,
        }
    }
}

impl TenantLocaleCacheVersionState {
    fn token(&self, tenant_id: Uuid) -> Option<u64> {
        if self.exhausted {
            return None;
        }
        Some(
            self.tenant_versions
                .get(&tenant_id)
                .copied()
                .unwrap_or(self.default_version),
        )
    }

    fn invalidate(&mut self, tenant_id: Uuid, maximum_tenants: usize) -> bool {
        if self.exhausted {
            return true;
        }

        let Some(next_version) = self.next_version.checked_add(1) else {
            self.exhausted = true;
            self.tenant_versions.clear();
            return true;
        };
        self.next_version = next_version;

        if !self.tenant_versions.contains_key(&tenant_id)
            && self.tenant_versions.len() >= maximum_tenants.max(1)
        {
            self.default_version = next_version;
            self.tenant_versions.clear();
            self.tenant_versions.insert(tenant_id, next_version);
            return true;
        }

        self.tenant_versions.insert(tenant_id, next_version);
        false
    }

    fn invalidate_all(&mut self) {
        if self.exhausted {
            self.tenant_versions.clear();
            return;
        }

        let Some(next_version) = self.next_version.checked_add(1) else {
            self.exhausted = true;
            self.tenant_versions.clear();
            return;
        };
        self.next_version = next_version;
        self.default_version = next_version;
        self.tenant_versions.clear();
    }
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct TenantLocaleCacheKey {
    tenant_id: Uuid,
    version: u64,
}

#[derive(Clone)]
struct TenantLocaleCache {
    cache: Cache<TenantLocaleCacheKey, Arc<Vec<TenantLocaleRecord>>>,
    versions: Arc<Mutex<TenantLocaleCacheVersionState>>,
    max_tenant_versions: usize,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    db_queries: Arc<AtomicU64>,
    invalidations: Arc<AtomicU64>,
}

impl TenantLocaleCache {
    fn new() -> Self {
        Self::with_limits(
            TENANT_LOCALE_CACHE_MAX_WEIGHT_BYTES,
            TENANT_LOCALE_CACHE_MAX_TENANT_VERSIONS,
        )
    }

    fn with_max_weight(max_weight_bytes: u64) -> Self {
        Self::with_limits(
            max_weight_bytes,
            TENANT_LOCALE_CACHE_MAX_TENANT_VERSIONS,
        )
    }

    #[cfg(test)]
    fn with_limits(max_weight_bytes: u64, max_tenant_versions: usize) -> Self {
        Self {
            cache: Cache::builder()
                .time_to_live(TENANT_LOCALE_CACHE_TTL)
                .weigher(tenant_locale_entry_weight)
                .max_capacity(max_weight_bytes)
                .build(),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            db_queries: Arc::new(AtomicU64::new(0)),
            versions: Arc::new(Mutex::new(TenantLocaleCacheVersionState::default())),
            max_tenant_versions: TENANT_LOCALE_CACHE_MAX_TENANT_VERSIONS,
            invalidations: Arc::new(AtomicU64::new(0)),
        }
    }

    fn tenant_version(&self, tenant_id: Uuid) -> Option<u64> {
        self.versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .token(tenant_id)
    }

    fn cache_key(&self, tenant_id: Uuid, version: u64) -> TenantLocaleCacheKey {
        TenantLocaleCacheKey { tenant_id, version }
    }

    async fn get(&self, tenant_id: Uuid) -> Option<Arc<Vec<TenantLocaleRecord>>> {
        let Some(version) = self.tenant_version(tenant_id) else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        let cached = self.cache.get(&self.cache_key(tenant_id, version)).await;
        if cached.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }
        cached
    }

    async fn get_or_load(
        &self,
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
    ) -> Result<Arc<Vec<TenantLocaleRecord>>, sea_orm::DbErr> {
        if let Some(locales) = self.get(tenant_id).await {
            return Ok(locales);
        }

        let Some(version) = self.tenant_version(tenant_id) else {
            self.record_db_query();
            return load_tenant_locales(ctx, tenant_id).await.map(Arc::new);
        };

        let cache = self.clone();
        self.cache
            .try_get_with(self.cache_key(tenant_id, version), async move {
                cache.record_db_query();
                load_tenant_locales(ctx, tenant_id).await.map(Arc::new)
            })
            .await
            .map_err(|error| {
                sea_orm::DbErr::Custom(format!("tenant locale cache load failed: {error}"))
            })
    }

    async fn invalidate(&self, tenant_id: Uuid) {
        self.invalidations.fetch_add(1, Ordering::Relaxed);

        let (old_version, invalidate_all) = {
            let mut versions = self
                .versions
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let old_version = versions.token(tenant_id);
            let invalidate_all = versions.invalidate(tenant_id, self.max_tenant_versions);
            (old_version, invalidate_all)
        };

        if invalidate_all {
            self.cache.invalidate_all();
            self.cache.run_pending_tasks().await;
            return;
        }

        if let Some(old_version) = old_version {
            self.cache
                .invalidate(&self.cache_key(tenant_id, old_version))
                .await;
            self.cache.run_pending_tasks().await;
        }
    }

    async fn invalidate_all(&self) {
        self.invalidations.fetch_add(1, Ordering::Relaxed);
        self.versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invalidate_all();
        self.cache.invalidate_all();
        self.cache.run_pending_tasks().await;
    }

    #[cfg(test)]
    fn tracked_tenant_versions(&self) -> usize {
        self.versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tenant_versions
            .len()
    }

    #[cfg(test)]
    fn exhaust_versions(&self) {
        let mut versions = self
            .versions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        versions.next_version = u64::MAX;
    }

    fn record_db_query(&self) {
        self.db_queries.fetch_add(1, Ordering::Relaxed);
    }

    fn stats(&self) -> TenantLocaleCacheStats {
        TenantLocaleCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            db_queries: self.db_queries.load(Ordering::Relaxed),
            invalidations: self.invalidations.load(Ordering::Relaxed),
            entries: self.cache.entry_count(),
        }
    }
}

fn tenant_locale_entry_weight(
    _key: &TenantLocaleCacheKey,
    locales: &Arc<Vec<TenantLocaleRecord>>,
) -> u32 {
    let mut weight = std::mem::size_of::<TenantLocaleCacheKey>()
        .saturating_add(std::mem::size_of::<Arc<Vec<TenantLocaleRecord>>>())
        .saturating_add(std::mem::size_of::<Vec<TenantLocaleRecord>>());
    for locale in locales.iter() {
        weight = weight
            .saturating_add(std::mem::size_of::<TenantLocaleRecord>())
            .saturating_add(locale.locale.len())
            .saturating_add(locale.fallback_locale.as_ref().map_or(0, String::len));
    }
    weight.clamp(1, u32::MAX as usize) as u32
}

fn tenant_locale_cache(ctx: &ServerRuntimeContext) -> Arc<TenantLocaleCache> {
    let candidate = Arc::new(TenantLocaleCache::new());
    let _ = ctx.shared_insert_if_absent(candidate.clone());
    ctx.shared_get::<Arc<TenantLocaleCache>>()
        .unwrap_or(candidate)
}

pub async fn resolve_locale(
    State(ctx): State<ServerRuntimeContext>,
    request: Request,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let (mut parts, body) = request.into_parts();
    let tenant_context = parts.extensions.tenant_context().cloned();
    let mut resolved = resolve_request_locale(
        &parts,
        tenant_context
            .as_ref()
            .map(|tenant| tenant.default_locale.as_str()),
    );

    if let Some(tenant) = tenant_context.as_ref() {
        let locales = get_tenant_locales_cached(&ctx, tenant.id)
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        resolved.effective_locale =
            constrain_locale_to_tenant(&resolved, locales.as_ref(), &tenant.default_locale);
    }

    let locale = Locale::parse(&resolved.effective_locale).unwrap_or_default();
    parts.extensions.insert(resolved.clone());
    parts.extensions.insert(locale);

    let request = Request::from_parts(parts, body);
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&resolved.effective_locale) {
        response.headers_mut().insert("content-language", value);
    }
    Ok(response)
}

async fn get_tenant_locales_cached(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
) -> Result<Arc<Vec<TenantLocaleRecord>>, sea_orm::DbErr> {
    tenant_locale_cache(ctx).get_or_load(ctx, tenant_id).await
}

pub async fn invalidate_tenant_locale_cache(ctx: &ServerRuntimeContext, tenant_id: Uuid) {
    tenant_locale_cache(ctx).invalidate(tenant_id).await;
}

pub async fn tenant_locale_cache_stats(ctx: &ServerRuntimeContext) -> TenantLocaleCacheStats {
    ctx.shared_get::<Arc<TenantLocaleCache>>()
        .map(|cache| cache.stats())
        .unwrap_or_default()
}

async fn load_tenant_locales(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
) -> Result<Vec<TenantLocaleRecord>, sea_orm::DbErr> {
    let service = TenantService::new(ctx.db_clone());
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-server.locale-resolver"),
        PLATFORM_FALLBACK_LOCALE,
        format!("tenant-locale-policy:{tenant_id}"),
    )
    .with_deadline(TENANT_LOCALE_PORT_TIMEOUT);
    let policy = service.read_locale_policy(context).await.map_err(|error| {
        sea_orm::DbErr::Custom(format!(
            "tenant locale policy port failed [{}]: {}",
            error.code, error.message
        ))
    })?;

    Ok(policy
        .locales
        .into_iter()
        .map(|locale| TenantLocaleRecord {
            locale: locale.locale.as_str().to_string(),
            is_enabled: locale.is_enabled,
            is_default: locale.is_default,
            fallback_locale: locale
                .fallback_locale
                .map(|fallback| fallback.as_str().to_string()),
        })
        .collect())
}

fn constrain_locale_to_tenant(
    resolved: &ResolvedRequestLocale,
    locales: &[TenantLocaleRecord],
    tenant_default_locale: &str,
) -> String {
    let locale_map = locales
        .iter()
        .map(|record| (record.locale.as_str(), record))
        .collect::<HashMap<_, _>>();

    if let Some(requested_locale) = resolved.requested_locale.as_deref() {
        if locale_map
            .get(requested_locale)
            .is_some_and(|record| record.is_enabled)
        {
            return requested_locale.to_string();
        }

        if let Some(fallback) = locale_map
            .get(requested_locale)
            .and_then(|record| record.fallback_locale.as_deref())
            .and_then(|fallback_locale| locale_map.get(fallback_locale))
            .filter(|record| record.is_enabled)
            .map(|record| record.locale.clone())
        {
            return fallback;
        }
    }

    if let Some(default_locale) = locales
        .iter()
        .find(|record| record.is_default && record.is_enabled)
        .map(|record| record.locale.clone())
    {
        return default_locale;
    }

    if locale_map
        .get(tenant_default_locale)
        .is_some_and(|record| record.is_enabled)
    {
        return tenant_default_locale.to_string();
    }

    locales
        .iter()
        .find(|record| record.is_enabled)
        .map(|record| record.locale.clone())
        .unwrap_or_else(|| {
            rustok_api::normalize_locale_tag(tenant_default_locale)
                .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string())
        })
}

/// Invalidate every process-local tenant-locale entry after an unverified or gapped durable
/// tenant-generation recovery. Do not create the cache merely to clear an empty runtime.
pub async fn invalidate_all_tenant_locale_cache(ctx: &ServerRuntimeContext) {
    let Some(cache) = ctx.shared_get::<Arc<TenantLocaleCache>>() else {
        return;
    };
    cache.invalidate_all().await;
}

#[cfg(test)]
mod tests {
    use super::{
        TenantLocaleCache, TenantLocaleRecord, constrain_locale_to_tenant,
        tenant_locale_entry_weight,
    };
    use rustok_api::request::ResolvedRequestLocale;
    use std::sync::Arc;
    use uuid::Uuid;

    #[test]
    fn locale_cache_weight_accounts_for_dynamic_strings() {
        let tenant_id = Uuid::new_v4();
        let short = Arc::new(vec![TenantLocaleRecord {
            locale: "en".to_string(),
            is_enabled: true,
            is_default: true,
            fallback_locale: None,
        }]);
        let long = Arc::new(vec![TenantLocaleRecord {
            locale: "x".repeat(512),
            is_enabled: true,
            is_default: false,
            fallback_locale: Some("y".repeat(512)),
        }]);

        assert!(
            tenant_locale_entry_weight(&tenant_id, &long)
                > tenant_locale_entry_weight(&tenant_id, &short)
        );
    }

    #[tokio::test]
    async fn tenant_locale_cache_tracks_hits_misses_and_invalidations() {
        let cache = TenantLocaleCache::new();
        let tenant_id = Uuid::new_v4();

        assert!(cache.get(tenant_id).await.is_none());
        cache.record_db_query();
        cache
            .cache
            .insert(
                tenant_id,
                Arc::new(vec![TenantLocaleRecord {
                    locale: "en".to_string(),
                    is_enabled: true,
                    is_default: true,
                    fallback_locale: None,
                }]),
            )
            .await;

        assert!(cache.get(tenant_id).await.is_some());
        cache.invalidate(tenant_id).await;
        assert!(cache.get(tenant_id).await.is_none());

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.db_queries, 1);
        assert_eq!(stats.invalidations, 1);
    }

    #[tokio::test]
    async fn tenant_cache_version_rotates_on_tenant_invalidation() {
        let cache = TenantLocaleCache::with_max_weight(1024 * 1024);
        let tenant_id = Uuid::new_v4();
        let initial = cache.tenant_version(tenant_id).expect("cache should be enabled");
        let key = cache.cache_key(tenant_id, initial);

        cache
            .cache
            .insert(
                key.clone(),
                Arc::new(vec![TenantLocaleRecord {
                    locale: "en".to_string(),
                    is_enabled: true,
                    is_default: true,
                    fallback_locale: None,
                }]),
            )
            .await;
        assert!(cache.get(tenant_id).await.is_some());

        cache.invalidate(tenant_id).await;

        let next = cache.tenant_version(tenant_id).expect("cache should remain enabled");
        assert_ne!(initial, next);
        assert!(cache.get(tenant_id).await.is_none());
    }

    #[tokio::test]
    async fn tenant_cache_version_registry_is_bounded_and_fails_closed_on_exhaustion() {
        let cache = TenantLocaleCache::with_limits(1024 * 1024, 2);
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let third = Uuid::new_v4();

        cache.invalidate(first).await;
        cache.invalidate(second).await;
        assert_eq!(cache.tracked_tenant_versions(), 2);

        cache.invalidate(third).await;
        assert!(cache.tracked_tenant_versions() <= 2);
        assert!(cache.tenant_version(third).is_some());
        assert!(cache.tenant_version(first).is_some());

        cache.exhaust_versions();
        cache.invalidate(first).await;
        assert!(cache.tenant_version(first).is_none());
        assert_eq!(cache.tracked_tenant_versions(), 0);
    }

    #[test]
    fn empty_tenant_locale_policy_never_accepts_requested_locale() {
        let resolved = ResolvedRequestLocale {
            requested_locale: Some("ru".to_string()),
            effective_locale: "ru".to_string(),
        };

        assert_eq!(
            constrain_locale_to_tenant(&resolved, &[], "en"),
            "en"
        );
    }

    #[test]
    fn prefers_requested_enabled_locale() {
        let resolved = ResolvedRequestLocale {
            requested_locale: Some("ru".to_string()),
            effective_locale: "ru".to_string(),
        };
        let locales = vec![
            TenantLocaleRecord {
                locale: "en".to_string(),
                is_enabled: true,
                is_default: true,
                fallback_locale: None,
            },
            TenantLocaleRecord {
                locale: "ru".to_string(),
                is_enabled: true,
                is_default: false,
                fallback_locale: Some("en".to_string()),
            },
        ];

        assert_eq!(constrain_locale_to_tenant(&resolved, &locales, "en"), "ru");
    }

    #[test]
    fn falls_back_from_disabled_requested_locale() {
        let resolved = ResolvedRequestLocale {
            requested_locale: Some("de".to_string()),
            effective_locale: "de".to_string(),
        };
        let locales = vec![
            TenantLocaleRecord {
                locale: "en".to_string(),
                is_enabled: true,
                is_default: true,
                fallback_locale: None,
            },
            TenantLocaleRecord {
                locale: "de".to_string(),
                is_enabled: false,
                is_default: false,
                fallback_locale: Some("en".to_string()),
            },
        ];

        assert_eq!(constrain_locale_to_tenant(&resolved, &locales, "en"), "en");
    }
}
