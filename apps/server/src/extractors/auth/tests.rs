use super::{
    classify_access_token_claims, resolve_current_user_from_access_token,
    resolve_service_token_permissions, AccessTokenSubjectKind, CurrentUser,
};
use crate::auth::{encode_access_token, AuthConfig, AuthSettingsOverrides, Claims};
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
    let ambiguous = claims(
        "authorization_code",
        Some(Uuid::new_v4()),
        Uuid::new_v4(),
    );
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
async fn oauth_service_token_resolves_granted_permissions_from_oauth_app() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let tenant = tenants::ActiveModel::new(
        "OAuth tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");

    let created = oauth_apps::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant.id),
        name: Set("Forum Bot".to_string()),
        slug: Set("forum-bot".to_string()),
        description: Set(Some("Service integration".to_string())),
        app_type: Set("service".to_string()),
        icon_url: Set(None),
        client_id: Set(Uuid::new_v4()),
        client_secret_hash: Set(Some("hash".to_string())),
        redirect_uris: Set(serde_json::json!([])),
        scopes: Set(serde_json::json!(["forum:*"])),
        grant_types: Set(serde_json::json!(["client_credentials"])),
        granted_permissions: Set(serde_json::json!(["forum_topics:list", "modules:list"])),
        manifest_ref: Set(None),
        auto_created: Set(false),
        is_active: Set(true),
        revoked_at: Set(None),
        last_used_at: Set(None),
        metadata: Set(serde_json::json!({})),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    }
    .insert(&db)
    .await
    .expect("insert oauth app");

    let expected_permissions = vec![
        Permission::from_str("forum_topics:list").expect("forum permission"),
        Permission::from_str("modules:list").expect("modules permission"),
    ];
    let (permissions, inferred_role) = resolve_service_token_permissions(
        &db,
        tenant.id,
        created.client_id,
        rustok_core::UserRole::Customer,
    )
    .await
    .expect("resolve service token permissions");

    assert_eq!(permissions, expected_permissions);
    assert_eq!(
        inferred_role,
        crate::context::infer_user_role_from_permissions(&permissions)
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
    let (user, session_id) =
        insert_user_with_session(&db, tenant.id, UserStatus::Inactive).await;
    let auth_config = test_auth_config();
    let token = encode_access_token(
        &auth_config,
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
    let tenant = tenants::ActiveModel::new(
        "RBAC failure tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let (user, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let auth_config = test_auth_config();
    let token = encode_access_token(
        &auth_config,
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
