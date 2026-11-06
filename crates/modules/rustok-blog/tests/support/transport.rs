// Migration-backed authenticated transport fixture shared by owner API scenarios.
use crate::support::apply_blog_schema;
use axum::{Extension, Router, body::Body, http::StatusCode};
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_core::UserRole;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ConnectionTrait, DatabaseConnection,
    sea_query::{Alias, Expr, Query},
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;

pub type Fixture = (
    DatabaseConnection,
    TransactionalEventBus,
    TenantContext,
    AuthContext,
);

pub async fn fixture() -> Fixture {
    fixture_on(rustok_test_utils::setup_test_db().await).await
}

pub async fn fixture_on(db: DatabaseConnection) -> Fixture {
    apply_blog_schema(&db).await.unwrap();
    rustok_modules::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20250101_000060_create_tenant_modules")
        .unwrap()
        .up(&SchemaManager::new(&db))
        .await
        .unwrap();
    let tenant = TenantContext {
        id: Uuid::new_v4(),
        name: "Transport fixture".into(),
        slug: "transport-fixture".into(),
        domain: None,
        settings: json!({}),
        default_locale: "en".into(),
        is_active: true,
    };
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new("tenants"))
        .columns([Alias::new("id"), Alias::new("name"), Alias::new("slug")])
        .values_panic([
            Expr::value(tenant.id),
            Expr::value(&tenant.name),
            Expr::value(&tenant.slug),
        ]);
    db.execute_raw(db.get_database_backend().build(&insert))
        .await
        .unwrap();
    let mut insert = Query::insert();
    insert
        .into_table(Alias::new("tenant_modules"))
        .columns([
            Alias::new("id"),
            Alias::new("tenant_id"),
            Alias::new("module_slug"),
            Alias::new("enabled"),
        ])
        .values_panic([
            Expr::value(Uuid::new_v4()),
            Expr::value(tenant.id),
            Expr::value("blog"),
            Expr::value(true),
        ]);
    db.execute_raw(db.get_database_backend().build(&insert))
        .await
        .unwrap();
    let auth = AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id: tenant.id,
        permissions: rustok_core::Rbac::permissions_for_role(&UserRole::Admin)
            .iter()
            .copied()
            .collect(),
        client_id: None,
        scopes: vec![],
        grant_type: "direct".into(),
    };
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    (db, bus, tenant, auth)
}

pub fn request_context(tenant: &TenantContext, auth: &AuthContext) -> RequestContext {
    RequestContext {
        tenant_id: tenant.id,
        user_id: Some(auth.user_id),
        channel_id: None,
        channel_slug: None,
        channel_resolution_source: None,
        locale: "fr".into(),
        correlation_id: Uuid::new_v4().to_string(),
    }
}

struct UnusedCategoryCleanup;

#[async_trait::async_trait]
impl rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort for UnusedCategoryCleanup {
    async fn cleanup_in_tx(
        &self,
        _txn: &sea_orm::DatabaseTransaction,
        _tenant: Uuid,
        _category: Uuid,
    ) -> rustok_taxonomy::TaxonomyResult<()> {
        Err(rustok_taxonomy::TaxonomyError::validation(
            "This read/create transport fixture must never delete a category",
        ))
    }
}

pub fn router(
    db: DatabaseConnection,
    bus: TransactionalEventBus,
    tenant: TenantContext,
    auth: Option<AuthContext>,
) -> Router {
    let cleanup: Arc<dyn rustok_taxonomy::TaxonomyCategoryDeleteCleanupPort> =
        Arc::new(UnusedCategoryCleanup);
    let host = rustok_api::HostRuntimeContext::new(db)
        .with_shared_value(bus)
        .with_shared_value(cleanup);
    let router = rustok_blog::controllers::axum_router(&host)
        .unwrap()
        .layer(Extension(rustok_api::context::TenantContextExtension(
            tenant,
        )))
        .layer(Extension(rustok_api::ResolvedRequestLocale {
            requested_locale: Some("fr".into()),
            effective_locale: "fr".into(),
        }));
    match auth {
        Some(auth) => router.layer(Extension(rustok_api::context::AuthContextExtension(auth))),
        None => router,
    }
}

pub async fn http(router: Router, method: &str, path: &str, body: Value) -> (StatusCode, Value) {
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .method(method)
                .uri(path)
                .header("content-type", "application/json")
                .body(if method == "GET" {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let json = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"raw_body": String::from_utf8_lossy(&bytes)}));
    (status, json)
}
