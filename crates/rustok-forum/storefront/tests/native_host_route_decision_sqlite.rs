#![cfg(feature = "ssr")]

use std::error::Error;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode, header};
use axum::routing::post;
use leptos::prelude::provide_context;
use leptos_axum::handle_server_fns_with_context;
use rustok_api::{HostRuntimeContext, TenantContext, TenantContextExtension};
use rustok_core::{MemoryTransport, MigrationSource, SecurityContext, UserRole};
use rustok_forum::{
    CategoryService, CreateCategoryInput, CreateTopicInput, ForumModule, ForumTopicRouteService,
    RenameForumTopicSlugInput, TopicService, UpdateCategoryInput,
};
use rustok_forum_storefront as _;
use rustok_outbox::TransactionalEventBus;
use rustok_taxonomy::TaxonomyModule;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::SchemaManager;
use tower::ServiceExt;
use uuid::Uuid;

const BODY_LIMIT: usize = 1024 * 1024;
const CATEGORY_ENDPOINT: &str = "/api/fn/forum/storefront-category-route";
const TOPIC_ENDPOINT: &str = "/api/fn/forum/storefront-topic-route";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn registered_native_host_resolves_forum_canonical_alias_and_missing_routes() -> TestResult<()>
{
    let tenant_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    let db = setup_db(tenant_id, admin_id).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(MemoryTransport::new()));
    let security = SecurityContext::new(UserRole::Admin, Some(admin_id));

    let category_service = CategoryService::new(db.clone());
    let category_id = category_service
        .create(
            tenant_id,
            security.clone(),
            CreateCategoryInput {
                locale: "en".to_string(),
                name: "Platform".to_string(),
                slug: "platform".to_string(),
                description: Some("Registered native host evidence".to_string()),
                icon: None,
                color: None,
                parent_id: None,
                position: Some(0),
                moderated: false,
            },
        )
        .await?
        .id;

    let topic_service = TopicService::new(db.clone(), event_bus.clone());
    let topic_id = topic_service
        .create(
            tenant_id,
            security.clone(),
            CreateTopicInput {
                locale: "en".to_string(),
                category_id,
                title: "Native host route".to_string(),
                slug: Some("native-host-route".to_string()),
                body: rustok_api::RichTextDocument::single_paragraph(
                    "Registered Forum native host route evidence",
                ),
                metadata: serde_json::json!({}),
                tags: Vec::new(),
                channel_slugs: None,
            },
        )
        .await?
        .id;

    category_service
        .update(
            tenant_id,
            category_id,
            security.clone(),
            UpdateCategoryInput {
                locale: "en".to_string(),
                slug: Some("platform-engineering".to_string()),
                ..UpdateCategoryInput::default()
            },
        )
        .await?;
    topic_service
        .rename_slug(
            tenant_id,
            topic_id,
            security,
            RenameForumTopicSlugInput {
                locale: "en".to_string(),
                slug: "registered-native-host".to_string(),
            },
        )
        .await?;

    let host = HostRuntimeContext::new(db).with_shared_value(event_bus);
    let app = native_server_fn_router(host);
    let tenant = tenant_context(tenant_id);
    let short_id = ForumTopicRouteService::short_identity(topic_id);

    let category_canonical = call(
        &app,
        &tenant,
        CATEGORY_ENDPOINT,
        "locale=en&slug=platform-engineering",
    )
    .await?;
    assert_ok_contains(
        &category_canonical,
        &["canonical", "/en/forum/c/platform-engineering"],
    );

    let category_alias = call(&app, &tenant, CATEGORY_ENDPOINT, "locale=en&slug=platform").await?;
    assert_ok_contains(
        &category_alias,
        &["redirect", "/en/forum/c/platform-engineering"],
    );

    let topic_canonical = call(
        &app,
        &tenant,
        TOPIC_ENDPOINT,
        &format!("locale=en&short_id={short_id}&slug=registered-native-host"),
    )
    .await?;
    assert_ok_contains(
        &topic_canonical,
        &[
            "canonical",
            &format!("/en/forum/t/{short_id}/registered-native-host"),
        ],
    );

    let topic_alias = call(
        &app,
        &tenant,
        TOPIC_ENDPOINT,
        &format!("locale=en&short_id={short_id}&slug=native-host-route"),
    )
    .await?;
    assert_ok_contains(
        &topic_alias,
        &[
            "redirect",
            &format!("/en/forum/t/{short_id}/registered-native-host"),
        ],
    );

    let missing_category = call(
        &app,
        &tenant,
        CATEGORY_ENDPOINT,
        "locale=en&slug=missing-category",
    )
    .await?;
    assert_absent(&missing_category);

    let missing_topic = call(
        &app,
        &tenant,
        TOPIC_ENDPOINT,
        "locale=en&short_id=000000000000&slug=missing-topic",
    )
    .await?;
    assert_absent(&missing_topic);

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

async fn call(
    app: &Router,
    tenant: &TenantContext,
    endpoint: &str,
    form: &str,
) -> TestResult<ServerFnResponse> {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(endpoint)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("X-Tenant-ID", tenant.id.to_string())
        .body(Body::from(form.to_string()))?;
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant.clone()));

    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let bytes = to_bytes(response.into_body(), BODY_LIMIT).await?;
    Ok(ServerFnResponse {
        status,
        body: std::str::from_utf8(&bytes)?.to_string(),
    })
}

fn assert_ok_contains(response: &ServerFnResponse, markers: &[&str]) {
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    for marker in markers {
        assert!(
            response.body.contains(marker),
            "response did not contain {marker:?}: {}",
            response.body
        );
    }
}

fn assert_absent(response: &ServerFnResponse) {
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert!(response.body.contains("null"), "{}", response.body);
    assert!(!response.body.contains("canonical"), "{}", response.body);
    assert!(!response.body.contains("redirect"), "{}", response.body);
}

async fn setup_db(tenant_id: Uuid, admin_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:forum_native_host_route_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    );
    let mut options = ConnectOptions::new(database_url);
    options
        .max_connections(5)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_unprepared(
        "CREATE TABLE users (id TEXT NOT NULL PRIMARY KEY, tenant_id TEXT NOT NULL)",
    )
    .await?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO users (id, tenant_id) VALUES (?1, ?2)",
        vec![admin_id.into(), tenant_id.into()],
    ))
    .await?;

    let manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration.up(&manager).await?;
    }
    for migration in ForumModule.migrations() {
        migration.up(&manager).await?;
    }
    Ok(db)
}

fn tenant_context(tenant_id: Uuid) -> TenantContext {
    TenantContext {
        id: tenant_id,
        name: "Forum native host tenant".to_string(),
        slug: "forum-native-host".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}
