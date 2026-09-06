#![cfg(feature = "ssr")]

use std::error::Error;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::routing::post;
use chrono::Utc;
use leptos::prelude::provide_context;
use leptos_axum::handle_server_fns_with_context;
use rustok_api::{
    ChannelContext, ChannelContextExtension, ChannelResolutionSource, HostRuntimeContext,
    TenantContext, TenantContextExtension,
};
use rustok_channel::{
    BindChannelModuleInput, ChannelModule, ChannelResponse, ChannelService, CreateChannelInput,
};
use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, SysEventsMigration, TransactionalEventBus};
use rustok_pages::dto::{
    CreatePageInput, PageBodyInput, PageTranslationInput, PatchPageMetadataInput,
};
use rustok_pages::entities::page_route_alias;
use rustok_pages::{PageService, PagesModule};
use rustok_pages_storefront as _;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectOptions, ConnectionTrait, Database,
    DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tower::ServiceExt;
use uuid::Uuid;

const RESPONSE_BODY_LIMIT: usize = 1024 * 1024;
const SERVER_FN_PATH: &str = "/api/fn/pages/route-decision";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn registered_host_route_decision_respects_admission_aliases_and_terminal_states()
-> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let page_id = create_and_rename_published_page(&db, event_bus.clone(), tenant_id).await?;

    let channel_service = ChannelService::new(db.clone());
    let channel = channel_service
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: "web".to_string(),
            name: "Web".to_string(),
            settings: None,
        })
        .await?;
    set_pages_enabled(&channel_service, channel.id, true).await?;

    let host = HostRuntimeContext::new(db.clone()).with_shared_value(event_bus);
    let app = native_server_fn_router(host);
    let tenant = tenant_context(tenant_id);
    let channel_context = channel_context(&channel);

    let first_alias = call_route(&app, &tenant, &channel_context, "about", "en").await?;
    assert_eq!(first_alias.status, StatusCode::OK);
    assert!(first_alias.body.contains("redirect"));
    assert!(first_alias.body.contains("/en/modules/pages?slug=company"));

    let second_alias = call_route(&app, &tenant, &channel_context, "about-us", "en").await?;
    assert_eq!(second_alias.status, StatusCode::OK);
    assert!(second_alias.body.contains("redirect"));
    assert!(second_alias.body.contains("company"));

    let canonical = call_route(&app, &tenant, &channel_context, "company", "en").await?;
    assert_eq!(canonical.status, StatusCode::OK);
    assert!(canonical.body.contains("canonical"));
    assert!(canonical.body.contains(&page_id.to_string()));

    let missing = call_route(&app, &tenant, &channel_context, "missing", "en").await?;
    assert_eq!(missing.status, StatusCode::OK);
    assert!(missing.body.contains("not_found"));

    insert_route_alias(&db, tenant_id, page_id, "removed", "gone", None, None).await?;
    let gone = call_route(&app, &tenant, &channel_context, "removed", "en").await?;
    assert_eq!(gone.status, StatusCode::OK);
    assert!(gone.body.contains("gone"));

    insert_route_alias(
        &db,
        tenant_id,
        page_id,
        "company",
        "redirect",
        Some(page_id),
        Some("en"),
    )
    .await?;
    let conflict = call_route(&app, &tenant, &channel_context, "company", "en").await?;
    assert_eq!(conflict.status, StatusCode::OK);
    assert!(conflict.body.contains("conflict"));

    set_pages_enabled(&channel_service, channel.id, false).await?;
    let denied_alias = call_route(&app, &tenant, &channel_context, "about", "en").await?;
    assert_eq!(denied_alias.status, StatusCode::OK);
    assert!(denied_alias.body.contains("not_found"));
    assert!(!denied_alias.body.contains("redirect"));

    Ok(())
}

fn native_server_fn_router(host: HostRuntimeContext) -> Router {
    Router::new().route(
        "/api/fn/{*fn_name}",
        post(move |request| {
            let host = host.clone();
            async move {
                handle_server_fns_with_context(
                    move || {
                        provide_context(host.clone());
                    },
                    request,
                )
                .await
            }
        }),
    )
}

struct ServerFnResponse {
    status: StatusCode,
    body: String,
}

async fn call_route(
    app: &Router,
    tenant: &TenantContext,
    channel: &ChannelContext,
    page_slug: &str,
    locale: &str,
) -> TestResult<ServerFnResponse> {
    let form = format!("page_slug={page_slug}&locale={locale}");
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(SERVER_FN_PATH)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("X-Tenant-ID", tenant.id.to_string())
        .body(Body::from(form))?;
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant.clone()));
    request
        .extensions_mut()
        .insert(ChannelContextExtension(channel.clone()));

    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), RESPONSE_BODY_LIMIT).await?;
    Ok(ServerFnResponse {
        status,
        body: std::str::from_utf8(&bytes)?.to_string(),
    })
}

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_host_route_decision_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_raw(Statement::from_string(
        DbBackend::Sqlite,
        "CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenants (id) VALUES (?)",
        [tenant_id.into()],
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    SysEventsMigration.up(&manager).await?;
    for migration in ChannelModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

async fn create_and_rename_published_page(
    db: &DatabaseConnection,
    event_bus: TransactionalEventBus,
    tenant_id: Uuid,
) -> TestResult<Uuid> {
    let service = PageService::new(db.clone(), event_bus);
    let draft = service
        .create(
            tenant_id,
            SecurityContext::system(),
            CreatePageInput {
                translations: vec![translation("About", "about")],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: serde_json::json!({
                        "pages": [],
                        "test_content": "host-route-source",
                    }),
                }),
                channel_slugs: Some(vec!["web".to_string()]),
                publish: false,
            },
        )
        .await?;
    let published = service
        .publish_non_builder_if_current(
            tenant_id,
            SecurityContext::system(),
            draft.id,
            Some(draft.version),
        )
        .await?;
    let renamed_once = service
        .patch_metadata(
            tenant_id,
            SecurityContext::system(),
            published.id,
            PatchPageMetadataInput {
                expected_version: published.version,
                translations: Some(vec![translation("About us", "about-us")]),
                template: None,
                channel_slugs: None,
            },
        )
        .await?;
    let renamed_twice = service
        .patch_metadata(
            tenant_id,
            SecurityContext::system(),
            renamed_once.id,
            PatchPageMetadataInput {
                expected_version: renamed_once.version,
                translations: Some(vec![translation("Company", "company")]),
                template: None,
                channel_slugs: None,
            },
        )
        .await?;
    Ok(renamed_twice.id)
}

fn translation(title: &str, slug: &str) -> PageTranslationInput {
    PageTranslationInput {
        locale: "en".to_string(),
        title: title.to_string(),
        slug: Some(slug.to_string()),
        meta_title: None,
        meta_description: None,
    }
}

async fn insert_route_alias(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
    slug: &str,
    disposition: &str,
    target_page_id: Option<Uuid>,
    target_locale: Option<&str>,
) -> TestResult<()> {
    page_route_alias::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        page_id: Set(page_id),
        locale: Set("en".to_string()),
        slug: Set(slug.to_string()),
        disposition: Set(disposition.to_string()),
        target_page_id: Set(target_page_id),
        target_locale: Set(target_locale.map(str::to_string)),
        reason: Set("Host route source fixture".to_string()),
        created_at: Set(Utc::now().into()),
    }
    .insert(db)
    .await?;
    Ok(())
}

async fn set_pages_enabled(
    service: &ChannelService,
    channel_id: Uuid,
    is_enabled: bool,
) -> TestResult<()> {
    service
        .bind_module(
            channel_id,
            BindChannelModuleInput {
                module_slug: "pages".to_string(),
                is_enabled,
                settings: None,
            },
        )
        .await?;
    Ok(())
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Host route tenant".to_string(),
        slug: "host-route-tenant".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn channel_context(channel: &ChannelResponse) -> ChannelContext {
    ChannelContext {
        id: channel.id,
        tenant_id: channel.tenant_id,
        slug: channel.slug.clone(),
        name: channel.name.clone(),
        is_active: channel.is_active,
        status: channel.status.clone(),
        target_type: None,
        target_value: None,
        settings: channel.settings.clone(),
        resolution_source: ChannelResolutionSource::HeaderSlug,
        resolution_trace: Vec::new(),
    }
}
