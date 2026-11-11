//! `GET /api/pages` applies the visitor rule of the GraphQL page read.
//!
//! A token without a page authoring permission reads as a visitor of the request channel: the
//! page channel list applies to it, exactly as to an anonymous GraphQL reader. A page author
//! resolves the page on any channel.

use std::error::Error;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use chrono::Utc;
use rustok_api::{
    AuthContext, AuthContextExtension, HostRuntimeContext, Permission, ResolvedRequestLocale,
    TenantContext, TenantContextExtension,
};
use rustok_core::MigrationSource;
use rustok_pages::entities::{page, page_channel_visibility, page_translation};
use rustok_pages::{PagesModule, controllers};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, Set};
use sea_orm_migration::SchemaManager;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

const RESPONSE_BODY_LIMIT: usize = 1024 * 1024;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn page_http_read_applies_the_channel_list_to_tokens_without_authoring_authority()
-> TestResult<()> {
    let db = setup_db().await?;
    let tenant_id = Uuid::new_v4();
    let restricted = insert_published_page(&db, tenant_id, "restricted", &["web"]).await?;
    let open = insert_published_page(&db, tenant_id, "open", &[]).await?;
    let host =
        HostRuntimeContext::new(db.clone()).with_shared_value(mock_transactional_event_bus());
    let app = controllers::axum_router(&host)?;

    // The request carries no channel, so a visitor sees only pages without a channel list.
    let visitor = vec![Permission::PAGES_READ, Permission::PAGES_LIST];
    let hidden = app
        .clone()
        .oneshot(page_request(tenant_id, "restricted", visitor.clone()))
        .await?;
    assert_eq!(hidden.status(), StatusCode::NOT_FOUND);

    let visible = app
        .clone()
        .oneshot(page_request(tenant_id, "open", visitor))
        .await?;
    assert_eq!(visible.status(), StatusCode::OK);
    assert_eq!(response_json(visible).await?["id"], json!(open.to_string()));

    // A page author resolves the page whatever the request channel is.
    let author = vec![Permission::PAGES_READ, Permission::PAGES_UPDATE];
    let authored = app
        .oneshot(page_request(tenant_id, "restricted", author))
        .await?;
    assert_eq!(authored.status(), StatusCode::OK);
    assert_eq!(
        response_json(authored).await?["id"],
        json!(restricted.to_string())
    );
    Ok(())
}

async fn response_json(response: axum::response::Response) -> TestResult<Value> {
    let bytes = to_bytes(response.into_body(), RESPONSE_BODY_LIMIT).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn page_request(tenant_id: Uuid, slug: &str, permissions: Vec<Permission>) -> Request<Body> {
    let mut request = Request::builder()
        .uri(format!("/api/pages?slug={slug}&locale=en"))
        .header("X-Tenant-ID", tenant_id.to_string())
        .body(Body::empty())
        .expect("page HTTP request must be valid");
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant_context(tenant_id)));
    request.extensions_mut().insert(ResolvedRequestLocale {
        requested_locale: Some("en".to_string()),
        effective_locale: "en".to_string(),
    });
    request
        .extensions_mut()
        .insert(AuthContextExtension(AuthContext {
            user_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
        }));
    request
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Page HTTP tenant".to_string(),
        slug: "page-http-tenant".to_string(),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

async fn insert_published_page(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
    channel_slugs: &[&str],
) -> TestResult<Uuid> {
    let page_id = Uuid::new_v4();
    let now: sea_orm::prelude::DateTimeWithTimeZone = Utc::now().into();
    page::ActiveModel {
        id: Set(page_id),
        tenant_id: Set(tenant_id),
        author_id: Set(None),
        status: Set("published".to_string()),
        template: Set("default".to_string()),
        metadata: Set(json!({})),
        created_at: Set(now),
        updated_at: Set(now),
        published_at: Set(Some(now)),
        archived_at: Set(None),
        version: Set(1),
    }
    .insert(db)
    .await?;
    page_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        page_id: Set(page_id),
        tenant_id: Set(tenant_id),
        locale: Set("en".to_string()),
        title: Set(format!("Page {slug}")),
        slug: Set(slug.to_string()),
        meta_title: Set(None),
        meta_description: Set(None),
        revision: Set(1),
    }
    .insert(db)
    .await?;
    for channel_slug in channel_slugs {
        page_channel_visibility::ActiveModel {
            id: Set(Uuid::new_v4()),
            page_id: Set(page_id),
            tenant_id: Set(tenant_id),
            channel_slug: Set((*channel_slug).to_string()),
            created_at: Set(now),
        }
        .insert(db)
        .await?;
    }
    Ok(page_id)
}

async fn setup_db() -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_http_visitor_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;
    let manager = SchemaManager::new(&db);
    for migration in PagesModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}
