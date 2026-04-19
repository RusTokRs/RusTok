use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use loco_rs::app::AppContext;
use moka::future::Cache;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::common::{
    extract_effective_host, peer_ip_from_extensions, RustokSettings, SharedRustokSettings,
};
use crate::context::{
    ChannelContext, ChannelContextExtension, ChannelResolutionSource, TenantContextExt,
};
use rustok_api::{ChannelResolutionOutcome, ChannelResolutionStage, ChannelResolutionTraceStep};
use rustok_channel::{
    ChannelResolutionOrigin, ChannelResolver, RequestFacts, ResolutionDecision, TargetSurface,
};

const CHANNEL_ID_HEADER: &str = "X-Channel-ID";
const CHANNEL_SLUG_HEADER: &str = "X-Channel-Slug";
const CHANNEL_CACHE_TTL: Duration = Duration::from_secs(60);
const CHANNEL_CACHE_MAX_CAPACITY: u64 = 2_000;

#[derive(Clone)]
struct ChannelResolutionCache {
    cache: Cache<ChannelCacheKey, Option<ChannelContext>>,
    tenant_versions: Arc<RwLock<HashMap<Uuid, u64>>>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ChannelCacheKey {
    tenant_id: Uuid,
    version: u64,
    header_channel_id: Option<Uuid>,
    header_channel_slug: Option<String>,
    query_channel_slug: Option<String>,
    host: Option<String>,
}

impl ChannelResolutionCache {
    fn new() -> Self {
        Self {
            cache: Cache::builder()
                .time_to_live(CHANNEL_CACHE_TTL)
                .max_capacity(CHANNEL_CACHE_MAX_CAPACITY)
                .build(),
            tenant_versions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn tenant_version(&self, tenant_id: Uuid) -> u64 {
        self.tenant_versions
            .read()
            .await
            .get(&tenant_id)
            .copied()
            .unwrap_or(0)
    }

    async fn invalidate_tenant(&self, tenant_id: Uuid) {
        let mut versions = self.tenant_versions.write().await;
        let next_version = versions
            .get(&tenant_id)
            .copied()
            .unwrap_or(0)
            .saturating_add(1);
        versions.insert(tenant_id, next_version);
    }
}

fn channel_cache(ctx: &AppContext) -> Arc<ChannelResolutionCache> {
    if let Some(cache) = ctx.shared_store.get::<Arc<ChannelResolutionCache>>() {
        return cache;
    }

    let cache = Arc::new(ChannelResolutionCache::new());
    ctx.shared_store.insert(cache.clone());
    cache
}

pub async fn resolve(
    State(ctx): State<AppContext>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let Some(tenant) = req.extensions().tenant_context().cloned() else {
        return Ok(next.run(req).await);
    };

    let shared = ctx
        .shared_store
        .get::<SharedRustokSettings>()
        .ok_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    let settings: &RustokSettings = &shared.0;
    let resolver = ChannelResolver::new(ctx.db.clone());
    let facts = build_request_facts(
        tenant.id,
        req.headers(),
        req.uri().query(),
        peer_ip_from_extensions(req.extensions()),
        &settings,
    );
    let cache = channel_cache(&ctx);
    let cache_key = ChannelCacheKey {
        tenant_id: facts.tenant_id,
        version: cache.tenant_version(facts.tenant_id).await,
        header_channel_id: facts.header_channel_id,
        header_channel_slug: facts.header_channel_slug.clone(),
        query_channel_slug: facts.query_channel_slug.clone(),
        host: facts.host.clone(),
    };

    if let Some(cached_context) = cache.cache.get(&cache_key).await {
        if let Some(context) = cached_context {
            req.extensions_mut()
                .insert(ChannelContextExtension(context));
        }
        return Ok(next.run(req).await);
    }

    let decision = resolver
        .resolve(&facts)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let cached_context =
        resolved_detail_source_and_trace(decision).map(|(detail, source, trace)| {
            let selected_target = detail
                .targets
                .iter()
                .find(|target| target.is_primary)
                .or_else(|| detail.targets.first());
            ChannelContext {
                id: detail.channel.id,
                tenant_id: detail.channel.tenant_id,
                slug: detail.channel.slug,
                name: detail.channel.name,
                is_active: detail.channel.is_active,
                status: detail.channel.status,
                target_type: selected_target.map(|target| target.target_type.clone()),
                target_value: selected_target.map(|target| target.value.clone()),
                settings: detail.channel.settings,
                resolution_source: source,
                resolution_trace: trace,
            }
        });

    cache.cache.insert(cache_key, cached_context.clone()).await;

    if let Some(context) = cached_context {
        req.extensions_mut()
            .insert(ChannelContextExtension(context));
    }

    Ok(next.run(req).await)
}

fn build_request_facts(
    tenant_id: Uuid,
    headers: &HeaderMap,
    query: Option<&str>,
    peer_ip: Option<std::net::IpAddr>,
    settings: &RustokSettings,
) -> RequestFacts {
    RequestFacts {
        tenant_id,
        surface: TargetSurface::Http,
        header_channel_id: channel_id_from_header(headers),
        header_channel_slug: channel_slug_from_header(headers),
        query_channel_slug: channel_slug_from_query(query),
        host: extract_effective_host(headers, peer_ip, &settings.runtime.request_trust),
        oauth_app_id: None,
        locale: None,
    }
}

fn resolved_detail_source_and_trace(
    decision: ResolutionDecision,
) -> Option<(
    rustok_channel::ChannelDetailResponse,
    ChannelResolutionSource,
    Vec<ChannelResolutionTraceStep>,
)> {
    let detail = decision.detail?;
    let source = match decision.source? {
        ChannelResolutionOrigin::HeaderId => ChannelResolutionSource::HeaderId,
        ChannelResolutionOrigin::HeaderSlug => ChannelResolutionSource::HeaderSlug,
        ChannelResolutionOrigin::Query => ChannelResolutionSource::Query,
        ChannelResolutionOrigin::Host => ChannelResolutionSource::Host,
        ChannelResolutionOrigin::Policy => ChannelResolutionSource::Policy,
        ChannelResolutionOrigin::Default => ChannelResolutionSource::Default,
    };

    Some((
        detail,
        source,
        decision.trace.into_iter().map(map_trace_step).collect(),
    ))
}

fn map_trace_step(step: rustok_channel::ResolutionTraceStep) -> ChannelResolutionTraceStep {
    ChannelResolutionTraceStep {
        stage: match step.stage {
            rustok_channel::ResolutionStage::HeaderId => ChannelResolutionStage::HeaderId,
            rustok_channel::ResolutionStage::HeaderSlug => ChannelResolutionStage::HeaderSlug,
            rustok_channel::ResolutionStage::Query => ChannelResolutionStage::Query,
            rustok_channel::ResolutionStage::Host => ChannelResolutionStage::Host,
            rustok_channel::ResolutionStage::Policy => ChannelResolutionStage::Policy,
            rustok_channel::ResolutionStage::Default => ChannelResolutionStage::Default,
        },
        outcome: match step.outcome {
            rustok_channel::ResolutionOutcome::Matched => ChannelResolutionOutcome::Matched,
            rustok_channel::ResolutionOutcome::Miss => ChannelResolutionOutcome::Miss,
            rustok_channel::ResolutionOutcome::Rejected => ChannelResolutionOutcome::Rejected,
        },
        detail: step.detail,
    }
}

fn channel_id_from_header(headers: &axum::http::HeaderMap) -> Option<Uuid> {
    headers
        .get(CHANNEL_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn channel_slug_from_header(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get(CHANNEL_SLUG_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn channel_slug_from_query(query: Option<&str>) -> Option<String> {
    query.and_then(|query| {
        query.split('&').find_map(|segment| {
            let (key, value) = segment.split_once('=')?;
            (key == "channel" && !value.trim().is_empty()).then(|| value.trim().to_string())
        })
    })
}

pub async fn invalidate_tenant_channel_cache(ctx: &AppContext, tenant_id: Uuid) {
    channel_cache(ctx).invalidate_tenant(tenant_id).await;
}

#[cfg(test)]
mod tests {
    use super::{
        build_request_facts, channel_id_from_header, channel_slug_from_header,
        channel_slug_from_query, resolved_detail_source_and_trace, ChannelResolutionOutcome,
        ChannelResolutionStage,
    };
    use crate::common::RustokSettings;
    use crate::context::ChannelResolutionSource;
    use axum::http::{header::HOST, HeaderMap};
    use rustok_channel::{
        migrations, ChannelResolver, ChannelService, CreateChannelInput, CreateChannelTargetInput,
    };
    use rustok_test_utils::setup_test_db;
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
    use sea_orm_migration::SchemaManager;
    use uuid::Uuid;

    async fn setup_channel_db() -> DatabaseConnection {
        let db = setup_test_db().await;
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE tenants (
                id TEXT PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL UNIQUE,
                domain TEXT NULL UNIQUE,
                settings TEXT NOT NULL DEFAULT '{}',
                default_locale TEXT NOT NULL DEFAULT 'en',
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        ))
        .await
        .expect("tenants table should exist for channel foreign keys");
        db.execute(Statement::from_string(
            db.get_database_backend(),
            r#"
            CREATE TABLE o_auth_apps (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                name TEXT NOT NULL,
                slug TEXT NOT NULL,
                app_type TEXT NOT NULL DEFAULT 'machine',
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        ))
        .await
        .expect("o_auth_apps table should exist for channel foreign keys");
        let manager = SchemaManager::new(&db);
        for migration in migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("channel migration should apply");
        }
        db
    }

    async fn seed_tenant(db: &DatabaseConnection, tenant_id: Uuid, slug: &str) {
        db.execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                tenant_id.into(),
                format!("{slug} tenant").into(),
                slug.to_string().into(),
                "{}".to_string().into(),
                "en".to_string().into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");
    }

    async fn create_channel(service: &ChannelService, tenant_id: Uuid, slug: &str) -> Uuid {
        service
            .create_channel(CreateChannelInput {
                tenant_id,
                slug: slug.to_string(),
                name: slug.to_string(),
                settings: None,
            })
            .await
            .expect("channel should be created")
            .id
    }

    async fn add_web_target(service: &ChannelService, channel_id: Uuid, host: &str) {
        service
            .add_target(
                channel_id,
                CreateChannelTargetInput {
                    target_type: "web_domain".to_string(),
                    value: host.to_string(),
                    is_primary: true,
                    settings: None,
                },
            )
            .await
            .expect("host target should be created");
    }

    fn test_settings() -> RustokSettings {
        RustokSettings::default()
    }

    #[test]
    fn parses_channel_id_header() {
        let mut headers = HeaderMap::new();
        let channel_id = Uuid::new_v4();
        headers.insert(
            "X-Channel-ID",
            channel_id.to_string().parse().expect("header"),
        );

        assert_eq!(channel_id_from_header(&headers), Some(channel_id));
    }

    #[test]
    fn parses_channel_slug_from_header_and_query() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Channel-Slug", "mobile-app".parse().expect("header"));

        assert_eq!(
            channel_slug_from_header(&headers).as_deref(),
            Some("mobile-app")
        );
        assert_eq!(
            channel_slug_from_query(Some("locale=ru&channel=web-store")).as_deref(),
            Some("web-store")
        );
    }

    #[tokio::test]
    async fn select_channel_prefers_header_id_over_slug_query_host_and_default() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());

        let _default_channel_id = create_channel(&service, tenant_id, "default").await;
        let header_id_channel_id = create_channel(&service, tenant_id, "header-id").await;
        let _header_slug_channel_id = create_channel(&service, tenant_id, "header-slug").await;
        let _query_channel_id = create_channel(&service, tenant_id, "query-channel").await;
        let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
        add_web_target(&service, host_channel_id, "shop.example.test").await;

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Channel-ID",
            header_id_channel_id
                .to_string()
                .parse()
                .expect("channel id header"),
        );
        headers.insert(
            "X-Channel-Slug",
            "header-slug".parse().expect("slug header"),
        );
        headers.insert(HOST, "shop.example.test".parse().expect("host header"));

        let resolver = ChannelResolver::new(db.clone());
        let selected = resolved_detail_source_and_trace(
            resolver
                .resolve(&build_request_facts(
                    tenant_id,
                    &headers,
                    Some("channel=query-channel"),
                    None,
                    &test_settings(),
                ))
                .await
                .expect("resolution should succeed"),
        )
        .expect("channel should be resolved");

        assert_eq!(selected.0.channel.id, header_id_channel_id);
        assert_eq!(selected.0.channel.slug, "header-id");
        assert_eq!(selected.1, ChannelResolutionSource::HeaderId);
    }

    #[tokio::test]
    async fn select_channel_falls_back_from_missing_query_to_host() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());

        let _default_channel_id = create_channel(&service, tenant_id, "default").await;
        let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
        add_web_target(&service, host_channel_id, "https://shop.example.test/").await;

        let mut headers = HeaderMap::new();
        headers.insert(HOST, "SHOP.EXAMPLE.TEST.:443".parse().expect("host header"));

        let resolver = ChannelResolver::new(db.clone());
        let selected = resolved_detail_source_and_trace(
            resolver
                .resolve(&build_request_facts(
                    tenant_id,
                    &headers,
                    Some("channel=missing"),
                    None,
                    &test_settings(),
                ))
                .await
                .expect("resolution should succeed"),
        )
        .expect("host fallback should resolve");

        assert_eq!(selected.0.channel.id, host_channel_id);
        assert_eq!(selected.0.channel.slug, "host-channel");
        assert_eq!(selected.1, ChannelResolutionSource::Host);
        assert_eq!(selected.0.targets.len(), 1);
        assert_eq!(selected.0.targets[0].value, "shop.example.test");
    }

    #[tokio::test]
    async fn select_channel_falls_back_to_default_when_no_selector_matches() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());

        let first_channel_id = create_channel(&service, tenant_id, "default").await;
        let explicit_default_channel_id = create_channel(&service, tenant_id, "secondary").await;
        service
            .set_default_channel(explicit_default_channel_id)
            .await
            .expect("explicit default channel should be saved");

        let headers = HeaderMap::new();
        let resolver = ChannelResolver::new(db.clone());
        let selected = resolved_detail_source_and_trace(
            resolver
                .resolve(&build_request_facts(
                    tenant_id,
                    &headers,
                    Some("channel=missing"),
                    None,
                    &test_settings(),
                ))
                .await
                .expect("resolution should succeed"),
        )
        .expect("default fallback should resolve");

        assert_ne!(selected.0.channel.id, first_channel_id);
        assert_eq!(selected.0.channel.id, explicit_default_channel_id);
        assert_eq!(selected.0.channel.slug, "secondary");
        assert_eq!(selected.1, ChannelResolutionSource::Default);
    }

    #[tokio::test]
    async fn select_channel_skips_inactive_explicit_slug_and_uses_host_fallback() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());

        let inactive_channel_id = create_channel(&service, tenant_id, "inactive").await;
        db.execute(Statement::from_sql_and_values(
            db.get_database_backend(),
            "UPDATE channels SET is_active = ? WHERE id = ?",
            [false.into(), inactive_channel_id.into()],
        ))
        .await
        .expect("channel should be deactivated");

        let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
        add_web_target(&service, host_channel_id, "shop.example.test").await;

        let mut headers = HeaderMap::new();
        headers.insert("X-Channel-Slug", "inactive".parse().expect("slug header"));
        headers.insert(HOST, "SHOP.EXAMPLE.TEST.:443".parse().expect("host header"));

        let resolver = ChannelResolver::new(db.clone());
        let selected = resolved_detail_source_and_trace(
            resolver
                .resolve(&build_request_facts(
                    tenant_id,
                    &headers,
                    None,
                    None,
                    &test_settings(),
                ))
                .await
                .expect("resolution should succeed"),
        )
        .expect("inactive channel must be skipped");

        assert_eq!(selected.0.channel.id, host_channel_id);
        assert_eq!(selected.0.channel.slug, "host-channel");
        assert_eq!(selected.1, ChannelResolutionSource::Host);
    }

    #[tokio::test]
    async fn resolved_context_keeps_resolution_trace_for_runtime_diagnostics() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db.clone());

        let _default_channel_id = create_channel(&service, tenant_id, "default").await;
        let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
        add_web_target(&service, host_channel_id, "shop.example.test").await;

        let mut headers = HeaderMap::new();
        headers.insert(HOST, "shop.example.test".parse().expect("host header"));

        let resolver = ChannelResolver::new(db);
        let selected = resolved_detail_source_and_trace(
            resolver
                .resolve(&build_request_facts(
                    tenant_id,
                    &headers,
                    Some("channel=missing"),
                    None,
                    &test_settings(),
                ))
                .await
                .expect("resolution should succeed"),
        )
        .expect("host fallback should resolve");

        assert!(
            selected
                .2
                .iter()
                .any(|step| step.stage == ChannelResolutionStage::Query
                    && step.outcome == ChannelResolutionOutcome::Miss),
            "trace should preserve pre-host misses for runtime diagnostics"
        );
        assert!(
            selected
                .2
                .iter()
                .any(|step| step.stage == ChannelResolutionStage::Host
                    && step.outcome == ChannelResolutionOutcome::Matched),
            "trace should preserve the final match"
        );
    }
}
