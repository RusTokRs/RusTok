use chrono::{Duration, Utc};
use loco_rs::app::AppContext;
use loco_rs::Error;
use loco_rs::Result;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};

use crate::auth::{auth_config_from_ctx, 
    decode_password_reset_token, encode_access_token, generate_refresh_token, hash_password,
    hash_refresh_token, verify_password, AuthConfig,
};
use crate::context::infer_user_role_from_permissions;
use crate::models::{sessions, users};
use std::sync::atomic::{AtomicU64, Ordering};

use super::rbac_service::RbacService;

pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub effective_role: rustok_core::UserRole,
}

#[derive(Debug)]
pub enum AuthLifecycleError {
    EmailAlreadyExists,
    InvalidCredentials,
    UserInactive,
    InvalidRefreshToken,
    SessionExpired,
    UserNotFound,
    InvalidResetToken,
    Internal(Error),
}

impl From<Error> for AuthLifecycleError {
    fn from(value: Error) -> Self {
        Self::Internal(value)
    }
}

impl From<sea_orm::DbErr> for AuthLifecycleError {
    fn from(value: sea_orm::DbErr) -> Self {
        Self::Internal(value.into())
    }
}

impl From<rustok_auth::AuthError> for AuthLifecycleError {
    fn from(value: rustok_auth::AuthError) -> Self {
        Self::Internal(crate::auth::auth_err(value))
    }
}

impl From<AuthLifecycleError> for Error {
    fn from(value: AuthLifecycleError) -> Self {
        match value {
            AuthLifecycleError::EmailAlreadyExists => {
                Error::BadRequest("Email already exists".into())
            }
            AuthLifecycleError::InvalidCredentials => {
                Error::Unauthorized("Invalid credentials".into())
            }
            AuthLifecycleError::UserInactive => Error::Unauthorized("User is inactive".into()),
            AuthLifecycleError::InvalidRefreshToken => {
                Error::Unauthorized("Invalid refresh token".into())
            }
            AuthLifecycleError::SessionExpired => Error::Unauthorized("Session expired".into()),
            AuthLifecycleError::UserNotFound => Error::Unauthorized("User not found".into()),
            AuthLifecycleError::InvalidResetToken => {
                Error::Unauthorized("Invalid reset token".into())
            }
            AuthLifecycleError::Internal(err) => err,
        }
    }
}

pub struct AuthLifecycleService;

#[derive(Clone, Copy, Debug, Default)]
pub struct AuthLifecycleMetricsSnapshot {
    pub password_reset_sessions_revoked_total: u64,
    pub change_password_sessions_revoked_total: u64,
    pub flow_inconsistency_total: u64,
    pub login_inactive_user_attempt_total: u64,
}

static AUTH_PASSWORD_RESET_SESSIONS_REVOKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static AUTH_CHANGE_PASSWORD_SESSIONS_REVOKED_TOTAL: AtomicU64 = AtomicU64::new(0);
static AUTH_FLOW_INCONSISTENCY_TOTAL: AtomicU64 = AtomicU64::new(0);
static AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL: AtomicU64 = AtomicU64::new(0);

impl AuthLifecycleService {
    pub fn metrics_snapshot() -> AuthLifecycleMetricsSnapshot {
        AuthLifecycleMetricsSnapshot {
            password_reset_sessions_revoked_total: AUTH_PASSWORD_RESET_SESSIONS_REVOKED_TOTAL
                .load(Ordering::Relaxed),
            change_password_sessions_revoked_total: AUTH_CHANGE_PASSWORD_SESSIONS_REVOKED_TOTAL
                .load(Ordering::Relaxed),
            flow_inconsistency_total: AUTH_FLOW_INCONSISTENCY_TOTAL.load(Ordering::Relaxed),
            login_inactive_user_attempt_total: AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL
                .load(Ordering::Relaxed),
        }
    }

    pub fn record_flow_inconsistency() {
        AUTH_FLOW_INCONSISTENCY_TOTAL.fetch_add(1, Ordering::Relaxed);
    }

    #[cfg(test)]
    fn reset_metrics_for_tests() {
        AUTH_PASSWORD_RESET_SESSIONS_REVOKED_TOTAL.store(0, Ordering::Relaxed);
        AUTH_CHANGE_PASSWORD_SESSIONS_REVOKED_TOTAL.store(0, Ordering::Relaxed);
        AUTH_FLOW_INCONSISTENCY_TOTAL.store(0, Ordering::Relaxed);
        AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL.store(0, Ordering::Relaxed);
    }

    pub async fn create_user(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        name: Option<String>,
        role: rustok_core::UserRole,
        status: Option<rustok_core::UserStatus>,
    ) -> std::result::Result<users::Model, AuthLifecycleError> {
        Self::create_user_db(&ctx.db, tenant_id, email, password, name, role, status).await
    }

    async fn create_user_db(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        name: Option<String>,
        role: rustok_core::UserRole,
        status: Option<rustok_core::UserStatus>,
    ) -> std::result::Result<users::Model, AuthLifecycleError> {
        if users::Entity::find_by_email(db, tenant_id, email)
            .await
            .map_err(AuthLifecycleError::from)?
            .is_some()
        {
            return Err(AuthLifecycleError::EmailAlreadyExists);
        }

        let password_hash = hash_password(password).map_err(AuthLifecycleError::from)?;

        let tx = db.begin().await.map_err(AuthLifecycleError::from)?;

        let mut user = users::ActiveModel::new(tenant_id, email, &password_hash);
        user.name = Set(name);
        if let Some(status) = status {
            user.status = Set(status);
        }

        let user = match user.insert(&tx).await {
            Ok(user) => user,
            Err(err) => {
                if users::Entity::find_by_email(db, tenant_id, email)
                    .await
                    .map_err(AuthLifecycleError::from)?
                    .is_some()
                {
                    return Err(AuthLifecycleError::EmailAlreadyExists);
                }

                return Err(AuthLifecycleError::from(err));
            }
        };

        RbacService::replace_user_role(&tx, &user.id, &tenant_id, role)
            .await
            .map_err(AuthLifecycleError::from)?;

        tx.commit().await.map_err(AuthLifecycleError::from)?;

        Ok(user)
    }

    pub async fn update_profile(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        name: Option<String>,
    ) -> std::result::Result<users::Model, AuthLifecycleError> {
        Self::update_profile_db(&ctx.db, tenant_id, user_id, name).await
    }

    async fn update_profile_db(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        name: Option<String>,
    ) -> std::result::Result<users::Model, AuthLifecycleError> {
        let user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::UserNotFound)?;

        let mut user_active: users::ActiveModel = user.into();
        if let Some(name) = name {
            let normalized = if name.trim().is_empty() {
                None
            } else {
                Some(name.trim().to_string())
            };
            user_active.name = Set(normalized);
        }

        user_active
            .update(db)
            .await
            .map_err(AuthLifecycleError::from)
    }

    pub async fn register(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        name: Option<String>,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        let config = auth_config_from_ctx(ctx).map_err(AuthLifecycleError::from)?;
        let user = Self::create_user(
            ctx,
            tenant_id,
            email,
            password,
            name,
            rustok_core::UserRole::Customer,
            None,
        )
        .await?;

        let tokens =
            Self::create_session_and_tokens(ctx, tenant_id, &user, None, None, &config).await?;

        Ok((user, tokens))
    }

    pub async fn login(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        let config = auth_config_from_ctx(ctx).map_err(AuthLifecycleError::from)?;

        Self::login_with_config(
            &ctx.db, &config, tenant_id, email, password, ip_address, user_agent,
        )
        .await
    }

    async fn login_with_config(
        db: &DatabaseConnection,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        let user = users::Entity::find_by_email(db, tenant_id, email)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidCredentials)?;

        if !verify_password(password, &user.password_hash).map_err(AuthLifecycleError::from)? {
            return Err(AuthLifecycleError::InvalidCredentials);
        }

        if !user.is_active() {
            AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL.fetch_add(1, Ordering::Relaxed);
            return Err(AuthLifecycleError::UserInactive);
        }

        let now = Utc::now();
        let mut user_active: users::ActiveModel = user.clone().into();
        user_active.last_login_at = Set(Some(now.into()));
        let user = user_active
            .update(db)
            .await
            .map_err(AuthLifecycleError::from)?;

        let tokens = Self::create_session_and_tokens_db(
            db, config, tenant_id, &user, ip_address, user_agent,
        )
        .await?;

        Ok((user, tokens))
    }

    pub async fn refresh(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        refresh_token: &str,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        let config = auth_config_from_ctx(ctx).map_err(AuthLifecycleError::from)?;
        Self::refresh_with_config_db(&ctx.db, &config, tenant_id, refresh_token).await
    }

    async fn refresh_with_config_db(
        db: &DatabaseConnection,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        refresh_token: &str,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        let token_hash = hash_refresh_token(refresh_token);

        let session = sessions::Entity::find_by_token_hash(db, tenant_id, &token_hash)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidRefreshToken)?;

        if !session.is_active() {
            return Err(AuthLifecycleError::SessionExpired);
        }

        let user = users::Entity::find_by_id(session.user_id)
            .one(db)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::UserNotFound)?;

        if !user.is_active() {
            return Err(AuthLifecycleError::UserInactive);
        }

        let now = Utc::now();
        let new_refresh_token = generate_refresh_token();
        let new_token_hash = hash_refresh_token(&new_refresh_token);
        let expires_at = now + Duration::seconds(config.refresh_expiration as i64);

        let session_id = session.id;
        let mut session_model: sessions::ActiveModel = session.into();
        session_model.token_hash = Set(new_token_hash);
        session_model.expires_at = Set(expires_at.into());
        session_model.last_used_at = Set(Some(now.into()));
        session_model
            .update(db)
            .await
            .map_err(AuthLifecycleError::from)?;

        let effective_role = Self::resolve_effective_role(db, tenant_id, user.id).await?;
        let access_token = encode_access_token(
            config,
            user.id,
            tenant_id,
            effective_role.clone(),
            session_id,
        )
        .map_err(AuthLifecycleError::from)?;

        Ok((
            user,
            AuthTokens {
                access_token,
                refresh_token: new_refresh_token,
                expires_in: config.access_expiration,
                effective_role,
            },
        ))
    }

    pub async fn confirm_password_reset(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        token: &str,
        password: &str,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let config = auth_config_from_ctx(ctx).map_err(AuthLifecycleError::from)?;
        Self::confirm_password_reset_with_config(&ctx.db, &config, tenant_id, token, password).await
    }

    async fn confirm_password_reset_with_config(
        db: &DatabaseConnection,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        token: &str,
        password: &str,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let claims = decode_password_reset_token(config, token)
            .map_err(|_| AuthLifecycleError::InvalidResetToken)?;

        if claims.tenant_id != tenant_id {
            return Err(AuthLifecycleError::InvalidResetToken);
        }

        let user = users::Entity::find_by_email(db, tenant_id, &claims.sub)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidResetToken)?;

        Self::reset_password_and_revoke_sessions(db, tenant_id, user, password, None).await
    }

    pub async fn change_password(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        current_session_id: uuid::Uuid,
        current_password: &str,
        new_password: &str,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(&ctx.db)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidCredentials)?;

        if !verify_password(current_password, &user.password_hash)
            .map_err(AuthLifecycleError::from)?
        {
            return Err(AuthLifecycleError::InvalidCredentials);
        }

        let mut user_active: users::ActiveModel = user.into();
        user_active.password_hash =
            Set(hash_password(new_password).map_err(AuthLifecycleError::from)?);
        user_active
            .update(&ctx.db)
            .await
            .map_err(AuthLifecycleError::from)?;

        let revoked_sessions =
            Self::revoke_user_sessions(ctx, tenant_id, user_id, Some(current_session_id)).await?;
        AUTH_CHANGE_PASSWORD_SESSIONS_REVOKED_TOTAL.fetch_add(revoked_sessions, Ordering::Relaxed);

        Ok(())
    }

    async fn reset_password_and_revoke_sessions(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        user: users::Model,
        new_password: &str,
        except_session_id: Option<uuid::Uuid>,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let user_id = user.id;
        let mut user_active: users::ActiveModel = user.into();
        user_active.password_hash =
            Set(hash_password(new_password).map_err(AuthLifecycleError::from)?);
        user_active
            .update(db)
            .await
            .map_err(AuthLifecycleError::from)?;

        let revoked_sessions =
            Self::revoke_user_sessions_db(db, tenant_id, user_id, except_session_id).await?;
        AUTH_PASSWORD_RESET_SESSIONS_REVOKED_TOTAL.fetch_add(revoked_sessions, Ordering::Relaxed);

        Ok(())
    }

    async fn create_session_and_tokens(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        user: &users::Model,
        ip_address: Option<String>,
        user_agent: Option<String>,
        config: &AuthConfig,
    ) -> std::result::Result<AuthTokens, AuthLifecycleError> {
        Self::create_session_and_tokens_db(&ctx.db, config, tenant_id, user, ip_address, user_agent)
            .await
    }

    async fn create_session_and_tokens_db(
        db: &DatabaseConnection,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        user: &users::Model,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> std::result::Result<AuthTokens, AuthLifecycleError> {
        let now = Utc::now();
        let refresh_token = generate_refresh_token();
        let token_hash = hash_refresh_token(&refresh_token);
        let expires_at = now + Duration::seconds(config.refresh_expiration as i64);

        let session = sessions::ActiveModel::new(
            tenant_id, user.id, token_hash, expires_at, ip_address, user_agent,
        )
        .insert(db)
        .await
        .map_err(AuthLifecycleError::from)?;

        let effective_role = Self::resolve_effective_role(db, tenant_id, user.id).await?;
        let access_token = encode_access_token(
            config,
            user.id,
            tenant_id,
            effective_role.clone(),
            session.id,
        )
        .map_err(AuthLifecycleError::from)?;

        Ok(AuthTokens {
            access_token,
            refresh_token,
            expires_in: config.access_expiration,
            effective_role,
        })
    }

    async fn resolve_effective_role(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> std::result::Result<rustok_core::UserRole, AuthLifecycleError> {
        let permissions = RbacService::get_user_permissions(db, &tenant_id, &user_id)
            .await
            .map_err(AuthLifecycleError::from)?;
        Ok(infer_user_role_from_permissions(&permissions))
    }

    async fn revoke_user_sessions(
        ctx: &AppContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        except_session_id: Option<uuid::Uuid>,
    ) -> std::result::Result<u64, AuthLifecycleError> {
        Self::revoke_user_sessions_db(&ctx.db, tenant_id, user_id, except_session_id).await
    }

    async fn revoke_user_sessions_db(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        except_session_id: Option<uuid::Uuid>,
    ) -> std::result::Result<u64, AuthLifecycleError> {
        let mut query = sessions::Entity::update_many()
            .col_expr(sessions::Column::RevokedAt, Expr::value(Utc::now()))
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::UserId.eq(user_id))
            .filter(sessions::Column::RevokedAt.is_null());

        if let Some(session_id) = except_session_id {
            query = query.filter(sessions::Column::Id.ne(session_id));
        }

        let result = query.exec(db).await.map_err(AuthLifecycleError::from)?;
        Ok(result.rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthLifecycleError, AuthLifecycleMetricsSnapshot, AuthLifecycleService, Error,
        AUTH_CHANGE_PASSWORD_SESSIONS_REVOKED_TOTAL, AUTH_FLOW_INCONSISTENCY_TOTAL,
        AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL, AUTH_PASSWORD_RESET_SESSIONS_REVOKED_TOTAL,
    };
    use crate::auth::{
        decode_access_token, hash_password, hash_refresh_token, verify_password, AuthConfig,
    };
    use crate::models::_entities::user_roles;
    use crate::models::{sessions, tenants, users};
    use crate::services::rbac_service::RbacService;
    use chrono::{Duration, Utc};
    use migration::Migrator;
    use rustok_core::UserStatus;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
    use serial_test::serial;
    use std::sync::atomic::Ordering;

    #[test]
    #[serial]
    fn metrics_snapshot_reads_current_auth_lifecycle_counters() {
        AuthLifecycleService::reset_metrics_for_tests();
        let before = AuthLifecycleService::metrics_snapshot();

        AUTH_PASSWORD_RESET_SESSIONS_REVOKED_TOTAL.fetch_add(2, Ordering::Relaxed);
        AUTH_CHANGE_PASSWORD_SESSIONS_REVOKED_TOTAL.fetch_add(3, Ordering::Relaxed);
        AUTH_FLOW_INCONSISTENCY_TOTAL.fetch_add(1, Ordering::Relaxed);
        AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL.fetch_add(4, Ordering::Relaxed);

        let after = AuthLifecycleService::metrics_snapshot();
        assert!(
            after.password_reset_sessions_revoked_total
                >= before.password_reset_sessions_revoked_total + 2
        );
        assert_eq!(
            after.change_password_sessions_revoked_total,
            before.change_password_sessions_revoked_total + 3
        );
        assert_eq!(
            after.flow_inconsistency_total,
            before.flow_inconsistency_total + 1
        );
        assert!(
            after.login_inactive_user_attempt_total >= before.login_inactive_user_attempt_total + 4
        );
    }

    #[test]
    #[serial]
    fn flow_inconsistency_counter_can_be_incremented() {
        AuthLifecycleService::reset_metrics_for_tests();
        let before = AuthLifecycleService::metrics_snapshot();
        AuthLifecycleService::record_flow_inconsistency();
        let after = AuthLifecycleService::metrics_snapshot();
        assert_eq!(
            after.flow_inconsistency_total,
            before.flow_inconsistency_total + 1
        );
    }

    #[test]
    #[serial]
    fn auth_lifecycle_metrics_snapshot_default_is_zeroed() {
        AuthLifecycleService::reset_metrics_for_tests();
        let snapshot = AuthLifecycleMetricsSnapshot::default();
        assert_eq!(snapshot.password_reset_sessions_revoked_total, 0);
        assert_eq!(snapshot.change_password_sessions_revoked_total, 0);
        assert_eq!(snapshot.flow_inconsistency_total, 0);
        assert_eq!(snapshot.login_inactive_user_attempt_total, 0);
    }

    #[test]
    fn maps_email_exists_to_bad_request() {
        let err: Error = AuthLifecycleError::EmailAlreadyExists.into();
        match err {
            Error::BadRequest(msg) => assert_eq!(msg, "Email already exists"),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn maps_invalid_credentials_to_unauthorized() {
        let err: Error = AuthLifecycleError::InvalidCredentials.into();
        match err {
            Error::Unauthorized(msg) => assert_eq!(msg, "Invalid credentials"),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn maps_user_inactive_to_unauthorized() {
        let err: Error = AuthLifecycleError::UserInactive.into();
        match err {
            Error::Unauthorized(msg) => assert_eq!(msg, "User is inactive"),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn maps_invalid_reset_token_to_unauthorized() {
        let err: Error = AuthLifecycleError::InvalidResetToken.into();
        match err {
            Error::Unauthorized(msg) => assert_eq!(msg, "Invalid reset token"),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn maps_user_not_found_to_unauthorized() {
        let err: Error = AuthLifecycleError::UserNotFound.into();
        match err {
            Error::Unauthorized(msg) => assert_eq!(msg, "User not found"),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn keeps_internal_error_as_is() {
        let err: Error = AuthLifecycleError::Internal(Error::Unauthorized("inner".into())).into();
        match err {
            Error::Unauthorized(msg) => assert_eq!(msg, "inner"),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[tokio::test]
    #[serial]
    async fn reset_password_revoke_sessions_marks_all_sessions_revoked() {
        AuthLifecycleService::reset_metrics_for_tests();
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Test tenant", "test-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let user = users::ActiveModel::new(tenant.id, "user@example.com", "old-hash")
            .insert(&db)
            .await
            .expect("failed to create user");

        let now = Utc::now();
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "token-1".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create session 1");
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "token-2".to_string(),
            now + Duration::hours(2),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create session 2");

        let metrics_before = AuthLifecycleService::metrics_snapshot();

        AuthLifecycleService::reset_password_and_revoke_sessions(
            &db,
            tenant.id,
            user.clone(),
            "new-password",
            None,
        )
        .await
        .expect("password reset should succeed");

        let active_sessions = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .all(&db)
            .await
            .expect("failed to query active sessions");

        assert!(active_sessions.is_empty());

        let metrics_after = AuthLifecycleService::metrics_snapshot();
        assert!(
            metrics_after.password_reset_sessions_revoked_total
                >= metrics_before.password_reset_sessions_revoked_total + 2
        );
    }

    #[tokio::test]
    async fn create_user_assigns_requested_role_relations() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Test tenant", "test-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let user = AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "manager@example.com",
            "password123",
            Some("Manager".to_string()),
            rustok_core::UserRole::Manager,
            Some(rustok_core::UserStatus::Active),
        )
        .await
        .expect("failed to create user via auth lifecycle");

        let relations_count = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(user.id))
            .count(&db)
            .await
            .expect("failed to query user role relations");

        assert!(
            relations_count > 0,
            "expected user_roles relation to be created"
        );

        let resolved_permissions = RbacService::get_user_permissions(&db, &tenant.id, &user.id)
            .await
            .expect("failed to resolve user permissions from rustok-rbac");

        assert!(
            resolved_permissions.contains(&rustok_core::Permission::PRODUCTS_CREATE),
            "manager role permissions should be available"
        );
        assert!(
            !resolved_permissions.contains(&rustok_core::Permission::USERS_MANAGE),
            "manager role should not get admin-only permissions"
        );
    }

    #[tokio::test]
    async fn update_profile_trims_name_and_allows_clearing_with_blank_value() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Profile tenant", "profile-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let user = AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "profile@example.com",
            "Password123!",
            Some("Original Name".to_string()),
            rustok_core::UserRole::Customer,
            None,
        )
        .await
        .expect("failed to create user");

        let updated = AuthLifecycleService::update_profile_db(
            &db,
            tenant.id,
            user.id,
            Some("   Updated Name   ".to_string()),
        )
        .await
        .expect("update_profile should trim name");

        assert_eq!(updated.name.as_deref(), Some("Updated Name"));

        let cleared = AuthLifecycleService::update_profile_db(
            &db,
            tenant.id,
            user.id,
            Some("   ".to_string()),
        )
        .await
        .expect("blank profile name should clear value");

        assert_eq!(cleared.name, None);
    }

    #[tokio::test]
    async fn update_profile_rejects_user_from_another_tenant() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_a = tenants::ActiveModel::new("Tenant A", "tenant-a")
            .insert(&db)
            .await
            .expect("failed to create tenant A");
        let tenant_b = tenants::ActiveModel::new("Tenant B", "tenant-b")
            .insert(&db)
            .await
            .expect("failed to create tenant B");

        let user = AuthLifecycleService::create_user_db(
            &db,
            tenant_a.id,
            "tenant-a@example.com",
            "Password123!",
            Some("Tenant A User".to_string()),
            rustok_core::UserRole::Customer,
            None,
        )
        .await
        .expect("failed to create user in tenant A");

        let result = AuthLifecycleService::update_profile_db(
            &db,
            tenant_b.id,
            user.id,
            Some("Should fail".to_string()),
        )
        .await;

        assert!(matches!(result, Err(AuthLifecycleError::UserNotFound)));
    }

    #[tokio::test]
    async fn user_permissions_are_consistent_for_same_role_across_creation_paths() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Permission Parity Tenant", "permission-parity")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let lifecycle_user = AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "lifecycle@example.com",
            "Password123!",
            Some("Lifecycle User".to_string()),
            rustok_core::UserRole::Manager,
            None,
        )
        .await
        .expect("create_user via lifecycle path should succeed");

        let password_hash = hash_password("Password123!").expect("failed to hash password");
        let legacy_user = users::ActiveModel::new(tenant.id, "legacy@example.com", &password_hash);
        let legacy_user = legacy_user
            .insert(&db)
            .await
            .expect("legacy user insert should succeed");

        RbacService::replace_user_role(
            &db,
            &legacy_user.id,
            &tenant.id,
            rustok_core::UserRole::Manager,
        )
        .await
        .expect("legacy path should assign manager relations");

        let lifecycle_permissions =
            RbacService::get_user_permissions(&db, &tenant.id, &lifecycle_user.id)
                .await
                .expect("failed to fetch permissions for lifecycle user");
        let legacy_permissions =
            RbacService::get_user_permissions(&db, &tenant.id, &legacy_user.id)
                .await
                .expect("failed to fetch permissions for legacy user");

        assert_eq!(
            lifecycle_permissions, legacy_permissions,
            "same role should resolve to identical permissions independent of creation path"
        );
        assert!(
            lifecycle_permissions.contains(&rustok_core::Permission::PRODUCTS_CREATE),
            "manager permission baseline should include PRODUCTS_CREATE"
        );
    }

    #[tokio::test]
    async fn access_token_role_is_derived_from_relations() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Relation Role Tenant", "relation-role-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let password_hash = hash_password("Password123!").expect("failed to hash password");
        let user = users::ActiveModel::new(tenant.id, "relation-role@example.com", &password_hash);
        let user = user.insert(&db).await.expect("user insert should succeed");

        RbacService::replace_user_role(&db, &user.id, &tenant.id, rustok_core::UserRole::Manager)
            .await
            .expect("manager relation assignment should succeed");

        let config = AuthConfig {
            secret: "relation-role-secret".to_string(),
            access_expiration: 600,
            refresh_expiration: 3600,
            issuer: "rustok-test".to_string(),
            audience: "rustok-test".to_string(),
        };

        let tokens = AuthLifecycleService::create_session_and_tokens_db(
            &db, &config, tenant.id, &user, None, None,
        )
        .await
        .expect("token issuance should succeed");

        let claims =
            decode_access_token(&config, &tokens.access_token).expect("access token should decode");

        assert_eq!(tokens.effective_role, rustok_core::UserRole::Manager);
        assert_eq!(claims.role, rustok_core::UserRole::Manager);
    }

    #[tokio::test]
    async fn create_user_applies_default_active_status_and_respects_explicit_status() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Status Tenant", "status-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let default_status_user = AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "default-status@example.com",
            "Password123!",
            Some("Default Status".to_string()),
            rustok_core::UserRole::Customer,
            None,
        )
        .await
        .expect("create_user with default status should succeed");

        assert_eq!(default_status_user.status, rustok_core::UserStatus::Active);

        let explicit_inactive_user = AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "explicit-inactive@example.com",
            "Password123!",
            Some("Explicit Inactive".to_string()),
            rustok_core::UserRole::Customer,
            Some(rustok_core::UserStatus::Inactive),
        )
        .await
        .expect("create_user with explicit inactive status should succeed");

        assert_eq!(
            explicit_inactive_user.status,
            rustok_core::UserStatus::Inactive
        );
    }

    #[tokio::test]
    #[serial]
    async fn create_user_rejects_duplicate_email_with_stable_error_contract() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Duplicate Email Tenant", "duplicate-email-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "dup@example.com",
            "Password123!",
            Some("First User".to_string()),
            rustok_core::UserRole::Customer,
            None,
        )
        .await
        .expect("first create_user should succeed");

        let result = AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "dup@example.com",
            "Password123!",
            Some("Second User".to_string()),
            rustok_core::UserRole::Customer,
            None,
        )
        .await;

        assert!(matches!(
            result,
            Err(AuthLifecycleError::EmailAlreadyExists)
        ));
    }

    #[tokio::test]
    #[serial]
    async fn login_rejects_inactive_user_and_increments_metric() {
        AuthLifecycleService::reset_metrics_for_tests();
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Inactive tenant", "inactive-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let password_hash = hash_password("Password123!").expect("failed to hash password");
        let mut user = users::ActiveModel::new(tenant.id, "inactive@example.com", &password_hash);
        user.status = sea_orm::Set(UserStatus::Inactive);
        user.insert(&db)
            .await
            .expect("failed to create inactive user");

        let config = AuthConfig {
            secret: "test-secret".to_string(),
            access_expiration: 3600,
            refresh_expiration: 3600,
            issuer: "rustok-test".to_string(),
            audience: "rustok-test".to_string(),
        };

        let metrics_before = AuthLifecycleService::metrics_snapshot();
        let result = AuthLifecycleService::login_with_config(
            &db,
            &config,
            tenant.id,
            "inactive@example.com",
            "Password123!",
            None,
            None,
        )
        .await;
        assert!(matches!(result, Err(AuthLifecycleError::UserInactive)));

        let metrics_after = AuthLifecycleService::metrics_snapshot();
        assert!(
            metrics_after.login_inactive_user_attempt_total
                > metrics_before.login_inactive_user_attempt_total
        );
    }

    #[tokio::test]
    async fn refresh_rejects_expired_or_revoked_session() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Expired session tenant", "expired-session-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let password_hash = hash_password("Password123!").expect("failed to hash password");
        let user =
            users::ActiveModel::new(tenant.id, "expired-session@example.com", &password_hash)
                .insert(&db)
                .await
                .expect("failed to create user");

        let expired_token = "expired-refresh-token";
        let mut expired_session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token(expired_token),
            Utc::now() - Duration::minutes(5),
            None,
            None,
        );
        expired_session.revoked_at = Set(Some(Utc::now().into()));
        expired_session
            .insert(&db)
            .await
            .expect("failed to create expired/revoked session");

        let config = AuthConfig {
            secret: "refresh-secret".to_string(),
            access_expiration: 600,
            refresh_expiration: 3600,
            issuer: "rustok-test".to_string(),
            audience: "rustok-test".to_string(),
        };

        let result =
            AuthLifecycleService::refresh_with_config_db(&db, &config, tenant.id, expired_token)
                .await;

        assert!(matches!(result, Err(AuthLifecycleError::SessionExpired)));
    }

    #[tokio::test]
    async fn refresh_rejects_unknown_refresh_token() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Unknown refresh tenant", "unknown-refresh-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let config = AuthConfig {
            secret: "refresh-secret".to_string(),
            access_expiration: 600,
            refresh_expiration: 3600,
            issuer: "rustok-test".to_string(),
            audience: "rustok-test".to_string(),
        };

        let result = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant.id,
            "unknown-refresh-token",
        )
        .await;

        assert!(matches!(
            result,
            Err(AuthLifecycleError::InvalidRefreshToken)
        ));
    }

    #[tokio::test]
    async fn refresh_rejects_inactive_user() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant =
            tenants::ActiveModel::new("Refresh inactive tenant", "refresh-inactive-tenant")
                .insert(&db)
                .await
                .expect("failed to create tenant");

        let password_hash = hash_password("Password123!").expect("failed to hash password");
        let mut user =
            users::ActiveModel::new(tenant.id, "refresh-inactive@example.com", &password_hash);
        user.status = Set(UserStatus::Inactive);
        let user = user
            .insert(&db)
            .await
            .expect("failed to create inactive user");

        let refresh_token = "inactive-user-refresh-token";
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token(refresh_token),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create active session for inactive user");

        let config = AuthConfig {
            secret: "refresh-secret".to_string(),
            access_expiration: 600,
            refresh_expiration: 3600,
            issuer: "rustok-test".to_string(),
            audience: "rustok-test".to_string(),
        };

        let result =
            AuthLifecycleService::refresh_with_config_db(&db, &config, tenant.id, refresh_token)
                .await;

        assert!(matches!(result, Err(AuthLifecycleError::UserInactive)));
    }

    #[tokio::test]
    async fn confirm_password_reset_rejects_invalid_token_payload() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant A", "tenant-a")
            .insert(&db)
            .await
            .expect("failed to create tenant A");

        let password_hash = hash_password("OldPassword123!").expect("failed to hash password");
        users::ActiveModel::new(tenant.id, "tenant-a-user@example.com", &password_hash)
            .insert(&db)
            .await
            .expect("failed to create user");

        let config = AuthConfig {
            secret: "reset-secret".to_string(),
            access_expiration: 3600,
            refresh_expiration: 7200,
            issuer: "rustok-test".to_string(),
            audience: "rustok-test".to_string(),
        };

        let err = AuthLifecycleService::confirm_password_reset_with_config(
            &db,
            &config,
            tenant.id,
            "not-a-jwt",
            "NewPassword123!",
        )
        .await
        .expect_err("invalid token payload must be rejected");

        assert!(matches!(err, AuthLifecycleError::InvalidResetToken));
    }

    #[tokio::test]
    #[serial]
    async fn reset_password_and_revoke_sessions_updates_password_and_revokes_all_sessions() {
        AuthLifecycleService::reset_metrics_for_tests();
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant reset", "tenant-reset-positive")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let old_password = "OldPassword123!";
        let new_password = "NewPassword123!";
        let old_hash = hash_password(old_password).expect("failed to hash old password");
        let user = users::ActiveModel::new(tenant.id, "reset-positive@example.com", &old_hash)
            .insert(&db)
            .await
            .expect("failed to create user");

        let now = Utc::now();
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "reset-positive-token-1".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create first session");
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "reset-positive-token-2".to_string(),
            now + Duration::hours(2),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create second session");

        let metrics_before = AuthLifecycleService::metrics_snapshot();

        AuthLifecycleService::reset_password_and_revoke_sessions(
            &db,
            tenant.id,
            user.clone(),
            new_password,
            None,
        )
        .await
        .expect("reset with revoke should succeed");

        let updated_user = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("failed to query updated user")
            .expect("updated user should exist");
        assert!(verify_password(new_password, &updated_user.password_hash)
            .expect("new password should verify"));
        assert!(!verify_password(old_password, &updated_user.password_hash)
            .expect("old password must not verify"));

        let active_sessions = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .count(&db)
            .await
            .expect("failed to query active sessions");
        assert_eq!(active_sessions, 0);

        let metrics_after = AuthLifecycleService::metrics_snapshot();
        assert!(
            metrics_after.password_reset_sessions_revoked_total
                >= metrics_before.password_reset_sessions_revoked_total + 2
        );
    }

    #[tokio::test]
    async fn revoke_user_sessions_repeat_call_is_idempotent() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant Revoke", "tenant-revoke-repeat")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let user = users::ActiveModel::new(tenant.id, "revoke-repeat@example.com", "hash")
            .insert(&db)
            .await
            .expect("failed to create user");

        let now = Utc::now();
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "revoke-repeat-token-1".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create first session");
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "revoke-repeat-token-2".to_string(),
            now + Duration::hours(2),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create second session");

        let first = AuthLifecycleService::revoke_user_sessions_db(&db, tenant.id, user.id, None)
            .await
            .expect("first revoke should succeed");
        let second = AuthLifecycleService::revoke_user_sessions_db(&db, tenant.id, user.id, None)
            .await
            .expect("second revoke should succeed");

        assert_eq!(first, 2);
        assert_eq!(second, 0);
    }

    #[tokio::test]
    #[serial]
    async fn revoke_user_sessions_is_strictly_scoped_by_tenant_and_user() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_a = tenants::ActiveModel::new("Tenant A", "tenant-a-revoke")
            .insert(&db)
            .await
            .expect("failed to create tenant A");
        let tenant_b = tenants::ActiveModel::new("Tenant B", "tenant-b-revoke")
            .insert(&db)
            .await
            .expect("failed to create tenant B");

        let user_a = users::ActiveModel::new(tenant_a.id, "user-a@example.com", "hash-a")
            .insert(&db)
            .await
            .expect("failed to create user A");
        let user_b = users::ActiveModel::new(tenant_b.id, "user-b@example.com", "hash-b")
            .insert(&db)
            .await
            .expect("failed to create user B");

        let now = Utc::now();
        sessions::ActiveModel::new(
            tenant_a.id,
            user_a.id,
            "tenant-a-token".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create tenant A session");
        sessions::ActiveModel::new(
            tenant_b.id,
            user_b.id,
            "tenant-b-token".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create tenant B session");

        let revoked_for_a =
            AuthLifecycleService::revoke_user_sessions_db(&db, tenant_a.id, user_a.id, None)
                .await
                .expect("revoke for tenant A should succeed");

        assert_eq!(revoked_for_a, 1);

        let tenant_a_active_sessions = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant_a.id))
            .filter(sessions::Column::UserId.eq(user_a.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .count(&db)
            .await
            .expect("failed to query tenant A active sessions");
        assert_eq!(tenant_a_active_sessions, 0);

        let tenant_b_active_sessions = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant_b.id))
            .filter(sessions::Column::UserId.eq(user_b.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .count(&db)
            .await
            .expect("failed to query tenant B active sessions");
        assert_eq!(tenant_b_active_sessions, 1);
    }

    #[tokio::test]
    async fn revoke_user_sessions_with_except_session_id_keeps_only_requested_session_active() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Except Session Tenant", "except-session-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let user = users::ActiveModel::new(tenant.id, "user@example.com", "hash")
            .insert(&db)
            .await
            .expect("failed to create user");

        let now = Utc::now();
        let keep_session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "keep-token".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create keep session");

        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "revoke-token".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create revoke session");

        let revoked = AuthLifecycleService::revoke_user_sessions_db(
            &db,
            tenant.id,
            user.id,
            Some(keep_session.id),
        )
        .await
        .expect("revoke_user_sessions should succeed");

        assert_eq!(revoked, 1);

        let active_sessions = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .all(&db)
            .await
            .expect("failed to query active sessions");

        assert_eq!(active_sessions.len(), 1);
        assert_eq!(active_sessions[0].id, keep_session.id);
    }

    #[tokio::test]
    #[serial]
    async fn reset_password_revoke_sessions_can_keep_current_session() {
        AuthLifecycleService::reset_metrics_for_tests();
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Test tenant", "test-tenant-2")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let user = users::ActiveModel::new(tenant.id, "user2@example.com", "old-hash")
            .insert(&db)
            .await
            .expect("failed to create user");

        let now = Utc::now();
        let current_session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "current-token".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create current session");
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "other-token".to_string(),
            now + Duration::hours(2),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create secondary session");

        let metrics_before = AuthLifecycleService::metrics_snapshot();

        AuthLifecycleService::reset_password_and_revoke_sessions(
            &db,
            tenant.id,
            user.clone(),
            "new-password",
            Some(current_session.id),
        )
        .await
        .expect("password reset should succeed");

        let still_active = sessions::Entity::find_by_id(current_session.id)
            .one(&db)
            .await
            .expect("failed to fetch current session")
            .expect("current session should exist");
        assert!(still_active.revoked_at.is_none());

        let revoked_count = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::RevokedAt.is_not_null())
            .count(&db)
            .await
            .expect("failed to query revoked sessions");

        assert_eq!(revoked_count, 1);

        let metrics_after = AuthLifecycleService::metrics_snapshot();
        assert!(
            metrics_after.password_reset_sessions_revoked_total
                > metrics_before.password_reset_sessions_revoked_total
        );
    }

    #[tokio::test]
    #[serial]
    async fn reset_password_revoke_sessions_repeat_call_on_already_revoked_sessions_is_idempotent()
    {
        AuthLifecycleService::reset_metrics_for_tests();
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Test tenant", "test-tenant-3")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let user = users::ActiveModel::new(tenant.id, "user3@example.com", "old-hash")
            .insert(&db)
            .await
            .expect("failed to create user");
        let user_id = user.id;

        let now = Utc::now();
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "token-a".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create session a");
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "token-b".to_string(),
            now + Duration::hours(2),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create session b");

        AuthLifecycleService::reset_password_and_revoke_sessions(
            &db,
            tenant.id,
            user.clone(),
            "new-password",
            None,
        )
        .await
        .expect("first password reset should succeed");

        let revoked_after_first_call = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user_id))
            .filter(sessions::Column::RevokedAt.is_not_null())
            .count(&db)
            .await
            .expect("failed to query revoked sessions after first call");
        assert_eq!(
            revoked_after_first_call, 2,
            "first call should revoke all active sessions"
        );

        AuthLifecycleService::reset_password_and_revoke_sessions(
            &db,
            tenant.id,
            user,
            "new-password-2",
            None,
        )
        .await
        .expect("second password reset should succeed");

        let revoked_after_second_call = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user_id))
            .filter(sessions::Column::RevokedAt.is_not_null())
            .count(&db)
            .await
            .expect("failed to query revoked sessions after second call");
        assert_eq!(
            revoked_after_second_call, revoked_after_first_call,
            "second call should revoke zero additional sessions"
        );
    }

    #[tokio::test]
    async fn create_user_scopes_duplicate_email_check_per_tenant() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_a = tenants::ActiveModel::new("Tenant A", "tenant-a-scope")
            .insert(&db)
            .await
            .expect("failed to create tenant A");
        let tenant_b = tenants::ActiveModel::new("Tenant B", "tenant-b-scope")
            .insert(&db)
            .await
            .expect("failed to create tenant B");

        AuthLifecycleService::create_user_db(
            &db,
            tenant_a.id,
            "shared@example.com",
            "Password123!",
            Some("Tenant A User".to_string()),
            rustok_core::UserRole::Customer,
            None,
        )
        .await
        .expect("create_user in tenant A should succeed");

        AuthLifecycleService::create_user_db(
            &db,
            tenant_b.id,
            "shared@example.com",
            "Password123!",
            Some("Tenant B User".to_string()),
            rustok_core::UserRole::Manager,
            None,
        )
        .await
        .expect("same email in another tenant should be allowed");

        let tenant_a_users = users::Entity::find()
            .filter(users::Column::TenantId.eq(tenant_a.id))
            .filter(users::Column::Email.eq("shared@example.com"))
            .count(&db)
            .await
            .expect("failed to query users in tenant A");
        assert_eq!(tenant_a_users, 1);

        let tenant_b_users = users::Entity::find()
            .filter(users::Column::TenantId.eq(tenant_b.id))
            .filter(users::Column::Email.eq("shared@example.com"))
            .count(&db)
            .await
            .expect("failed to query users in tenant B");
        assert_eq!(tenant_b_users, 1);
    }
}
