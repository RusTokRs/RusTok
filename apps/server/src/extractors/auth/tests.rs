use super::{
    AccessTokenSubjectKind, CurrentUser, classify_access_token_claims,
    resolve_current_user_from_access_token, resolve_service_token_permissions,
};
use crate::auth::{
    AuthConfig, AuthSettingsOverrides, Claims, encode_access_token, encode_oauth_access_token,
};
use crate::common::settings::RustokSettings;
use crate::models::{oauth_apps, sessions, tenants, users};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext};
use chrono::{Duration, Utc};
use rustok_api::Permission;
use rustok_core::{SecurityActorKind, UserRole, UserStatus};
use rustok_migrations::Migrator;
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, Schema, Set};
use sea_orm_migration::SchemaManager;
use std::str::FromStr;
use uuid::Uuid;

async fn ensure_oauth_apps_table(db: &DatabaseConnection) {
    let manager = SchemaManager::new(db);
    if manager
        .has_table("oauth_apps")
        .await
        .expect("check oauth_apps presence")
    {
        return;
    }

    let builder = db.get_database_backend();
    assert_eq!(builder, DbBackend::Sqlite, "expected sqlite test backend");
    let schema = Schema::new(builder);
    let mut statement = schema.create_table_from_entity(oauth_apps::Entity);
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("create oauth_apps table for auth extractor tests");
}

fn test_auth_config() -> AuthConfig {
    rustok_auth::build_auth_config(
        "test-secret-key-for-auth-extractor-32b".to_string(),
        900,
        AuthSettingsOverrides::default(),
    )
    .expect("auth config")
}

fn test_auth_runtime(db: DatabaseConnection) -> ServerAuthRuntime {
    let runtime_ctx = ServerRuntimeContext::new(db, RustokSettings::default());
    ServerAuthRuntime::new(runtime_ctx, test_auth_config())
}

fn claims(grant_type: &str, client_id: Option<Uuid>, session_id: Uuid) -> Claims {
    Claims {
        sub: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        role: UserRole::Customer,
        session_id,
        iss: "issuer".to_string(),
        aud: "audience".to_string(),
        exp: usize::MAX,
        iat: 0,
        client_id,
        scopes: Vec::new(),
        grant_type: grant_type.to_string(),
    }
}

async fn insert_user_with_session(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    status: UserStatus,
) -> (users::Model, Uuid) {
    let mut user = users::ActiveModel::new(
        tenant_id,
        &format!("{}@example.com", Uuid::new_v4()),
        "hash",
    );
    user.status = Set(status);
    let user = user.insert(db).await.expect("insert auth test user");

    let session_id = Uuid::new_v4();
    let mut session = sessions::ActiveModel::new(
        tenant_id,
        user.id,
        "refresh-token-hash".to_string(),
        Utc::now() + Duration::hours(1),
        None,
        None,
    );
    session.id = Set(session_id);
    session.insert(db).await.expect("insert auth test session");

    (user, session_id)
}

async fn insert_oauth_app(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    app_type: &str,
    grant_types: &[&str],
    is_active: bool,
) -> oauth_apps::Model {
    oauth_apps::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        name: Set("Auth extractor test app".to_string()),
        slug: Set(format!("auth-test-{}", Uuid::new_v4())),
        description: Set(Some("Auth extractor integration test".to_string())),
        app_type: Set(app_type.to_string()),
        icon_url: Set(None),
        client_id: Set(Uuid::new_v4()),
        client_secret_hash: Set(Some("hash".to_string())),
        redirect_uris: Set(serde_json::json!(["https://example.com/callback"])),
        scopes: Set(serde_json::json!(["forum:*"])),
        grant_types: Set(serde_json::json!(grant_types)),
        granted_permissions: Set(serde_json::json!(["forum_topics:list", "modules:list"])),
        manifest_ref: Set(None),
        auto_created: Set(false),
        is_active: Set(is_active),
        revoked_at: Set(None),
        last_used_at: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(db)
    .await
    .expect("insert oauth app")
}

#[test]
fn token_claim_classifier_separates_user_and_service_subjects() {
    let direct = claims("direct", None, Uuid::new_v4());
    let oauth_user = claims("authorization_code", Some(Uuid::new_v4()), Uuid::nil());
    let service = claims("client_credentials", Some(Uuid::new_v4()), Uuid::nil());

    assert_eq!(
        classify_access_token_claims(&direct).expect("direct token"),
        AccessTokenSubjectKind::User
    );
    assert_eq!(
        classify_access_token_claims(&oauth_user).expect("OAuth user token"),
        AccessTokenSubjectKind::User
    );
    assert_eq!(
        classify_access_token_claims(&service).expect("service token"),
        AccessTokenSubjectKind::Service
    );
}

#[test]
fn token_claim_classifier_rejects_ambiguous_subjects() {
    let ambiguous = claims("authorization_code", Some(Uuid::new_v4()), Uuid::new_v4());
    assert!(classify_access_token_claims(&ambiguous).is_err());
}

#[test]
fn service_current_user_builds_service_security_context() {
    let current = CurrentUser {
        user: users::Model::default_service_user(Uuid::new_v4(), Uuid::new_v4()),
        session_id: Uuid::nil(),
        permissions: vec![Permission::MODULES_LIST],
        inferred_role: UserRole::Customer,
        actor_kind: SecurityActorKind::Service,
        client_id: Some(Uuid::new_v4()),
        scopes: Vec::new(),
        grant_type: "client_credentials".to_string(),
    };

    assert_eq!(
        current.security_context().actor_kind,
        SecurityActorKind::Service
    );
}

#[tokio::test]
async fn oauth_service_token_intersects_app_permissions_with_scopes() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let tenant = tenants::ActiveModel::new("OAuth tenant", &format!("tenant-{}", Uuid::new_v4()))
        .insert(&db)
        .await
        .expect("create tenant");
    let app = insert_oauth_app(&db, tenant.id, "service", &["client_credentials"], true).await;

    let (permissions, inferred_role) = resolve_service_token_permissions(
        &db,
        tenant.id,
        app.id,
        app.client_id,
        UserRole::Customer,
        &["forum:*".to_string()],
    )
    .await
    .expect("resolve service token permissions");

    assert_eq!(
        permissions,
        vec![Permission::from_str("forum_topics:list").expect("forum permission")]
    );
    assert_eq!(inferred_role, UserRole::Customer);
}

#[tokio::test]
async fn oauth_service_token_with_empty_scopes_has_no_effective_permissions() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let tenant = tenants::ActiveModel::new(
        "Empty OAuth scope tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let app = insert_oauth_app(&db, tenant.id, "service", &["client_credentials"], true).await;

    let (permissions, _) = resolve_service_token_permissions(
        &db,
        tenant.id,
        app.id,
        app.client_id,
        UserRole::Customer,
        &[],
    )
    .await
    .expect("resolve service token permissions");

    assert!(permissions.is_empty());
}

#[tokio::test]
async fn oauth_service_token_rejects_subject_that_is_not_the_app_id() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let tenant = tenants::ActiveModel::new(
        "OAuth subject tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let app = insert_oauth_app(&db, tenant.id, "service", &["client_credentials"], true).await;

    let error = resolve_service_token_permissions(
        &db,
        tenant.id,
        Uuid::new_v4(),
        app.client_id,
        UserRole::Customer,
        &["forum:*".to_string()],
    )
    .await
    .expect_err("service token subject must equal app id");

    assert_eq!(
        error,
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "OAuth service token subject mismatch",
        )
    );
}

#[tokio::test]
async fn authorization_code_token_rejects_inactive_oauth_app() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let auth_runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new(
        "Inactive OAuth app tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let (user, _) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let app = insert_oauth_app(
        &db,
        tenant.id,
        "third_party",
        &["authorization_code"],
        false,
    )
    .await;
    let token = encode_oauth_access_token(
        &test_auth_config(),
        user.id,
        tenant.id,
        UserRole::Customer,
        app.client_id,
        &[],
        "authorization_code",
        900,
    )
    .expect("encode oauth access token");

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("inactive OAuth app must revoke authorization-code access");

    assert_eq!(
        error,
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "OAuth app not found or inactive",
        )
    );
}

#[tokio::test]
async fn authorization_code_token_rejects_scope_removed_from_oauth_app() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let auth_runtime = test_auth_runtime(db.clone());
    let tenant =
        tenants::ActiveModel::new("OAuth scope tenant", &format!("tenant-{}", Uuid::new_v4()))
            .insert(&db)
            .await
            .expect("create tenant");
    let (user, _) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let app = insert_oauth_app(&db, tenant.id, "third_party", &["authorization_code"], true).await;
    let removed_scopes = vec!["admin:*".to_string()];
    let token = encode_oauth_access_token(
        &test_auth_config(),
        user.id,
        tenant.id,
        UserRole::Customer,
        app.client_id,
        &removed_scopes,
        "authorization_code",
        900,
    )
    .expect("encode oauth access token");

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("token scope must remain allowed by the OAuth app");

    assert_eq!(
        error,
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "OAuth token scopes are no longer allowed",
        )
    );
}

#[tokio::test]
async fn direct_token_rejects_session_owned_by_another_subject() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let auth_runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new(
        "Session subject tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let (_, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let token = encode_access_token(
        &test_auth_config(),
        Uuid::new_v4(),
        tenant.id,
        UserRole::Customer,
        session_id,
    )
    .expect("encode mismatched access token");

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("session must be bound to token subject");

    assert_eq!(
        error,
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "Session subject mismatch",
        )
    );
}

#[tokio::test]
async fn access_token_resolver_returns_forbidden_for_inactive_user() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let auth_runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new(
        "Inactive user tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let (user, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Inactive).await;
    let token = encode_access_token(
        &test_auth_config(),
        user.id,
        tenant.id,
        UserRole::Customer,
        session_id,
    )
    .expect("encode access token");

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("inactive user should be rejected");

    assert_eq!(
        error,
        (axum::http::StatusCode::FORBIDDEN, "User is inactive")
    );
}

#[tokio::test]
async fn access_token_resolver_returns_internal_server_error_on_rbac_storage_failure() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let auth_runtime = test_auth_runtime(db.clone());
    let tenant =
        tenants::ActiveModel::new("RBAC failure tenant", &format!("tenant-{}", Uuid::new_v4()))
            .insert(&db)
            .await
            .expect("create tenant");
    let (user, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let token = encode_access_token(
        &test_auth_config(),
        user.id,
        tenant.id,
        UserRole::Customer,
        session_id,
    )
    .expect("encode access token");
    RbacService::invalidate_user_rbac_caches(&tenant.id, &user.id).await;
    db.execute(sea_orm::Statement::from_string(
        sea_orm::DatabaseBackend::Sqlite,
        "DROP TABLE user_roles".to_string(),
    ))
    .await
    .expect("drop user_roles to simulate RBAC storage failure");

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("RBAC storage failure should surface as 500");

    assert_eq!(
        error,
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Database error",
        )
    );
}
