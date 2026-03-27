use axum::{
    body::Body,
    extract::State,
    http::{header::HOST, HeaderMap, Request},
    middleware::Next,
    response::Response,
};
use loco_rs::app::AppContext;
use uuid::Uuid;

use crate::context::{
    ChannelContext, ChannelContextExtension, ChannelResolutionSource, TenantContextExt,
};
use rustok_channel::{ChannelDetailResponse, ChannelService};

const CHANNEL_ID_HEADER: &str = "X-Channel-ID";
const CHANNEL_SLUG_HEADER: &str = "X-Channel-Slug";

pub async fn resolve(
    State(ctx): State<AppContext>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, axum::http::StatusCode> {
    let Some(tenant) = req.extensions().tenant_context().cloned() else {
        return Ok(next.run(req).await);
    };

    let service = ChannelService::new(ctx.db.clone());
    let selected = select_channel(&service, tenant.id, req.headers(), req.uri().query()).await;

    if let Some(ResolvedChannelDetail { detail, source }) = selected {
        let selected_target = detail
            .targets
            .iter()
            .find(|target| target.is_primary)
            .or_else(|| detail.targets.first());
        req.extensions_mut()
            .insert(ChannelContextExtension(ChannelContext {
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
            }));
    }

    Ok(next.run(req).await)
}

struct ResolvedChannelDetail {
    detail: ChannelDetailResponse,
    source: ChannelResolutionSource,
}

async fn select_channel(
    service: &ChannelService,
    tenant_id: Uuid,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Option<ResolvedChannelDetail> {
    if let Some(channel_id) = channel_id_from_header(headers) {
        if let Some(selected) = resolve_channel_from_id(service, tenant_id, channel_id).await {
            return Some(selected);
        }
    }

    if let Some(slug) = channel_slug_from_header(headers) {
        if let Some(selected) = resolve_channel_from_slug(
            service,
            tenant_id,
            &slug,
            ChannelResolutionSource::HeaderSlug,
        )
        .await
        {
            return Some(selected);
        }
    }

    if let Some(slug) = channel_slug_from_query(query) {
        if let Some(selected) =
            resolve_channel_from_slug(service, tenant_id, &slug, ChannelResolutionSource::Query)
                .await
        {
            return Some(selected);
        }
    }

    if let Some(host) = extract_host(headers) {
        if let Some(selected) = resolve_channel_from_host(service, tenant_id, host).await {
            return Some(selected);
        }
    }

    resolve_default_channel(service, tenant_id).await
}

async fn resolve_channel_from_id(
    service: &ChannelService,
    tenant_id: Uuid,
    channel_id: Uuid,
) -> Option<ResolvedChannelDetail> {
    match service.get_channel_detail(channel_id).await {
        Ok(detail) if detail.channel.tenant_id == tenant_id && detail.channel.is_active => {
            Some(ResolvedChannelDetail {
                detail,
                source: ChannelResolutionSource::HeaderId,
            })
        }
        _ => None,
    }
}

async fn resolve_channel_from_slug(
    service: &ChannelService,
    tenant_id: Uuid,
    slug: &str,
    source: ChannelResolutionSource,
) -> Option<ResolvedChannelDetail> {
    match service.get_channel_detail_by_slug(tenant_id, slug).await {
        Ok(Some(detail)) if detail.channel.is_active => {
            Some(ResolvedChannelDetail { detail, source })
        }
        _ => None,
    }
}

async fn resolve_channel_from_host(
    service: &ChannelService,
    tenant_id: Uuid,
    host: &str,
) -> Option<ResolvedChannelDetail> {
    let normalized = host
        .split(':')
        .next()
        .unwrap_or(host)
        .trim()
        .to_ascii_lowercase();
    service
        .get_channel_by_host_target_value(tenant_id, &normalized)
        .await
        .ok()
        .flatten()
        .map(|detail| ResolvedChannelDetail {
            detail,
            source: ChannelResolutionSource::Host,
        })
}

async fn resolve_default_channel(
    service: &ChannelService,
    tenant_id: Uuid,
) -> Option<ResolvedChannelDetail> {
    service
        .get_default_channel(tenant_id)
        .await
        .ok()
        .flatten()
        .map(|detail| ResolvedChannelDetail {
            detail,
            source: ChannelResolutionSource::Default,
        })
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

fn extract_host(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get("x-forwarded-host")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .or_else(|| headers.get(HOST).and_then(|value| value.to_str().ok()))
}

#[cfg(test)]
mod tests {
    use super::{
        channel_id_from_header, channel_slug_from_header, channel_slug_from_query, select_channel,
    };
    use crate::context::ChannelResolutionSource;
    use axum::http::{header::HOST, HeaderMap};
    use rustok_channel::{
        migrations, ChannelService, CreateChannelInput, CreateChannelTargetInput,
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
        let service = ChannelService::new(db);

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

        let selected = select_channel(&service, tenant_id, &headers, Some("channel=query-channel"))
            .await
            .expect("channel should be resolved");

        assert_eq!(selected.detail.channel.id, header_id_channel_id);
        assert_eq!(selected.detail.channel.slug, "header-id");
        assert_eq!(selected.source, ChannelResolutionSource::HeaderId);
    }

    #[tokio::test]
    async fn select_channel_falls_back_from_missing_query_to_host() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);

        let _default_channel_id = create_channel(&service, tenant_id, "default").await;
        let host_channel_id = create_channel(&service, tenant_id, "host-channel").await;
        add_web_target(&service, host_channel_id, "shop.example.test").await;

        let mut headers = HeaderMap::new();
        headers.insert(HOST, "shop.example.test".parse().expect("host header"));

        let selected = select_channel(&service, tenant_id, &headers, Some("channel=missing"))
            .await
            .expect("host fallback should resolve");

        assert_eq!(selected.detail.channel.id, host_channel_id);
        assert_eq!(selected.detail.channel.slug, "host-channel");
        assert_eq!(selected.source, ChannelResolutionSource::Host);
        assert_eq!(selected.detail.targets.len(), 1);
        assert_eq!(selected.detail.targets[0].value, "shop.example.test");
    }

    #[tokio::test]
    async fn select_channel_falls_back_to_default_when_no_selector_matches() {
        let db = setup_channel_db().await;
        let tenant_id = Uuid::new_v4();
        seed_tenant(&db, tenant_id, "tenant").await;
        let service = ChannelService::new(db);

        let first_channel_id = create_channel(&service, tenant_id, "default").await;
        let explicit_default_channel_id = create_channel(&service, tenant_id, "secondary").await;
        service
            .set_default_channel(explicit_default_channel_id)
            .await
            .expect("explicit default channel should be saved");

        let headers = HeaderMap::new();
        let selected = select_channel(&service, tenant_id, &headers, Some("channel=missing"))
            .await
            .expect("default fallback should resolve");

        assert_ne!(selected.detail.channel.id, first_channel_id);
        assert_eq!(selected.detail.channel.id, explicit_default_channel_id);
        assert_eq!(selected.detail.channel.slug, "secondary");
        assert_eq!(selected.source, ChannelResolutionSource::Default);
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
        headers.insert(HOST, "shop.example.test".parse().expect("host header"));

        let selected = select_channel(&service, tenant_id, &headers, None)
            .await
            .expect("inactive channel must be skipped");

        assert_eq!(selected.detail.channel.id, host_channel_id);
        assert_eq!(selected.detail.channel.slug, "host-channel");
        assert_eq!(selected.source, ChannelResolutionSource::Host);
    }
}
