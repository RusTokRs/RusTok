use crate::auth::{auth_config_from_ctx, decode_access_token};
use crate::context::{infer_user_role_from_permissions, TenantContextExt};
use crate::models::{
    oauth_apps::Entity as OAuthApps,
    sessions::Entity as Sessions,
    users::{self, Entity as Users},
};
use crate::services::rbac_service::RbacService;
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{request::Parts, StatusCode},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use loco_rs::app::AppContext;
use rustok_core::{Permission, UserRole};
use sea_orm::{DatabaseConnection, EntityTrait};
use tracing::warn;

pub struct CurrentUser {
    pub user: users::Model,
    pub session_id: uuid::Uuid,
    pub permissions: Vec<Permission>,
    pub inferred_role: UserRole,
    pub client_id: Option<uuid::Uuid>,
    pub scopes: Vec<String>,
    pub grant_type: String,
}

impl CurrentUser {
    pub fn security_context(&self) -> rustok_core::SecurityContext {
        rustok_core::SecurityContext::from_permissions(
            self.inferred_role.clone(),
            Some(self.user.id),
            self.permissions.iter().copied(),
        )
    }
}

async fn resolve_service_token_permissions(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    client_id: uuid::Uuid,
    claimed_role: UserRole,
) -> Result<(Vec<Permission>, UserRole), (StatusCode, &'static str)> {
    let app = OAuthApps::find_active_by_client_id(db, client_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
        .ok_or((StatusCode::UNAUTHORIZED, "OAuth app not found or inactive"))?;

    if app.tenant_id != tenant_id {
        return Err((StatusCode::FORBIDDEN, "Token belongs to another tenant"));
    }

    let permissions = app.parsed_granted_permissions().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "OAuth app permissions are invalid",
        )
    })?;
    let inferred_role = infer_user_role_from_permissions(&permissions);
    if claimed_role != inferred_role {
        RbacService::record_claim_role_mismatch();
        warn!(
            client_id = %client_id,
            tenant_id = %tenant_id,
            claimed_role = %claimed_role,
            inferred_role = %inferred_role,
            "rbac_claim_role_mismatch"
        );
    }

    Ok((permissions, inferred_role))
}

pub(crate) async fn resolve_current_user<S>(
    parts: &mut Parts,
    state: &S,
) -> Result<CurrentUser, (StatusCode, &'static str)>
where
    S: Send + Sync,
    AppContext: FromRef<S>,
{
    let ctx = AppContext::from_ref(state);

    let tenant_id = parts
        .tenant_context()
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Tenant context missing"))?
        .id;

    let TypedHeader(Authorization(bearer)) =
        TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Missing or invalid token"))?;

    resolve_current_user_from_access_token(&ctx, tenant_id, bearer.token()).await
}

pub async fn resolve_current_user_from_access_token(
    ctx: &AppContext,
    tenant_id: uuid::Uuid,
    access_token: &str,
) -> Result<CurrentUser, (StatusCode, &'static str)> {
    let auth_config = auth_config_from_ctx(ctx).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "JWT secret not configured",
        )
    })?;

    let claims = decode_access_token(&auth_config, access_token)
        .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token signature"))?;

    if claims.tenant_id != tenant_id {
        return Err((StatusCode::FORBIDDEN, "Token belongs to another tenant"));
    }

    let is_oauth_service_token =
        claims.client_id.is_some() && claims.session_id == uuid::Uuid::nil();

    if !is_oauth_service_token {
        let session = Sessions::find_by_id(claims.session_id)
            .one(&ctx.db)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?
            .ok_or((StatusCode::UNAUTHORIZED, "Session not found"))?;

        if session.tenant_id != tenant_id || !session.is_active() {
            return Err((StatusCode::UNAUTHORIZED, "Session expired"));
        }
    }

    let user = Users::find_by_id(claims.sub)
        .one(&ctx.db)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

    let (user, permissions, inferred_role, session_id) = if let Some(user) = user {
        if !user.is_active() {
            return Err((StatusCode::FORBIDDEN, "User is inactive"));
        }

        let permissions = RbacService::get_user_permissions(&ctx.db, &tenant_id, &user.id)
            .await
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Database error"))?;

        let inferred_role = infer_user_role_from_permissions(&permissions);
        if claims.role != inferred_role {
            RbacService::record_claim_role_mismatch();
            warn!(
                user_id = %user.id,
                tenant_id = %tenant_id,
                claimed_role = %claims.role,
                inferred_role = %inferred_role,
                "rbac_claim_role_mismatch"
            );
        }

        (user, permissions, inferred_role, claims.session_id)
    } else if is_oauth_service_token {
        let client_id = claims.client_id.ok_or((
            StatusCode::UNAUTHORIZED,
            "OAuth service token is missing client_id",
        ))?;
        let (permissions, inferred_role) =
            resolve_service_token_permissions(&ctx.db, tenant_id, client_id, claims.role).await?;

        (
            users::Model::default_service_user(claims.sub, tenant_id),
            permissions,
            inferred_role,
            uuid::Uuid::nil(),
        )
    } else {
        return Err((StatusCode::UNAUTHORIZED, "User not found"));
    };

    Ok(CurrentUser {
        user,
        session_id,
        permissions,
        inferred_role,
        client_id: claims.client_id,
        scopes: claims.scopes,
        grant_type: claims.grant_type,
    })
}

impl<S> FromRequestParts<S> for CurrentUser
where
    S: Send + Sync,
    AppContext: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        resolve_current_user(parts, state).await
    }
}

pub struct OptionalCurrentUser(pub Option<CurrentUser>);

impl<S> FromRequestParts<S> for OptionalCurrentUser
where
    S: Send + Sync,
    AppContext: FromRef<S>,
{
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        if parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .is_none()
        {
            return Ok(Self(None));
        }

        let current_user = resolve_current_user(parts, state).await?;
        Ok(Self(Some(current_user)))
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_current_user_from_access_token, resolve_service_token_permissions};
    use crate::auth::{auth_config_from_ctx, encode_access_token};
    use crate::models::{oauth_apps, sessions, tenants, users};
    use crate::services::rbac_service::RbacService;
    use chrono::{Duration, Utc};
    use loco_rs::{
        app::{AppContext, SharedStore},
        cache,
        config::{Auth, JWT},
        environment::Environment,
        storage::{self, Storage},
        tests_cfg::config::test_config,
    };
    use migration::Migrator;
    use rustok_core::{Permission, UserRole, UserStatus};
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbBackend, Schema, Set};
    use sea_orm_migration::SchemaManager;
    use std::{str::FromStr, sync::Arc};
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

    fn test_app_context(db: DatabaseConnection) -> AppContext {
        let mut config = test_config();
        config.auth = Some(Auth {
            jwt: Some(JWT {
                location: None,
                secret: "test-secret-key-for-auth-extractor-32b".to_string(),
                expiration: 900,
            }),
        });

        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config,
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store: Arc::new(SharedStore::default()),
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

    #[tokio::test]
    async fn oauth_service_token_resolves_granted_permissions_from_oauth_app() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        ensure_oauth_apps_table(&db).await;
        let tenant =
            tenants::ActiveModel::new("OAuth tenant", &format!("tenant-{}", Uuid::new_v4()))
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
        let ctx = test_app_context(db.clone());
        let tenant = tenants::ActiveModel::new(
            "Inactive user tenant",
            &format!("tenant-{}", Uuid::new_v4()),
        )
        .insert(&db)
        .await
        .expect("create tenant");
        let (user, session_id) =
            insert_user_with_session(&db, tenant.id, UserStatus::Inactive).await;
        let auth_config = auth_config_from_ctx(&ctx).expect("auth config");
        let token = encode_access_token(
            &auth_config,
            user.id,
            tenant.id,
            UserRole::Customer,
            session_id,
        )
        .expect("encode access token");

        let error = resolve_current_user_from_access_token(&ctx, tenant.id, &token)
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
        let ctx = test_app_context(db.clone());
        let tenant =
            tenants::ActiveModel::new("RBAC failure tenant", &format!("tenant-{}", Uuid::new_v4()))
                .insert(&db)
                .await
                .expect("create tenant");
        let (user, session_id) = insert_user_with_session(&db, tenant.id, UserStatus::Active).await;
        let auth_config = auth_config_from_ctx(&ctx).expect("auth config");
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

        let error = resolve_current_user_from_access_token(&ctx, tenant.id, &token)
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
}
