use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    response::IntoResponse,
};
use leptos::prelude::provide_context;
use rustok_api::{AuthContext, HostRuntimeContext, TenantContext};
use rustok_core::{MigrationSource, UserRole};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{
    ConnectionTrait, DatabaseConnection,
    sea_query::{Alias, Expr, Query},
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value, json};
use uuid::Uuid;

#[path = "../../../../tests/support/schema.rs"]
mod schema;

pub(super) struct Fixture {
    pub db: DatabaseConnection,
    pub bus: TransactionalEventBus,
    pub host: HostRuntimeContext,
    pub tenant: TenantContext,
    pub auth: AuthContext,
}

pub(super) async fn execute(
    host: &HostRuntimeContext,
    tenant: &TenantContext,
    auth: &AuthContext,
    path: &str,
    request: impl serde::Serialize,
) -> (StatusCode, Value) {
    let body = serde_json::to_vec(&request).unwrap();
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap();
    request
        .extensions_mut()
        .insert(rustok_api::context::TenantContextExtension(tenant.clone()));
    request
        .extensions_mut()
        .insert(rustok_api::context::AuthContextExtension(auth.clone()));
    request
        .extensions_mut()
        .insert(rustok_api::ResolvedRequestLocale {
            requested_locale: Some("fr".into()),
            effective_locale: "fr".into(),
        });
    let host = host.clone();
    let response =
        leptos_axum::handle_server_fns_with_context(move || provide_context(host.clone()), request)
            .await
            .into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"error": String::from_utf8_lossy(&bytes)})),
    )
}

pub(super) async fn fixture() -> Fixture {
    let db = rustok_test_utils::setup_test_db().await;
    schema::apply_blog_schema(&db, rustok_blog::BlogModule.migrations())
        .await
        .unwrap();
    rustok_modules::migrations::migrations()
        .into_iter()
        .find(|migration| migration.name() == "m20250101_000060_create_tenant_modules")
        .unwrap()
        .up(&SchemaManager::new(&db))
        .await
        .unwrap();
    let tenant = TenantContext {
        id: Uuid::new_v4(),
        name: "Native widgets".into(),
        slug: "native-widgets".into(),
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
    let host = HostRuntimeContext::new(db.clone()).with_shared_value(bus.clone());
    Fixture {
        db,
        bus,
        host,
        tenant,
        auth,
    }
}
