use std::{
    collections::HashMap,
    sync::{
        Arc,
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
use rustok_api::request::{ResolvedRequestLocale, resolve_request_locale};
use rustok_core::i18n::Locale;
use sea_orm::ConnectionTrait;
use sea_orm::sea_query::{Alias, Expr, Order, Query};
use uuid::Uuid;

use crate::context::TenantContextExt;
use crate::services::server_runtime_context::ServerRuntimeContext;

const TENANT_LOCALE_CACHE_TTL: Duration = Duration::from_secs(60);
const TENANT_LOCALE_CACHE_MAX_WEIGHT_BYTES: u64 = 8 * 1024 * 1024;

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
struct TenantLocaleCache {
    cache: Cache<Uuid, Arc<Vec<TenantLocaleRecord>>>,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    db_queries: Arc<AtomicU64>,
    invalidations: Arc<AtomicU64>,
}

impl TenantLocaleCache {
    fn new() -> Self {
        Self::with_max_weight(TENANT_LOCALE_CACHE_MAX_WEIGHT_BYTES)
    }

    fn with_max_weight(max_weight_bytes: u64) -> Self {
        Self {
            cache: Cache::builder()
                .time_to_live(TENANT_LOCALE_CACHE_TTL)
                .weigher(tenant_locale_entry_weight)
                .max_capacity(max_weight_bytes)
                .build(),
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            db_queries: Arc::new(AtomicU64::new(0)),
            invalidations: Arc::new(AtomicU64::new(0)),
        }
    }

    async fn get(&self, tenant_id: Uuid) -> Option<Arc<Vec<TenantLocaleRecord>>> {
        let cached = self.cache.get(&tenant_id).await;
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

        let cache = self.clone();
        self.cache
            .try_get_with(tenant_id, async move {
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
        self.cache.invalidate(&tenant_id).await;
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

fn tenant_locale_entry_weight(_tenant_id: &Uuid, locales: &Arc<Vec<TenantLocaleRecord>>) -> u32 {
    let mut weight = std::mem::size_of::<Uuid>()
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
        if !locales.is_empty() {
            resolved.effective_locale =
                constrain_locale_to_tenant(&resolved, locales.as_ref(), &tenant.default_locale);
        }
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
    let statement = Query::select()
        .from(Alias::new("tenant_locales"))
        .columns([
            Alias::new("locale"),
            Alias::new("is_enabled"),
            Alias::new("is_default"),
            Alias::new("fallback_locale"),
        ])
        .and_where(Expr::col(Alias::new("tenant_id")).eq(tenant_id))
        .order_by(Alias::new("is_default"), Order::Desc)
        .order_by(Alias::new("locale"), Order::Asc)
        .to_owned();

    let rows = ctx
        .db()
        .query_all(ctx.db().get_database_backend().build(&statement))
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok(TenantLocaleRecord {
                locale: row.try_get("", "locale")?,
                is_enabled: row.try_get("", "is_enabled")?,
                is_default: row.try_get("", "is_default")?,
                fallback_locale: row.try_get("", "fallback_locale").ok(),
            })
        })
        .collect()
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
        .unwrap_or_else(|| resolved.effective_locale.clone())
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
