use std::error::Error;

use async_graphql::{EmptySubscription, Request as GraphqlRequest, Schema, Variables};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use rustok_api::{
    Action, AuthContext, AuthContextExtension, HostRuntimeContext, Permission, Resource,
    TenantContext, TenantContextExtension,
};
use rustok_core::{PermissionScope, security_context_from_access_token};
use rustok_pages::{PagesMutation, PagesQuery};
use rustok_test_utils::mock_transactional_event_bus;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
type PagesSchema = Schema<PagesQuery, PagesMutation, EmptySubscription>;

const REBUILD_MUTATION: &str = r#"
mutation Rebuild($id: UUID!, $tenantId: UUID, $input: RebuildGqlPageArtifactInput!) {
  rebuildPageArtifact(id: $id, tenantId: $tenantId, input: $input) { operationId }
}
"#;

const ACTIVATE_MUTATION: &str = r#"
mutation Activate($id: UUID!, $tenantId: UUID, $input: ActivateGqlRebuiltPageArtifactInput!) {
  activateRebuiltPageArtifact(id: $id, tenantId: $tenantId, input: $input) { operationId }
}
"#;

fn pages_manage_permission() -> Permission {
    Permission::new(Resource::Pages, Action::Manage)
}

fn tenant(id: Uuid) -> TenantContext {
    TenantContext {
        id,
        name: "Pages repair request contract".to_string(),
        slug: format!("pages-repair-{id}"),
        domain: None,
        settings: json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    }
}

fn auth(tenant_id: Uuid, permissions: Vec<Permission>) -> AuthContext {
    AuthContext {
        user_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        tenant_id,
        permissions,
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    }
}

async fn setup_graphql_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    let database_url = format!(
        "sqlite:file:pages_repair_request_contract_{}?mode=memory&cache=shared",
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
        "CREATE TABLE tenant_modules (tenant_id TEXT NOT NULL, module_slug TEXT NOT NULL, enabled INTEGER NOT NULL)".to_string(),
    ))
    .await?;
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "INSERT INTO tenant_modules (tenant_id, module_slug, enabled) VALUES (?, ?, ?)",
        vec![tenant_id.into(), "pages".into(), true.into()],
    ))
    .await?;
    Ok(db)
}

fn graphql_schema(db: DatabaseConnection, tenant: TenantContext, auth: AuthContext) -> PagesSchema {
    Schema::build(
        PagesQuery::default(),
        PagesMutation::default(),
        EmptySubscription,
    )
    .data(db)
    .data(mock_transactional_event_bus())
    .data(tenant)
    .data(auth)
    .finish()
}

fn rebuild_variables(page_id: Uuid, tenant_id: Option<Uuid>) -> Variables {
    Variables::from_json(json!({
        "id": page_id.to_string(),
        "tenantId": tenant_id.map(|id| id.to_string()),
        "input": {
            "sourceId": Uuid::new_v4().to_string(),
            "expectedProvenanceHash": "0".repeat(64),
            "idempotencyKey": "request-contract-rebuild",
            "runtime": {
                "format": "request-contract-runtime",
                "scenarioId": "request-contract",
                "context": {},
                "reviewHash": "0".repeat(64)
            }
        }
    }))
}

fn activation_variables(page_id: Uuid, tenant_id: Option<Uuid>) -> Variables {
    Variables::from_json(json!({
        "id": page_id.to_string(),
        "tenantId": tenant_id.map(|id| id.to_string()),
        "input": {
            "rebuildOperationId": Uuid::new_v4().to_string(),
            "expectedVersion": 1,
            "expectedCurrentArtifactId": Uuid::new_v4().to_string(),
            "idempotencyKey": "request-contract-activate"
        }
    }))
}

fn graphql_error(response: async_graphql::Response) -> (String, String) {
    let value = serde_json::to_value(response).expect("GraphQL response must serialize");
    (
        value["errors"][0]["extensions"]["code"]
            .as_str()
            .expect("GraphQL error code must be a string")
            .to_string(),
        value["errors"][0]["message"]
            .as_str()
            .expect("GraphQL error message must be a string")
            .to_string(),
    )
}

fn http_request(
    path: String,
    tenant: TenantContext,
    auth: AuthContext,
    body: Value,
) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::to_vec(&body).expect("HTTP request body must serialize"),
        ))
        .expect("HTTP request must be valid");
    request
        .extensions_mut()
        .insert(TenantContextExtension(tenant));
    request.extensions_mut().insert(AuthContextExtension(auth));
    request
}

async fn http_error_body(response: axum::response::Response) -> TestResult<Value> {
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn rebuild_http_body() -> Value {
    json!({
        "source_id": Uuid::new_v4(),
        "expected_provenance_hash": "0".repeat(64),
        "idempotency_key": "request-contract-rebuild",
        "runtime": {
            "format": "request-contract-runtime",
            "scenario_id": "request-contract",
            "context": {},
            "review_hash": "0".repeat(64)
        }
    })
}

fn activation_http_body() -> Value {
    json!({
        "rebuild_operation_id": Uuid::new_v4(),
        "expected_version": 1,
        "expected_current_artifact_id": Uuid::new_v4(),
        "idempotency_key": "request-contract-activate"
    })
}

#[test]
fn pages_manage_transport_snapshot_is_all_or_none_never_own() {
    let actor = Uuid::new_v4();
    let manage = security_context_from_access_token(actor, "direct", &[pages_manage_permission()]);
    assert_eq!(
        manage.get_scope(Resource::Pages, Action::Manage),
        PermissionScope::All
    );

    let absent = security_context_from_access_token(actor, "direct", &[Permission::PAGES_UPDATE]);
    assert_eq!(
        absent.get_scope(Resource::Pages, Action::Manage),
        PermissionScope::None
    );
    assert_ne!(
        manage.get_scope(Resource::Pages, Action::Manage),
        PermissionScope::Own
    );
}

#[tokio::test]
async fn graphql_repair_requests_enforce_tenant_manage_and_static_validation() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_graphql_db(tenant_id).await?;
    let other_tenant = Uuid::new_v4();

    for (query, variables) in [
        (
            REBUILD_MUTATION,
            rebuild_variables(Uuid::new_v4(), Some(other_tenant)),
        ),
        (
            ACTIVATE_MUTATION,
            activation_variables(Uuid::new_v4(), Some(other_tenant)),
        ),
    ] {
        let schema = graphql_schema(
            db.clone(),
            tenant(tenant_id),
            auth(tenant_id, vec![pages_manage_permission()]),
        );
        let (code, message) = graphql_error(
            schema
                .execute(GraphqlRequest::new(query).variables(variables))
                .await,
        );
        assert_eq!(code, "PERMISSION_DENIED");
        assert_eq!(
            message,
            "Pages artifact repair mutations must use the current tenant"
        );
    }

    for (query, variables) in [
        (REBUILD_MUTATION, rebuild_variables(Uuid::new_v4(), None)),
        (
            ACTIVATE_MUTATION,
            activation_variables(Uuid::new_v4(), None),
        ),
    ] {
        let schema = graphql_schema(db.clone(), tenant(tenant_id), auth(tenant_id, Vec::new()));
        let (code, message) = graphql_error(
            schema
                .execute(GraphqlRequest::new(query).variables(variables))
                .await,
        );
        assert_eq!(code, "PERMISSION_DENIED");
        assert_eq!(message, "Permission denied: pages:manage required");
    }

    for (query, variables, expected_message) in [
        (
            REBUILD_MUTATION,
            rebuild_variables(Uuid::nil(), None),
            "Invalid immutable artifact rebuild input",
        ),
        (
            ACTIVATE_MUTATION,
            activation_variables(Uuid::nil(), None),
            "Invalid rebuilt artifact activation input",
        ),
    ] {
        let schema = graphql_schema(
            db.clone(),
            tenant(tenant_id),
            auth(tenant_id, vec![pages_manage_permission()]),
        );
        let (code, message) = graphql_error(
            schema
                .execute(GraphqlRequest::new(query).variables(variables))
                .await,
        );
        assert_eq!(code, "PAGE_ARTIFACT_REPAIR_INVALID_INPUT");
        assert_eq!(message, expected_message);
    }

    Ok(())
}

#[tokio::test]
async fn http_repair_requests_enforce_tenant_manage_and_static_validation() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_graphql_db(tenant_id).await?;
    let host = HostRuntimeContext::new(db).with_shared_value(mock_transactional_event_bus());
    let app = rustok_pages::http::axum_router(&host)?;

    for (path, body) in [
        (
            format!("/api/admin/pages/{}/artifacts/rebuild", Uuid::new_v4()),
            rebuild_http_body(),
        ),
        (
            format!("/api/admin/pages/{}/artifacts/activate", Uuid::new_v4()),
            activation_http_body(),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(http_request(
                path,
                tenant(tenant_id),
                auth(Uuid::new_v4(), vec![pages_manage_permission()]),
                body,
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            http_error_body(response).await?,
            json!({
                "code": "PAGES_PERMISSION_DENIED",
                "message": "Pages artifact repair routes must use the current tenant"
            })
        );
    }

    for (path, body) in [
        (
            format!("/api/admin/pages/{}/artifacts/rebuild", Uuid::new_v4()),
            rebuild_http_body(),
        ),
        (
            format!("/api/admin/pages/{}/artifacts/activate", Uuid::new_v4()),
            activation_http_body(),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(http_request(
                path,
                tenant(tenant_id),
                auth(tenant_id, Vec::new()),
                body,
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            http_error_body(response).await?,
            json!({
                "code": "PAGES_PERMISSION_DENIED",
                "message": "Permission denied: pages:manage required"
            })
        );
    }

    for (path, body, expected_message) in [
        (
            format!("/api/admin/pages/{}/artifacts/rebuild", Uuid::nil()),
            rebuild_http_body(),
            "Invalid immutable artifact rebuild input",
        ),
        (
            format!("/api/admin/pages/{}/artifacts/activate", Uuid::nil()),
            activation_http_body(),
            "Invalid rebuilt artifact activation input",
        ),
    ] {
        let response = app
            .clone()
            .oneshot(http_request(
                path,
                tenant(tenant_id),
                auth(tenant_id, vec![pages_manage_permission()]),
                body,
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            http_error_body(response).await?,
            json!({
                "code": "PAGE_ARTIFACT_REPAIR_INVALID_INPUT",
                "message": expected_message
            })
        );
    }

    Ok(())
}
