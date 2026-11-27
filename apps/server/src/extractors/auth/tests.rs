use super::{
    CurrentUser, classify_access_token_claims, ensure_access_token_not_expired,
    resolve_current_user, resolve_current_user_from_access_token,
    resolve_service_token_permissions,
};
use crate::auth::{AuthConfig, Claims, encode_access_token, encode_oauth_access_token};
use crate::common::settings::RustokSettings;
use crate::models::{oauth_apps, oauth_consents, oauth_tokens, sessions, tenants, users};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::{ServerAuthRuntime, ServerRuntimeContext};
use chrono::{Duration, Utc};
use rustok_api::{AuthPrincipalKind, Permission};
use rustok_core::{SecurityActorKind, UserRole, UserStatus};
use rustok_migrations::SqliteTestMigrator as Migrator;
use rustok_test_utils::db::setup_test_db_with_migrations;
use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, Schema, Set};
use sea_orm_migration::SchemaManager;
use std::str::FromStr;
use uuid::Uuid;

#[test]
fn resolved_access_token_authority_expires_at_the_earliest_credential_deadline() {
    let expires_at = Utc::now();
    let later = expires_at + Duration::minutes(1);
    // Either the signed or the persisted credential can expire first.
    for (signed, durable) in [
        (expires_at, later),
        (later, expires_at),
        (expires_at, expires_at),
    ] {
        assert!(ensure_access_token_not_expired(
            signed,
            durable,
            expires_at - Duration::nanoseconds(1),
        )
        .is_ok());
        for now in [expires_at, expires_at + Duration::nanoseconds(1)] {
            assert_eq!(
                ensure_access_token_not_expired(signed, durable, now),
                Err((axum::http::StatusCode::UNAUTHORIZED, "Access token expired"))
            );
        }
    }
}

#[tokio::test]
async fn standalone_auth_extractor_rejects_duplicate_authorization_headers() {
    use axum::http::{Request, StatusCode, header::AUTHORIZATION};

    let db = setup_test_db_with_migrations::<Migrator>().await;
    let runtime =
        ServerAuthRuntime::without_auth(ServerRuntimeContext::new(db, RustokSettings::default()));
    let (mut parts, _) = Request::builder()
        .header(AUTHORIZATION, "Bearer first")
        .header(AUTHORIZATION, "Bearer first")
        .body(())
        .unwrap()
        .into_parts();

    assert_eq!(
        resolve_current_user(&mut parts, &runtime)
            .await
            .unwrap_err(),
        (
            StatusCode::BAD_REQUEST,
            "Multiple Authorization headers are not allowed"
        )
    );
}

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
    db.execute_raw(builder.build(&statement))
        .await
        .expect("create oauth_apps table for auth extractor tests");
}

fn test_auth_config() -> AuthConfig {
    crate::auth::test_auth_config("test-secret-key-for-auth-extractor-32b".to_string())
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
        jti: None,
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
    insert_oauth_app_with_scopes(db, tenant_id, app_type, grant_types, is_active, &["forum:*"]).await
}

async fn insert_oauth_app_with_scopes(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    app_type: &str,
    grant_types: &[&str],
    is_active: bool,
    scopes: &[&str],
) -> oauth_apps::Model {
    let model = oauth_apps::ActiveModel {
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
        scopes: Set(serde_json::json!(scopes)),
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
    };
    // Manual (non-manifest) app writes copy display text into the
    // translation table and require a request-scoped locale.
    oauth_apps::scope_runtime_copy_locale("en".to_string(), model.insert(db))
        .await
        .expect("insert oauth app")
}

#[test]
fn token_claim_classifier_returns_explicit_principal_kinds() {
    let direct = claims("direct", None, Uuid::new_v4());
    let oauth_user = claims("authorization_code", Some(Uuid::new_v4()), Uuid::nil());
    let service = claims("client_credentials", Some(Uuid::new_v4()), Uuid::nil());

    assert_eq!(
        classify_access_token_claims(&direct).expect("direct token"),
        AuthPrincipalKind::DirectUser
    );
    assert_eq!(
        classify_access_token_claims(&oauth_user).expect("OAuth user token"),
        AuthPrincipalKind::DelegatedUser
    );
    assert_eq!(
        classify_access_token_claims(&service).expect("service token"),
        AuthPrincipalKind::Service
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
        principal_kind: AuthPrincipalKind::Service,
        client_id: Some(Uuid::new_v4()),
        scopes: Vec::new(),
        grant_type: "client_credentials".to_string(),
        access_token_expires_at: Utc::now() + Duration::hours(1),
        oauth_credential: None,
    };

    assert_eq!(
        current.security_context().actor_kind,
        SecurityActorKind::Service
    );
}

async fn registered_service_token(
    db: &DatabaseConnection,
    config: &AuthConfig,
    app: &oauth_apps::Model,
    scopes: &[String],
) -> String {
    let token_id = Uuid::new_v4();
    let token = encode_oauth_access_token(
        config,
        app.id,
        app.tenant_id,
        UserRole::Customer,
        app.client_id,
        scopes,
        "client_credentials",
        900,
        token_id,
    )
    .expect("encode service access token");
    let now = Utc::now();
    oauth_tokens::ActiveModel {
        id: Set(token_id),
        app_id: Set(app.id),
        user_id: Set(None),
        tenant_id: Set(app.tenant_id),
        token_hash: Set(crate::auth::hash_refresh_token(&token)),
        family_id: Set(token_id),
        authorization_code_id: Set(None),
        grant_type: Set("client_credentials".to_string()),
        token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
        consent_granted_at: Set(None),
        scopes: Set(serde_json::json!(scopes)),
        expires_at: Set((now + Duration::minutes(15)).into()),
        revoked_at: Set(None),
        last_used_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("register service access token");
    token
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

    let runtime = test_auth_runtime(db.clone());
    let token = registered_service_token(
        &db,
        runtime.require_auth_config().unwrap(),
        &app,
        &["forum:*".to_string()],
    )
    .await;
    let current = resolve_current_user_from_access_token(&runtime, tenant.id, &token)
        .await
        .expect("resolve service token permissions");
    let permissions = current.permissions;
    let inferred_role = current.inferred_role;

    assert_eq!(
        permissions,
        vec![Permission::from_str("forum_topics:list").expect("forum permission")]
    );
    assert_eq!(inferred_role, UserRole::Customer);
}

#[tokio::test]
async fn oauth_service_token_rejects_future_registration() {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let tenant = tenants::ActiveModel::new(
        "Future registration tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("tenant");
    let app = insert_oauth_app(&db, tenant.id, "service", &["client_credentials"], true).await;
    let runtime = test_auth_runtime(db.clone());
    let token = registered_service_token(
        &db,
        runtime.require_auth_config().unwrap(),
        &app,
        &["forum:*".to_string()],
    )
    .await;
    assert!(
        resolve_current_user_from_access_token(&runtime, tenant.id, &token)
            .await
            .is_ok()
    );

    let row = oauth_tokens::Entity::find()
        .filter(oauth_tokens::Column::TokenHash.eq(crate::auth::hash_refresh_token(&token)))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut changed: oauth_tokens::ActiveModel = row.into();
    changed.created_at = Set((Utc::now() + Duration::minutes(1)).into());
    changed
        .update(&db)
        .await
        .expect("set future registration creation");
    let error = resolve_current_user_from_access_token(&runtime, tenant.id, &token)
        .await
        .expect_err("future registration must not authorize access");
    assert_eq!(
        error,
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "OAuth access token revoked or unknown"
        )
    );
}

#[tokio::test]
async fn oauth_service_token_rejects_corrupt_or_mismatched_registered_scopes() {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let tenant = tenants::ActiveModel::new(
        "OAuth scope registration tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let app = insert_oauth_app(&db, tenant.id, "service", &["client_credentials"], true).await;
    let runtime = test_auth_runtime(db.clone());
    let token = registered_service_token(
        &db,
        runtime.require_auth_config().unwrap(),
        &app,
        &["forum:*".to_string()],
    )
    .await;
    assert!(
        resolve_current_user_from_access_token(&runtime, tenant.id, &token)
            .await
            .is_ok()
    );

    let mut row = oauth_tokens::Entity::find()
        .filter(oauth_tokens::Column::TokenHash.eq(crate::auth::hash_refresh_token(&token)))
        .one(&db)
        .await
        .expect("load access registration")
        .expect("access row exists");
    for (stored, message) in [
        (serde_json::json!([]), "OAuth access token scopes mismatch"),
        (
            serde_json::json!(["forum:*", 42]),
            "Stored OAuth access token scopes are invalid",
        ),
        (
            serde_json::json!(["forum:*", "modules:list"]),
            "OAuth access token scopes mismatch",
        ),
    ] {
        let mut changed: oauth_tokens::ActiveModel = row.into();
        changed.scopes = Set(stored);
        row = changed.update(&db).await.expect("change registered scopes");
        let error = resolve_current_user_from_access_token(&runtime, tenant.id, &token)
            .await
            .expect_err("inconsistent registration must fail closed");
        assert_eq!(error, (axum::http::StatusCode::UNAUTHORIZED, message));
    }
}

#[tokio::test]
async fn oauth_service_token_authentication_does_not_depend_on_translation_storage() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let tenant = tenants::ActiveModel::new(
        "OAuth security lookup tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let app = insert_oauth_app(&db, tenant.id, "service", &["client_credentials"], true).await;

    db.execute_unprepared("DROP TABLE IF EXISTS oauth_app_translations")
        .await
        .expect("drop translation table to isolate authentication lookup");

    let runtime = test_auth_runtime(db.clone());
    let token = registered_service_token(
        &db,
        runtime.require_auth_config().unwrap(),
        &app,
        &["forum:*".to_string()],
    )
    .await;
    let current = resolve_current_user_from_access_token(&runtime, tenant.id, &token)
        .await
        .expect("OAuth authentication must not depend on presentation translations");
    let permissions = current.permissions;
    let inferred_role = current.inferred_role;

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

    let runtime = test_auth_runtime(db.clone());
    let token =
        registered_service_token(&db, runtime.require_auth_config().unwrap(), &app, &[]).await;
    let current = resolve_current_user_from_access_token(&runtime, tenant.id, &token)
        .await
        .expect("resolve service token permissions");

    assert!(current.permissions.is_empty());
}

#[test]
fn service_permission_snapshot_drops_grants_owned_by_disabled_modules() {
    use std::collections::{BTreeMap, BTreeSet};

    let ownership = BTreeMap::from([
        ("forum_topics:list".to_string(), "forum".to_string()),
        ("modules:list".to_string(), "modules".to_string()),
    ]);
    let availability = rustok_rbac::PermissionAvailability::new(
        ownership,
        BTreeSet::from(["modules".to_string()]),
    );
    let permissions = vec![
        Permission::from_str("forum_topics:list").expect("forum permission"),
        Permission::MODULES_LIST,
    ];

    let active = super::filter_available_permissions(permissions, Some(availability));

    assert_eq!(active, vec![Permission::MODULES_LIST]);
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

    let runtime_ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    let error = resolve_service_token_permissions(
        &runtime_ctx,
        tenant.id,
        Uuid::new_v4(),
        app.client_id,
        UserRole::Customer,
        &["forum:*".to_string()],
        "",
        None,
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
        auth_runtime.require_auth_config().unwrap(),
        user.id,
        tenant.id,
        UserRole::Customer,
        app.client_id,
        &[],
        "authorization_code",
        900,
        Uuid::new_v4(),
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
        auth_runtime.require_auth_config().unwrap(),
        user.id,
        tenant.id,
        UserRole::Customer,
        app.client_id,
        &removed_scopes,
        "authorization_code",
        900,
        Uuid::new_v4(),
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

async fn insert_access_registration(
    db: &DatabaseConnection,
    app: &oauth_apps::Model,
    user_id: Uuid,
    token_id: Uuid,
    token_hash: String,
    revoked: bool,
) {
    let now = Utc::now();
    oauth_tokens::ActiveModel {
        id: Set(token_id),
        app_id: Set(app.id),
        user_id: Set(Some(user_id)),
        tenant_id: Set(app.tenant_id),
        token_hash: Set(token_hash),
        family_id: Set(token_id),
        authorization_code_id: Set(None),
        grant_type: Set("authorization_code".to_string()),
        token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
        consent_granted_at: Set(None),
        scopes: Set(serde_json::json!([])),
        expires_at: Set((now + Duration::minutes(15)).into()),
        revoked_at: Set(if revoked { Some(now.into()) } else { None }),
        last_used_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(db)
    .await
    .expect("insert access registration");
}

#[tokio::test]
async fn oauth_access_token_without_registration_is_rejected() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let auth_runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new(
        "Unregistered OAuth tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let (user, _) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let app = insert_oauth_app(&db, tenant.id, "third_party", &["authorization_code"], true).await;
    let token = encode_oauth_access_token(
        auth_runtime.require_auth_config().unwrap(),
        user.id,
        tenant.id,
        UserRole::Customer,
        app.client_id,
        &[],
        "authorization_code",
        900,
        Uuid::new_v4(),
    )
    .expect("encode oauth access token");

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("unregistered OAuth access JWT must fail closed");
    assert_eq!(
        error,
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "OAuth access token revoked or unknown",
        )
    );
}

#[tokio::test]
async fn revoked_oauth_access_registration_is_rejected() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let auth_runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new(
        "Revoked OAuth tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let (user, _) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let app = insert_oauth_app(&db, tenant.id, "third_party", &["authorization_code"], true).await;
    let token_id = Uuid::new_v4();
    let token = encode_oauth_access_token(
        auth_runtime.require_auth_config().unwrap(),
        user.id,
        tenant.id,
        UserRole::Customer,
        app.client_id,
        &[],
        "authorization_code",
        900,
        token_id,
    )
    .expect("encode oauth access token");
    insert_access_registration(
        &db,
        &app,
        user.id,
        token_id,
        crate::auth::hash_refresh_token(&token),
        true,
    )
    .await;

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("revoked OAuth access registration must fail closed");
    assert_eq!(
        error,
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "OAuth access token revoked or unknown",
        )
    );
}

#[tokio::test]
async fn oauth_access_token_with_stale_consent_epoch_is_rejected() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let auth_runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new(
        "Consent epoch tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let (user, _) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let app = insert_oauth_app(&db, tenant.id, "third_party", &["authorization_code"], true).await;
    let now = Utc::now();
    oauth_consents::ActiveModel {
        id: Set(Uuid::new_v4()),
        app_id: Set(app.id),
        user_id: Set(user.id),
        tenant_id: Set(tenant.id),
        scopes: Set(serde_json::json!([])),
        granted_at: Set(now.into()),
        revoked_at: Set(None),
    }
    .insert(&db)
    .await
    .expect("insert active consent");

    let token_id = Uuid::new_v4();
    let token = encode_oauth_access_token(
        auth_runtime.require_auth_config().unwrap(),
        user.id,
        tenant.id,
        UserRole::Customer,
        app.client_id,
        &[],
        "authorization_code",
        900,
        token_id,
    )
    .expect("encode oauth access token");
    let stale_epoch = now - Duration::minutes(5);
    oauth_tokens::ActiveModel {
        id: Set(token_id),
        app_id: Set(app.id),
        user_id: Set(Some(user.id)),
        tenant_id: Set(app.tenant_id),
        token_hash: Set(crate::auth::hash_refresh_token(&token)),
        family_id: Set(token_id),
        authorization_code_id: Set(None),
        grant_type: Set("authorization_code".to_string()),
        token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
        consent_granted_at: Set(Some(stale_epoch.into())),
        scopes: Set(serde_json::json!([])),
        expires_at: Set((now + Duration::minutes(15)).into()),
        revoked_at: Set(None),
        last_used_at: Set(None),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .expect("insert stale-epoch access registration");

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("stale consent epoch must fail closed");
    assert_eq!(
        error,
        (
            axum::http::StatusCode::UNAUTHORIZED,
            "OAuth token predates the current consent epoch",
        )
    );
}

#[tokio::test]
async fn delegated_user_admin_rejects_revoked_registration_and_consent_epoch() {
    use crate::services::auth_admin_mutation_provider::ServerAuthAdminMutationProvider;
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
    use rustok_auth::{AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, UserAdminMutationPort};
    use sea_orm::EntityTrait;

    let db = setup_test_db_with_migrations::<Migrator>().await;
    let runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new("Admin consent", &format!("tenant-{}", Uuid::new_v4()))
        .insert(&db).await.unwrap();
    let (actor, _) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
        &db, tenant.id, actor.id, UserRole::Admin,
    ).await.unwrap();
    // Include the customer's baseline permissions so the create command can
    // independently authorize the destination role as well as USERS_CREATE.
    let app = insert_oauth_app_with_scopes(
        &db, tenant.id, "third_party", &["authorization_code"], true, &["*:*"],
    ).await;
    let now = Utc::now();
    let consent = oauth_consents::ActiveModel {
        id: Set(Uuid::new_v4()), app_id: Set(app.id), user_id: Set(actor.id),
        tenant_id: Set(tenant.id), scopes: Set(serde_json::json!(["*:*"])),
        granted_at: Set(now.into()), revoked_at: Set(None),
    }.insert(&db).await.unwrap();
    let id = Uuid::new_v4();
    let token = encode_oauth_access_token(
        runtime.require_auth_config().unwrap(), actor.id, tenant.id, UserRole::Admin,
        app.client_id, &["*:*".to_string()], "authorization_code", 900, id,
    ).unwrap();
    let registration = oauth_tokens::ActiveModel {
        id: Set(id), app_id: Set(app.id), user_id: Set(Some(actor.id)),
        tenant_id: Set(tenant.id), token_hash: Set(crate::auth::hash_refresh_token(&token)),
        family_id: Set(id), authorization_code_id: Set(None),
        grant_type: Set("authorization_code".to_string()),
        token_kind: Set(oauth_tokens::TOKEN_KIND_ACCESS.to_string()),
        consent_granted_at: Set(Some(consent.granted_at)),
        scopes: Set(serde_json::json!(["*:*"])), expires_at: Set((now + Duration::minutes(15)).into()),
        revoked_at: Set(None), last_used_at: Set(None), created_at: Set(now.into()),
        updated_at: Set(now.into()),
    }.insert(&db).await.unwrap();
    let admitted = resolve_current_user_from_access_token(&runtime, tenant.id, &token).await.unwrap();
    let scope = || RbacRequestScope::authenticated(&admitted, admitted.access_token_expires_at);
    let provider = ServerAuthAdminMutationProvider::new(runtime.runtime_ctx(), runtime.require_auth_config().unwrap().clone());
    let context = AuthAdminMutationContext {
        actor_id: actor.id, tenant_id: tenant.id, request_id: None,
        locale: Some("en".into()), principal_kind: Some(AuthPrincipalKind::DelegatedUser),
    };
    let create = |email: &str| CreateUserCommand {
        email: email.into(), password: "Password123!Strong".into(), name: None,
        role: Some("customer".into()), status: None, custom_fields: None,
    };
    with_rbac_request_scope(Some(scope()), provider.create_user(&context, create("delegated-ok@example.test")))
        .await.unwrap();

    // A changed consent epoch must fail even when a noncooperating writer
    // omitted token revocation. The exact admitted epoch cannot be replaced.
    oauth_consents::ActiveModel {
        id: Set(consent.id), granted_at: Set((now + Duration::seconds(1)).into()),
        ..Default::default()
    }.update(&db).await.unwrap();
    let denied = with_rbac_request_scope(Some(scope()), provider.create_user(&context, create("delegated-epoch@example.test"))).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)), "{denied:?}");
    oauth_consents::ActiveModel {
        id: Set(consent.id), granted_at: Set(consent.granted_at), ..Default::default()
    }.update(&db).await.unwrap();
    oauth_tokens::ActiveModel {
        id: Set(registration.id), revoked_at: Set(Some(Utc::now().into())), ..Default::default()
    }.update(&db).await.unwrap();
    let denied = with_rbac_request_scope(Some(scope()), provider.create_user(&context, create("delegated-revoked@example.test"))).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)), "{denied:?}");
    for email in ["delegated-epoch@example.test", "delegated-revoked@example.test"] {
        assert!(users::Entity::find_by_email(&db, tenant.id, email).await.unwrap().is_none());
    }
}

#[tokio::test]
async fn oauth_admin_mutations_reject_revoked_direct_session() {
    use crate::services::auth_admin_mutation_provider::ServerAuthAdminMutationProvider;
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
    use rustok_auth::{AuthAdminMutationContext, AuthAdminMutationError, CreateOAuthAppCommand, UpdateOAuthAppCommand, OAuthAdminPort};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let db = setup_test_db_with_migrations::<Migrator>().await;
    let runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new("OAuth admin session", &format!("tenant-{}", Uuid::new_v4()))
        .insert(&db).await.unwrap();
    let (actor, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(&db, tenant.id, actor.id, UserRole::SuperAdmin)
        .await.unwrap();
    let token = encode_access_token(runtime.require_auth_config().unwrap(), actor.id, tenant.id, UserRole::SuperAdmin, session_id).unwrap();
    let admitted = resolve_current_user_from_access_token(&runtime, tenant.id, &token).await.unwrap();
    let scope = || RbacRequestScope::authenticated(&admitted, admitted.access_token_expires_at);
    let provider = ServerAuthAdminMutationProvider::new(runtime.runtime_ctx(), runtime.require_auth_config().unwrap().clone());
    let context = AuthAdminMutationContext {
        actor_id: actor.id, tenant_id: tenant.id, request_id: None,
        locale: Some("en".into()), principal_kind: Some(AuthPrincipalKind::DirectUser),
    };
    let create = |slug: &str| CreateOAuthAppCommand {
        name: "OAuth target".into(), slug: slug.into(), description: None, app_type: "third_party".into(),
        icon_url: None, redirect_uris: vec!["https://client.example/callback".into()],
        scopes: vec!["forum:*".into()], grant_types: vec!["authorization_code".into()],
        granted_permissions: Vec::new(),
    };
    let created = with_rbac_request_scope(Some(scope()), provider.create_oauth_app(&context, create("admin-session-target")))
        .await.unwrap();
    let app_id = created.app.id;
    with_rbac_request_scope(Some(scope()), provider.grant_oauth_app_consent(&context, app_id, vec!["forum:*".into()]))
        .await.unwrap();
    sessions::ActiveModel {
        id: Set(session_id), revoked_at: Set(Some(Utc::now().into())), ..Default::default()
    }.update(&db).await.unwrap();

    let denied = with_rbac_request_scope(Some(scope()), provider.create_oauth_app(&context, create("admin-session-denied"))).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)));
    let denied = with_rbac_request_scope(Some(scope()), provider.update_oauth_app(&context, UpdateOAuthAppCommand {
        id: app_id, name: "Unauthorized change".into(), description: None, icon_url: None,
        redirect_uris: vec!["https://client.example/callback".into()], scopes: vec!["forum:*".into()],
        grant_types: vec!["authorization_code".into()], granted_permissions: Vec::new(),
    })).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)));
    let denied = with_rbac_request_scope(Some(scope()), provider.rotate_oauth_app_secret(&context, app_id)).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)));
    let denied = with_rbac_request_scope(Some(scope()), provider.revoke_oauth_app(&context, app_id)).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)));
    let denied = with_rbac_request_scope(Some(scope()), provider.grant_oauth_app_consent(&context, app_id, vec!["forum:*".into()])).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)));
    let denied = with_rbac_request_scope(Some(scope()), provider.revoke_oauth_app_consent(&context, app_id)).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)));
    assert!(oauth_apps::Entity::find()
        .filter(oauth_apps::Column::TenantId.eq(tenant.id))
        .filter(oauth_apps::Column::Slug.eq("admin-session-denied"))
        .one(&db).await.unwrap().is_none());
    let persisted = oauth_apps::Entity::find_by_id(app_id).one(&db).await.unwrap().unwrap();
    assert!(persisted.is_active());
    assert!(crate::services::oauth_app::OAuthAppService::verify_client_secret(
        &created.client_secret, persisted.client_secret_hash.as_deref().unwrap(),
    ).unwrap());
    assert!(oauth_consents::Entity::find_active_consent(&db, app_id, actor.id, tenant.id).await.unwrap().is_some());
}

#[tokio::test]
async fn oauth_admin_rejects_revoked_service_registration() {
    use crate::services::auth_admin_mutation_provider::ServerAuthAdminMutationProvider;
    use crate::services::oauth_app::{CreateOAuthAppInput, OAuthAppService};
    use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
    use rustok_auth::{AuthAdminMutationContext, AuthAdminMutationError, OAuthAdminPort};
    use sea_orm::EntityTrait;

    let db = setup_test_db_with_migrations::<Migrator>().await;
    let runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new("OAuth service admin", &format!("tenant-{}", Uuid::new_v4()))
        .insert(&db).await.unwrap();
    let issuer = oauth_apps::scope_runtime_copy_locale("en".into(), OAuthAppService::create_app(
        &db, tenant.id, CreateOAuthAppInput {
            name: "Service administrator".into(), slug: "service-admin".into(), description: None,
            app_type: "service".into(), icon_url: None, redirect_uris: Vec::new(),
            scopes: vec!["settings:*".into()], grant_types: vec!["client_credentials".into()],
            granted_permissions: vec![Permission::SETTINGS_MANAGE.to_string()],
        },
    )).await.unwrap().app;
    let target = insert_oauth_app(&db, tenant.id, "third_party", &["authorization_code"], true).await;
    let token = registered_service_token(&db, runtime.require_auth_config().unwrap(), &issuer, &["settings:*".into()]).await;
    let admitted = resolve_current_user_from_access_token(&runtime, tenant.id, &token).await.unwrap();
    let registration_id = admitted.oauth_credential.as_ref().unwrap().registration_id;
    let scope = || RbacRequestScope::authenticated(&admitted, admitted.access_token_expires_at);
    let context = AuthAdminMutationContext {
        actor_id: issuer.id, tenant_id: tenant.id, request_id: None, locale: Some("en".into()),
        principal_kind: Some(AuthPrincipalKind::Service),
    };
    let provider = ServerAuthAdminMutationProvider::new(runtime.runtime_ctx(), runtime.require_auth_config().unwrap().clone());
    // Service identity is an app, not a persisted user. A live service
    // credential may administer another app but cannot grant user consent.
    let first = with_rbac_request_scope(Some(scope()), provider.revoke_oauth_app(&context, target.id)).await.unwrap();
    assert!(!first.is_active);
    let other = insert_oauth_app(&db, tenant.id, "third_party", &["authorization_code"], true).await;
    oauth_tokens::ActiveModel {
        id: Set(registration_id), revoked_at: Set(Some(Utc::now().into())), ..Default::default()
    }.update(&db).await.unwrap();
    let denied = with_rbac_request_scope(Some(scope()), provider.revoke_oauth_app(&context, other.id)).await;
    assert!(matches!(denied, Err(AuthAdminMutationError::Unauthorized)));
    assert!(oauth_apps::Entity::find_by_id(other.id).one(&db).await.unwrap().unwrap().is_active());
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
        auth_runtime.require_auth_config().unwrap(),
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
        auth_runtime.require_auth_config().unwrap(),
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
async fn access_token_resolver_rejects_unverified_users_when_email_verification_is_enabled() {
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let tenant = tenants::ActiveModel::new(
        "Email verification tenant",
        &format!("tenant-{}", Uuid::new_v4()),
    )
    .insert(&db)
    .await
    .expect("create tenant");
    let (user, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    let config = test_auth_config();
    let token = encode_access_token(&config, user.id, tenant.id, UserRole::Customer, session_id)
        .expect("encode access token");

    let mut settings = RustokSettings::default();
    settings.features.email_verification = true;
    let auth_runtime = ServerAuthRuntime::new(ServerRuntimeContext::new(db, settings), config);

    let error = resolve_current_user_from_access_token(&auth_runtime, tenant.id, &token)
        .await
        .expect_err("unverified users must not authenticate while verification is required");

    assert_eq!(
        error,
        (
            axum::http::StatusCode::FORBIDDEN,
            "Email verification required",
        )
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
        auth_runtime.require_auth_config().unwrap(),
        user.id,
        tenant.id,
        UserRole::Customer,
        session_id,
    )
    .expect("encode access token");
    RbacService::invalidate_user_rbac_caches(&tenant.id, &user.id).await;
    db.execute_raw(sea_orm::Statement::from_string(
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

#[tokio::test]
async fn websocket_lease_revalidates_live_authority_and_fails_closed() {
    use super::{WebSocketAuthLease, WebSocketAuthLeaseError};
    use sea_orm::EntityTrait;

    let db = setup_test_db_with_migrations::<Migrator>().await;
    let runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new("WS authority", &format!("tenant-{}", Uuid::new_v4()))
        .insert(&db)
        .await
        .expect("create tenant");
    let (user, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
        &db,
        tenant.id,
        user.id,
        UserRole::Manager,
    )
    .await
    .expect("assign manager role");
    // Persist an absolute deadline earlier than the signed JWT expiry.
    let mut session = sessions::ActiveModel {
        id: Set(session_id),
        absolute_expires_at: Set(Some((Utc::now() + Duration::minutes(5)).into())),
        ..Default::default()
    };
    let persisted = session.clone().update(&db).await.expect("shorten session");
    let deadline = persisted.absolute_deadline();
    let token = encode_access_token(
        runtime.require_auth_config().unwrap(),
        user.id,
        tenant.id,
        UserRole::Manager,
        session_id,
    )
    .expect("encode token");
    let current = resolve_current_user_from_access_token(&runtime, tenant.id, &token)
        .await
        .expect("resolve initial authority");
    assert_eq!(current.access_token_expires_at, deadline);
    let lease = WebSocketAuthLease::new(token, &current);
    assert_eq!(lease.expires_at, deadline);
    assert!(lease.revalidate(&runtime).await.is_ok());

    session.absolute_expires_at = Set(Some((deadline - Duration::minutes(1)).into()));
    session
        .clone()
        .update(&db)
        .await
        .expect("further shorten session");
    assert_eq!(
        lease.revalidate(&runtime).await,
        Err(WebSocketAuthLeaseError::AuthorityChanged)
    );
    session.absolute_expires_at = Set(Some(deadline.into()));
    session
        .update(&db)
        .await
        .expect("restore original deadline");
    assert!(lease.revalidate(&runtime).await.is_ok());

    let mut monotonic = lease.clone();
    monotonic.expired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    assert!(
        monotonic
            .remaining_at(
                deadline - Duration::seconds(1),
                monotonic.monotonic_deadline
            )
            .is_zero()
    );
    assert!(
        monotonic
            .remaining_at(
                deadline - Duration::seconds(2),
                monotonic.monotonic_deadline - std::time::Duration::from_secs(1)
            )
            .is_zero()
    );

    let mut expired = lease.clone();
    expired.expired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    expired.expires_at = Utc::now() - Duration::seconds(1);
    assert_eq!(
        expired.revalidate(&runtime).await,
        Err(WebSocketAuthLeaseError::Expired)
    );

    rustok_rbac::RbacRoleAssignmentDbWriter::replace_role_on(
        &db,
        tenant.id,
        user.id,
        UserRole::Customer,
    )
    .await
    .expect("replace role");
    assert_eq!(
        lease.revalidate(&runtime).await,
        Err(WebSocketAuthLeaseError::AuthorityChanged)
    );

    sessions::Entity::delete_by_id(session_id)
        .exec(&db)
        .await
        .expect("delete session");
    assert_eq!(
        lease.revalidate(&runtime).await,
        Err(WebSocketAuthLeaseError::ResolutionFailed)
    );
}

#[tokio::test]
async fn registered_service_access_token_rejects_disabled_tenant() {
    use sea_orm::IntoActiveModel;

    let db = setup_test_db_with_migrations::<Migrator>().await;
    ensure_oauth_apps_table(&db).await;
    let runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new("Service tenant", &format!("tenant-{}", Uuid::new_v4()))
        .insert(&db)
        .await
        .expect("create tenant");
    let tenant_id = tenant.id;
    let app = insert_oauth_app(&db, tenant_id, "service", &["client_credentials"], true).await;
    let token = registered_service_token(
        &db,
        runtime.require_auth_config().unwrap(),
        &app,
        &["forum:*".to_string()],
    )
    .await;
    assert!(
        resolve_current_user_from_access_token(&runtime, tenant_id, &token)
            .await
            .is_ok()
    );

    let mut update = tenant.into_active_model();
    update.is_active = Set(false);
    update.update(&db).await.expect("disable tenant");
    let error = resolve_current_user_from_access_token(&runtime, tenant_id, &token)
        .await
        .expect_err("disabled tenant must reject even a live registered service token");
    assert_eq!(
        error,
        (axum::http::StatusCode::FORBIDDEN, "Tenant is unavailable")
    );
}

#[tokio::test]
async fn websocket_lease_rejects_a_disabled_tenant() {
    use super::{WebSocketAuthLease, WebSocketAuthLeaseError};
    use sea_orm::IntoActiveModel;

    let db = setup_test_db_with_migrations::<Migrator>().await;
    let runtime = test_auth_runtime(db.clone());
    let tenant = tenants::ActiveModel::new("WS tenant", &format!("tenant-{}", Uuid::new_v4()))
        .insert(&db)
        .await
        .expect("create tenant");
    let (user, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
        &db,
        tenant.id,
        user.id,
        UserRole::Customer,
    )
    .await
    .expect("assign customer role");
    let token = encode_access_token(
        runtime.require_auth_config().unwrap(),
        user.id,
        tenant.id,
        UserRole::Customer,
        session_id,
    )
    .expect("encode token");
    let current = resolve_current_user_from_access_token(&runtime, tenant.id, &token)
        .await
        .expect("resolve initial authority");
    let lease = WebSocketAuthLease::new(token.clone(), &current);
    assert!(lease.revalidate(&runtime).await.is_ok());
    let tenant_id = tenant.id;
    let mut update = tenant.into_active_model();
    update.is_active = Set(false);
    update.update(&db).await.expect("disable tenant");
    let error = resolve_current_user_from_access_token(&runtime, tenant_id, &token)
        .await
        .expect_err("disabled tenant must reject a live first-party session");
    assert_eq!(
        error,
        (axum::http::StatusCode::FORBIDDEN, "Tenant is unavailable")
    );
    assert_eq!(
        lease.revalidate(&runtime).await,
        Err(WebSocketAuthLeaseError::ResolutionFailed)
    );
}

#[tokio::test]
async fn final_authority_check_rejects_membership_aba_and_revoked_session() {
    use sea_orm::EntityTrait;
    let db = setup_test_db_with_migrations::<Migrator>().await;
    let runtime = test_auth_runtime(db.clone());
    let tenant =
        tenants::ActiveModel::new("Final admission", &format!("tenant-{}", Uuid::new_v4()))
            .insert(&db)
            .await
            .unwrap();
    let (user, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
    rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
        &db,
        tenant.id,
        user.id,
        UserRole::Manager,
    )
    .await
    .unwrap();
    let config = runtime.require_auth_config().unwrap();
    let token =
        encode_access_token(config, user.id, tenant.id, UserRole::Manager, session_id).unwrap();
    let claims = crate::auth::decode_access_token(config, &token).unwrap();
    let current = resolve_current_user_from_access_token(&runtime, tenant.id, &token)
        .await
        .unwrap();
    let session = sessions::Entity::find_by_id(session_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let durable_deadline = session
        .expires_at
        .with_timezone(&Utc)
        .min(session.absolute_deadline());
    let initial = super::read_static_authority_revision(&db, tenant.id, user.id)
        .await
        .unwrap();
    macro_rules! revalidate {
        ($revision:expr) => {
            super::revalidate_admitted_authority(
                &runtime,
                tenant.id,
                &claims,
                &token,
                &current.user,
                &current.permissions,
                &current.inferred_role,
                AuthPrincipalKind::DirectUser,
                None,
                durable_deadline,
                Utc::now(),
                Some($revision),
            )
        };
    }
    assert!(revalidate!(&initial).await.is_ok());
    rustok_rbac::RbacRoleAssignmentDbWriter::replace_role_on(
        &db,
        tenant.id,
        user.id,
        UserRole::Customer,
    )
    .await
    .unwrap();
    rustok_rbac::RbacRoleAssignmentDbWriter::replace_role_on(
        &db,
        tenant.id,
        user.id,
        UserRole::Manager,
    )
    .await
    .unwrap();
    let restored = super::read_static_authority_revision(&db, tenant.id, user.id)
        .await
        .unwrap();
    assert_ne!(initial, restored, "membership ABA must retain history");
    assert!(revalidate!(&initial).await.is_err());
    assert!(revalidate!(&restored).await.is_ok());
    let mut session: sessions::ActiveModel = session.into();
    session.revoked_at = Set(Some(Utc::now().into()));
    session.update(&db).await.unwrap();
    assert!(revalidate!(&restored).await.is_err());
}
