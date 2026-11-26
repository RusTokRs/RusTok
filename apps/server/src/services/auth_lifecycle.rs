use crate::error::Error;
use rustok_auth::AuthRefreshRetryKey;
use rustok_events::DomainEvent;

use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set, Statement,
    TransactionTrait, sea_query::Expr,
};

use crate::auth::{
    AuthConfig, generate_refresh_token, hash_password_async,
    hash_refresh_token, normalize_display_name, normalize_email, password_hash_needs_rehash,
    validate_password_policy, verify_password_async, MAX_PASSWORD_LENGTH_BYTES,
};
use crate::models::{_entities::session_refresh_history, auth_email_change_requests, sessions, users};
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::services::server_runtime_context::ServerRuntimeContext;
use std::sync::{
    atomic::{AtomicU64, Ordering},
};
use tracing::instrument;

use super::rbac_service::RbacService;

pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    /// Remaining lifetime of the persisted refresh session, bounded by its
    /// original absolute deadline. Used for the browser cookie Max-Age.
    pub refresh_expires_in: u64,
    pub effective_role: rustok_core::UserRole,
    /// Effective RBAC permission snapshot (`"{resource}:{action}"` strings)
    /// resolved on the same transaction that issued the session, so every
    /// user payload returned at authentication time carries the grants the
    /// backend will enforce for the session. Display-level only: request
    /// authorization re-resolves per-request.
    pub permissions: Vec<String>,
}

// Keep the durable deadline until the last fallible step before commit;
// computing a cookie lifetime before database and policy work overstates it.
struct PreparedSessionTokens {
    access_token: String,
    refresh_token: String,
    access_deadline: chrono::DateTime<Utc>,
    refresh_deadline: chrono::DateTime<Utc>,
    effective_role: rustok_core::UserRole,
    permissions: Vec<String>,
}

impl PreparedSessionTokens {
    fn ensure_live(&self, now: chrono::DateTime<Utc>) -> Result<(), AuthLifecycleError> {
        AuthLifecycleService::remaining_refresh_lifetime(self.refresh_deadline, now)?;
        AuthLifecycleService::remaining_refresh_lifetime(self.access_deadline, now)?;
        Ok(())
    }

    fn finish(self, now: chrono::DateTime<Utc>) -> Result<AuthTokens, AuthLifecycleError> {
        let refresh_expires_in =
            AuthLifecycleService::remaining_refresh_lifetime(self.refresh_deadline, now)?;
        let expires_in =
            AuthLifecycleService::remaining_refresh_lifetime(self.access_deadline, now)?;
        Ok(AuthTokens {
            access_token: self.access_token,
            refresh_token: self.refresh_token,
            expires_in,
            refresh_expires_in,
            effective_role: self.effective_role,
            permissions: self.permissions,
        })
    }
}

#[derive(Debug)]
pub enum AuthLifecycleError {
    EmailAlreadyExists,
    InvalidCredentials,
    UserInactive,
    AccountLocked,
    EmailNotVerified,
    InvalidRefreshToken,
    RefreshAlreadyRotated,
    SessionLineageMismatch,
    SessionExpired,
    UserNotFound,
    InvalidResetToken,
    InvalidVerificationToken,
    InvalidEmailChangeToken,
    EmailDeliveryUnavailable,
    Validation(String),
    Internal(Error),
}

impl std::fmt::Display for AuthLifecycleError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthLifecycleError::EmailAlreadyExists => formatter.write_str("email already exists"),
            AuthLifecycleError::InvalidCredentials => formatter.write_str("invalid credentials"),
            AuthLifecycleError::UserInactive => formatter.write_str("user is inactive"),
            AuthLifecycleError::AccountLocked => {
                formatter.write_str("account is temporarily locked")
            }
            AuthLifecycleError::EmailNotVerified => {
                formatter.write_str("email address is not verified")
            }
            AuthLifecycleError::InvalidRefreshToken => formatter.write_str("invalid refresh token"),
            AuthLifecycleError::RefreshAlreadyRotated => {
                formatter.write_str("refresh token was already rotated by another request")
            },
            AuthLifecycleError::SessionLineageMismatch => formatter.write_str("session lineage mismatch"),
            AuthLifecycleError::SessionExpired => formatter.write_str("session expired"),
            AuthLifecycleError::UserNotFound => formatter.write_str("user not found"),
            AuthLifecycleError::InvalidResetToken => formatter.write_str("invalid reset token"),
            AuthLifecycleError::InvalidVerificationToken => {
                formatter.write_str("invalid verification token")
            }
            AuthLifecycleError::InvalidEmailChangeToken => {
                formatter.write_str("invalid or expired email change token")
            }
            AuthLifecycleError::EmailDeliveryUnavailable => {
                formatter.write_str("email delivery is not configured")
            }
            AuthLifecycleError::Validation(message) => formatter.write_str(message),
            AuthLifecycleError::Internal(err) => write!(formatter, "{err}"),
        }
    }
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
        match value {
            rustok_auth::AuthError::Validation(message) => Self::Validation(message),
            other => Self::Internal(crate::auth::auth_err(other)),
        }
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
            AuthLifecycleError::AccountLocked => {
                Error::Unauthorized("Account is temporarily locked".into())
            }
            AuthLifecycleError::EmailNotVerified => {
                Error::Unauthorized("Email address is not verified".into())
            }
            AuthLifecycleError::InvalidRefreshToken => {
                Error::Unauthorized("Invalid refresh token".into())
            }
            AuthLifecycleError::RefreshAlreadyRotated => Error::Conflict(
                "Refresh token was already rotated by another request".into(),
            ),
            AuthLifecycleError::SessionLineageMismatch => Error::Conflict("Session lineage mismatch".into()),
            AuthLifecycleError::SessionExpired => Error::Unauthorized("Session expired".into()),
            AuthLifecycleError::UserNotFound => Error::Unauthorized("User not found".into()),
            AuthLifecycleError::InvalidResetToken => {
                Error::Unauthorized("Invalid reset token".into())
            }
            AuthLifecycleError::InvalidVerificationToken => {
                Error::Unauthorized("Invalid verification token".into())
            }
            AuthLifecycleError::InvalidEmailChangeToken => {
                Error::Unauthorized("Invalid or expired email change token".into())
            }
            AuthLifecycleError::EmailDeliveryUnavailable => {
                Error::Message("Email delivery is not configured".into())
            }
            AuthLifecycleError::Validation(message) => Error::BadRequest(message),
            AuthLifecycleError::Internal(err) => err,
        }
    }
}

/// New-account credentials after canonical validation and Argon2
/// hashing (see `AuthLifecycleService::prepare_new_user`). Fields are
/// crate-private so the only way to obtain one is through that
/// validation point.
pub(crate) struct PreparedNewUser {
    email: String,
    name: Option<String>,
    password_hash: String,
}

impl std::fmt::Debug for PreparedNewUser {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedNewUser")
            .field("email", &self.email)
            .field("name", &self.name)
            .field("password_hash", &"<redacted>")
            .finish()
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

const MAX_SESSION_LIST_LIMIT: u64 = 100;

/// Consecutive failed credential checks after which an account locks.
/// Combined with Argon2 cost and IP rate limiting, this bounds online
/// guessing per account across every transport and source address.
const MAX_FAILED_LOGIN_ATTEMPTS: i32 = 10;

/// How long an account stays locked once the failure threshold is hit.
/// A successful password reset or password change clears the lock early.
const LOGIN_LOCKOUT_MINUTES: i64 = 15;

/// Only a retry proving the original operation can recover its still-current
/// successor in this window. Other duplicates conflict without revocation;
/// historical credentials replayed outside the window revoke the lineage.
const REFRESH_REUSE_GRACE_SECONDS: i64 = 30;

impl AuthLifecycleService {
    /// Fresh DB corroboration on the same connection, then the live JWT gate.
    async fn credential_now(db: &impl ConnectionTrait, config: &AuthConfig)
        -> Result<chrono::DateTime<Utc>, AuthLifecycleError>
    {
        let verified = rustok_auth::application_clock::verified_now(db).await?;
        let live = config.credential_now()?;
        Ok(rustok_auth::application_clock::decision_time(verified, live)?)
    }

    fn response_now(config: &AuthConfig) -> Result<chrono::DateTime<Utc>, AuthLifecycleError> {
        rustok_auth::application_clock::check_local()?;
        Ok(config.credential_now()?)
    }

    /// Acquire the tenant-scoped user mutation fence before reading the current
    /// row. PostgreSQL/MySQL use a row lock; SQLite reserves its writer before
    /// the read so a deferred read-to-write upgrade cannot race another user
    /// status, role, or credential mutation.
    pub(crate) async fn lock_user_for_mutation<C>(
        db: &C,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<Option<users::Model>, sea_orm::DbErr>
    where
        C: ConnectionTrait,
    {
        let query = || {
            users::Entity::find_by_id(user_id).filter(users::Column::TenantId.eq(tenant_id))
        };

        match db.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => query()
                .lock_exclusive()
                .one(db)
                .await,
            DatabaseBackend::Sqlite => {
                let result = users::Entity::update_many()
                    .col_expr(
                        users::Column::UpdatedAt,
                        Expr::col(users::Column::UpdatedAt),
                    )
                    .filter(users::Column::Id.eq(user_id))
                    .filter(users::Column::TenantId.eq(tenant_id))
                    .exec(db)
                    .await?;
                if result.rows_affected != 1 {
                    return Ok(None);
                }
                query().one(db).await
            }
            _ => Err(sea_orm::DbErr::Custom(
                "unsupported database backend for user mutation locking".to_string(),
            )),
        }
    }

    /// Acquire the existing user mutation fence, then check the signed
    /// recovery deadline. The returned timestamp is the admission instant,
    /// not a timestamp captured before hashing or waiting for a row lock.
    pub(crate) async fn lock_user_for_recovery(
        tx: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        expires_at: chrono::DateTime<Utc>,
    ) -> Result<Option<(users::Model, chrono::DateTime<Utc>)>, sea_orm::DbErr> {
        let user = Self::lock_user_for_mutation(tx, tenant_id, user_id).await?;
        let now = rustok_auth::application_clock::verified_now(tx).await?;
        Ok(user.filter(|_| now < expires_at).map(|user| (user, now)))
    }

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

    pub fn record_password_reset_sessions_revoked(count: u64) {
        AUTH_PASSWORD_RESET_SESSIONS_REVOKED_TOTAL.fetch_add(count, Ordering::Relaxed);
    }

    /// Publish a user audit event inside the caller's transaction so
    /// the event is durably recorded exactly when the state change it
    /// describes commits (transactional outbox).
    #[instrument(skip(ctx, tx, event))]
    pub(crate) async fn publish_user_event_in_tx(
        ctx: &ServerRuntimeContext,
        tx: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        event: DomainEvent,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let event_bus = transactional_event_bus_from_context(ctx);
        event_bus
            .publish_in_tx(tx, tenant_id, Some(user_id), event)
            .await
            .map_err(|error| AuthLifecycleError::Internal(error.into()))
    }

    #[cfg(test)]
    fn reset_metrics_for_tests() {
        AUTH_PASSWORD_RESET_SESSIONS_REVOKED_TOTAL.store(0, Ordering::Relaxed);
        AUTH_CHANGE_PASSWORD_SESSIONS_REVOKED_TOTAL.store(0, Ordering::Relaxed);
        AUTH_FLOW_INCONSISTENCY_TOTAL.store(0, Ordering::Relaxed);
        AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL.store(0, Ordering::Relaxed);
    }

    #[instrument(skip(ctx, password, email, name))]
    pub async fn create_user_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        name: Option<String>,
        role: rustok_core::UserRole,
        status: Option<rustok_core::UserStatus>,
        min_password_length: usize,
    ) -> std::result::Result<users::Model, AuthLifecycleError> {
        Self::create_user_db(
            ctx.db(),
            tenant_id,
            email,
            password,
            name,
            role,
            status,
            min_password_length,
        )
        .await
    }

    #[instrument(skip(db, password, email, name))]
    pub(crate) async fn create_user_db(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        name: Option<String>,
        role: rustok_core::UserRole,
        status: Option<rustok_core::UserStatus>,
        min_password_length: usize,
    ) -> std::result::Result<users::Model, AuthLifecycleError> {
        let prepared = Self::prepare_new_user(email, password, name, min_password_length).await?;
        let tx = db.begin().await.map_err(AuthLifecycleError::from)?;

        let user = Self::create_user_in_tx(&tx, tenant_id, prepared, role, status).await?;

        tx.commit().await.map_err(AuthLifecycleError::from)?;

        Ok(user)
    }

    /// Validate and hash new-account credentials *before* any database
    /// transaction begins.
    ///
    /// This is the single canonical validation point for every
    /// account-creation path (registration, admin provisioning, invite
    /// acceptance, bootstrap seeds): no caller can persist an
    /// unnormalized email, an over-long display name, or a password
    /// below policy, because `create_user_in_tx` only accepts the
    /// prepared result. Keeping Argon2 (tens of milliseconds on a
    /// blocking thread) outside the transaction also means account
    /// creation no longer holds a pooled connection and an open
    /// transaction — with its locks — for the duration of the hash.
    pub(crate) async fn prepare_new_user(
        email: &str,
        password: &str,
        name: Option<String>,
        min_password_length: usize,
    ) -> std::result::Result<PreparedNewUser, AuthLifecycleError> {
        let email = normalize_email(email).map_err(AuthLifecycleError::from)?;
        let name = normalize_display_name(name.as_deref()).map_err(AuthLifecycleError::from)?;
        validate_password_policy(password, min_password_length)
            .map_err(AuthLifecycleError::from)?;

        let password_hash = hash_password_async(password.to_string())
            .await
            .map_err(AuthLifecycleError::from)?;

        Ok(PreparedNewUser {
            email,
            name,
            password_hash,
        })
    }

    #[instrument(skip(db, prepared))]
    pub(crate) async fn create_user_in_tx<C>(
        db: &C,
        tenant_id: uuid::Uuid,
        prepared: PreparedNewUser,
        role: rustok_core::UserRole,
        status: Option<rustok_core::UserStatus>,
    ) -> std::result::Result<users::Model, AuthLifecycleError>
    where
        C: ConnectionTrait,
    {
        if users::Entity::find()
            .filter(users::Column::TenantId.eq(tenant_id))
            .filter(users::Column::Email.eq(prepared.email.as_str()))
            .one(db)
            .await
            .map_err(AuthLifecycleError::from)?
            .is_some()
        {
            return Err(AuthLifecycleError::EmailAlreadyExists);
        }

        let mut user = users::ActiveModel::new(tenant_id, &prepared.email, &prepared.password_hash);
        user.name = Set(prepared.name);
        if let Some(status) = status {
            user.status = Set(status);
        }

        let user = match user.insert(db).await {
            Ok(user) => user,
            Err(err) => {
                // Do not re-SELECT here: on PostgreSQL the failed INSERT
                // aborts the transaction, so a follow-up query errors out
                // and the duplicate-email race degrades into a 500. The
                // unique (tenant_id, email) constraint is the authority.
                if matches!(
                    err.sql_err(),
                    Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
                ) {
                    return Err(AuthLifecycleError::EmailAlreadyExists);
                }

                return Err(AuthLifecycleError::from(err));
            }
        };

        RbacService::replace_user_role(db, &user.id, &tenant_id, role)
            .await
            .map_err(AuthLifecycleError::from)?;

        Ok(user)
    }

    /// Deactivate a tenant user without erasing records referenced by audits,
    /// orders, sessions, or permissions. Session revocation is coordinated by
    /// the administrative mutation in the same transaction.
    pub(crate) async fn deactivate_user_in_tx<C>(
        db: &C,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> std::result::Result<(), AuthLifecycleError>
    where
        C: ConnectionTrait,
    {
        let user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::UserNotFound)?;

        let mut active: users::ActiveModel = user.into();
        active.status = Set(rustok_core::UserStatus::Inactive);
        active.update(db).await.map_err(AuthLifecycleError::from)?;
        Ok(())
    }

    #[instrument(skip(ctx, name))]
    pub async fn update_profile_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        name: Option<String>,
    ) -> std::result::Result<users::Model, AuthLifecycleError> {
        Self::update_profile_db(ctx, tenant_id, user_id, name).await
    }

    async fn update_profile_db(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        name: Option<String>,
    ) -> std::result::Result<users::Model, AuthLifecycleError> {
        let tx = ctx.db().begin().await.map_err(AuthLifecycleError::from)?;
        let user = Self::find_user_for_update_in_tx(&tx, tenant_id, user_id)
            .await?
            .ok_or(AuthLifecycleError::UserNotFound)?;

        let next_name = match name {
            Some(value) => {
                Some(normalize_display_name(Some(&value)).map_err(AuthLifecycleError::from)?)
            }
            None => None,
        };
        let name_changed = next_name.as_ref().is_some_and(|next| user.name != *next);

        let mut user_active: users::ActiveModel = user.into();
        if let Some(next_name) = next_name {
            user_active.name = Set(next_name);
        }

        let updated = user_active
            .update(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;

        if name_changed {
            Self::publish_user_event_in_tx(
                ctx,
                &tx,
                tenant_id,
                user_id,
                DomainEvent::UserUpdated { user_id },
            )
            .await?;
        }

        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(updated)
    }

    pub async fn register_runtime(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        name: Option<String>,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        let prepared =
            Self::prepare_new_user(email, password, name, config.min_password_length).await?;
        rustok_auth::application_clock::admit_now(ctx.db()).await?;
        let tx = ctx.db().begin().await.map_err(AuthLifecycleError::from)?;
        let user = Self::create_user_in_tx(
            &tx,
            tenant_id,
            prepared,
            rustok_core::UserRole::Customer,
            None,
        )
        .await?;
        let prepared_tokens =
            Self::create_session_and_tokens_in_tx(&tx, config, tenant_id, &user, None, None)
                .await?;

        Self::publish_user_event_in_tx(
            ctx,
            &tx,
            tenant_id,
            user.id,
            DomainEvent::UserAccountRegistered { user_id: user.id },
        )
        .await?;

        prepared_tokens.ensure_live(Self::credential_now(&tx, config).await?)?;
        tx.commit().await.map_err(AuthLifecycleError::from)?;

        Ok((user, prepared_tokens.finish(Self::response_now(config)?)?))
    }

    pub async fn login_runtime(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
        require_email_verification: bool,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        Self::login_with_config(
            ctx,
            config,
            tenant_id,
            email,
            password,
            ip_address,
            user_agent,
            require_email_verification,
        )
        .await
    }

    async fn login_with_config(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        email: &str,
        password: &str,
        ip_address: Option<String>,
        user_agent: Option<String>,
        require_email_verification: bool,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        let db = ctx.db();
        // Gate before lookup so a clock outage cannot distinguish known emails.
        rustok_auth::application_clock::admit_now(db).await?;
        let admitted_at = Self::credential_now(db, config).await?;
        // Lookup-only canonicalization: login must not reject malformed
        // input with a distinct validation error (that would be an account
        // enumeration oracle), it simply finds no user and fails as
        // InvalidCredentials below.
        let email = email.trim().to_lowercase();
        let user = match users::Entity::find_by_email(db, tenant_id, &email)
            .await
            .map_err(AuthLifecycleError::from)?
        {
            Some(user) => user,
            None => {
                // Run a full Argon2 verification against a dummy hash so
                // an unknown email takes as long as a wrong password:
                // without this, login latency enumerates accounts.
                crate::auth::mask_auth_lookup_timing()
                    .await
                    .map_err(AuthLifecycleError::from)?;
                return Err(AuthLifecycleError::InvalidCredentials);
            }
        };

        // A locked account must not expose whether a submitted password is
        // correct. Failed attempts are no longer counted during the lockout,
        // so verifying the real hash here would create an unlimited password
        // oracle (InvalidCredentials vs AccountLocked). Match the unknown-user
        // path with the same dummy hash and generic failure.
        if Self::is_login_locked_at(&user, admitted_at) {
            crate::auth::mask_auth_lookup_timing()
                .await
                .map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::InvalidCredentials);
        }

        let needs_timing_mask =
            Self::requires_password_timing_mask(password, &user.password_hash);
        let password_matches = if needs_timing_mask {
            // Run the real verifier and a fixed-cost dummy verifier concurrently.
            // Sequential padding would make weak legacy hashes slower than both
            // current hashes and unknown-account lookups, preserving a timing signal.
            let (password_matches, timing_mask) = tokio::join!(
                verify_password_async(password.to_string(), user.password_hash.clone()),
                crate::auth::mask_auth_lookup_timing(),
            );
            timing_mask.map_err(AuthLifecycleError::from)?;
            password_matches.map_err(AuthLifecycleError::from)?
        } else {
            verify_password_async(password.to_string(), user.password_hash.clone())
                .await
                .map_err(AuthLifecycleError::from)?
        };
        if !password_matches {
            Self::register_failed_login_attempt(db, tenant_id, &user).await?;
            return Err(AuthLifecycleError::InvalidCredentials);
        }

        if !user.is_active() {
            AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL.fetch_add(1, Ordering::Relaxed);
            return Err(AuthLifecycleError::UserInactive);
        }

        if require_email_verification && !user.is_email_verified() {
            return Err(AuthLifecycleError::EmailNotVerified);
        }

        // Transparently upgrade a stored hash whose Argon2id parameters
        // predate the current target: the plaintext is only available here,
        // at a successful verification. The new hash is computed before the
        // transaction opens and applied inside it only after the locked row
        // proves the credential did not rotate in the meantime.
        let upgraded_password_hash = if password_hash_needs_rehash(&user.password_hash) {
            Some(
                hash_password_async(password.to_string())
                    .await
                    .map_err(AuthLifecycleError::from)?,
            )
        } else {
            None
        };

        let txn = db.begin().await.map_err(AuthLifecycleError::from)?;
        let locked_user = Self::find_user_for_login_in_tx(&txn, tenant_id, &email)
            .await?
            .ok_or(AuthLifecycleError::InvalidCredentials)?;

        if locked_user.password_hash != user.password_hash {
            return Err(AuthLifecycleError::InvalidCredentials);
        }
        let now = Self::credential_now(&txn, config).await?;
        if Self::is_login_locked_at(&locked_user, now) {
            // The lock may have been activated after the initial user lookup.
            // Keep the result identical to the wrong-password path.
            return Err(AuthLifecycleError::InvalidCredentials);
        }
        if !locked_user.is_active() {
            AUTH_LOGIN_INACTIVE_USER_ATTEMPT_TOTAL.fetch_add(1, Ordering::Relaxed);
            return Err(AuthLifecycleError::UserInactive);
        }
        if require_email_verification && !locked_user.is_email_verified() {
            return Err(AuthLifecycleError::EmailNotVerified);
        }

        let mut user_active: users::ActiveModel = locked_user.into();
        if let Some(upgraded_password_hash) = upgraded_password_hash {
            user_active.password_hash = Set(upgraded_password_hash);
        }
        user_active.last_login_at = Set(Some(now.into()));
        user_active.failed_login_count = Set(0);
        user_active.locked_until = Set(None);
        let user = user_active
            .update(&txn)
            .await
            .map_err(AuthLifecycleError::from)?;

        let prepared_tokens = Self::create_session_and_tokens_in_tx(
            &txn, config, tenant_id, &user, ip_address, user_agent,
        )
        .await?;
        Self::publish_user_event_in_tx(
            ctx,
            &txn,
            tenant_id,
            user.id,
            DomainEvent::UserLoggedIn { user_id: user.id },
        )
        .await?;
        prepared_tokens.ensure_live(Self::credential_now(&txn, config).await?)?;
        txn.commit().await.map_err(AuthLifecycleError::from)?;

        Ok((user, prepared_tokens.finish(Self::response_now(config)?)?))
    }

    #[cfg(test)]
    fn is_login_locked(user: &users::Model) -> bool {
        Self::is_login_locked_at(user, Utc::now())
    }

    fn requires_password_timing_mask(password: &str, password_hash: &str) -> bool {
        password.is_empty()
            || password.len() > MAX_PASSWORD_LENGTH_BYTES
            || password_hash_needs_rehash(password_hash)
    }

    fn is_login_locked_at(user: &users::Model, now: chrono::DateTime<Utc>) -> bool {
        user.locked_until
            .as_ref()
            .is_some_and(|locked_until| locked_until.with_timezone(&Utc) > now)
    }

    /// Serialize failed attempts with successful login and credential changes.
    /// A failure verified against an old password must not lock the new owner.
    async fn register_failed_login_attempt(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        observed_user: &users::Model,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let tx = db.begin().await.map_err(AuthLifecycleError::from)?;
        let user =
            Self::find_user_for_update_in_tx(&tx, tenant_id, observed_user.id).await?;
        let now = rustok_auth::application_clock::verified_now(&tx).await?;
        if let Some(user) = user
            .filter(|u| u.password_hash == observed_user.password_hash && !Self::is_login_locked_at(u, now))
        {
            let attempts = user.failed_login_count.saturating_add(1);
            let mut active: users::ActiveModel = user.into();
            if attempts >= MAX_FAILED_LOGIN_ATTEMPTS {
                active.failed_login_count = Set(0);
                active.locked_until = Set(Some(
                    now.checked_add_signed(Duration::minutes(LOGIN_LOCKOUT_MINUTES))
                        .ok_or(AuthLifecycleError::Internal(Error::InternalServerError))?.into(),
                ));
            } else {
                active.failed_login_count = Set(attempts);
            }
            active.update(&tx).await.map_err(AuthLifecycleError::from)?;
        }
        rustok_auth::application_clock::verified_now(&tx).await?;
        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(())
    }

    pub async fn refresh_runtime(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        refresh_token: &str,
        require_email_verification: bool,
        retry_key: Option<&AuthRefreshRetryKey>,
        expected_session_id: Option<uuid::Uuid>,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        Self::refresh_with_config_db(
            ctx.db(),
            config,
            tenant_id,
            refresh_token,
            require_email_verification,
            retry_key,
            expected_session_id,
        )
        .await
    }

    async fn find_user_for_login_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        email: &str,
    ) -> std::result::Result<Option<users::Model>, AuthLifecycleError> {
        let normalized_email = email.to_lowercase();
        let query = users::Entity::find()
            .filter(users::Column::TenantId.eq(tenant_id))
            .filter(users::Column::Email.eq(&normalized_email));

        match txn.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => query
                .lock_exclusive()
                .one(txn)
                .await
                .map_err(AuthLifecycleError::from),
            DatabaseBackend::Sqlite => {
                // Reserve SQLite's write lock by the unique tenant/email key
                // before loading the account. This avoids a deferred
                // read-then-write upgrade racing a concurrent login or
                // credential mutation.
                let statement = Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "UPDATE users SET updated_at = updated_at WHERE tenant_id = ?1 AND email = ?2",
                    [tenant_id.into(), normalized_email.into()],
                );
                let result = txn
                    .execute_raw(statement)
                    .await
                    .map_err(AuthLifecycleError::from)?;
                if result.rows_affected() != 1 {
                    return Ok(None);
                }
                query.one(txn).await.map_err(AuthLifecycleError::from)
            }
            _ => Err(AuthLifecycleError::Internal(Error::InternalServerError)),
        }
    }

    pub(crate) async fn find_session_by_id_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        session_id: uuid::Uuid,
    ) -> std::result::Result<Option<sessions::Model>, AuthLifecycleError> {
        Self::find_session_record_for_update_in_tx(txn, tenant_id, session_id, false).await
    }

    // Expected-lineage logout needs retained revocation evidence, while all
    // other callers of the active-row helper must continue excluding revoked rows.
    async fn find_session_record_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        session_id: uuid::Uuid,
        include_revoked: bool,
    ) -> std::result::Result<Option<sessions::Model>, AuthLifecycleError> {
        let query = || {
            let query = sessions::Entity::find_by_id(session_id)
                .filter(sessions::Column::TenantId.eq(tenant_id));
            if include_revoked {
                query
            } else {
                query.filter(sessions::Column::RevokedAt.is_null())
            }
        };
        match txn.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => query()
                .lock_exclusive()
                .one(txn)
                .await
                .map_err(AuthLifecycleError::from),
            DatabaseBackend::Sqlite => {
                // The id is already known, so reserve the SQLite write lock
                // before reading. A concurrent refresh/logout cannot change
                // the session between the state read and the revocation write.
                let sql = if include_revoked {
                    "UPDATE sessions SET last_used_at = last_used_at WHERE tenant_id = ?1 AND id = ?2"
                } else {
                    "UPDATE sessions SET last_used_at = last_used_at WHERE tenant_id = ?1 AND id = ?2 AND revoked_at IS NULL"
                };
                let statement = Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    sql,
                    [tenant_id.into(), session_id.into()],
                );
                let result = txn
                    .execute_raw(statement)
                    .await
                    .map_err(AuthLifecycleError::from)?;
                if result.rows_affected() != 1 {
                    return Ok(None);
                }
                query().one(txn).await.map_err(AuthLifecycleError::from)
            }
            _ => Err(AuthLifecycleError::Internal(Error::InternalServerError)),
        }
    }

    async fn find_refresh_session_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        token_hash: &str,
    ) -> std::result::Result<Option<sessions::Model>, AuthLifecycleError> {
        Self::find_refresh_session_by_hash_for_update_in_tx(txn, tenant_id, token_hash, false).await
    }

    /// Logout can arrive with the token from the immediately preceding
    /// rotation when the response carrying the new cookie was lost.
    /// Resolve that digest as well, while holding the same row lock used by
    /// refresh, so a successful logout revokes the active lineage.
    async fn find_previous_refresh_session_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        token_hash: &str,
    ) -> std::result::Result<Option<sessions::Model>, AuthLifecycleError> {
        Self::find_refresh_session_by_hash_for_update_in_tx(txn, tenant_id, token_hash, true).await
    }

    async fn find_refresh_session_by_hash_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        token_hash: &str,
        previous_hash: bool,
    ) -> std::result::Result<Option<sessions::Model>, AuthLifecycleError> {
        let column = if previous_hash {
            sessions::Column::PreviousTokenHash
        } else {
            sessions::Column::TokenHash
        };
        let mut query = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(column.eq(token_hash));
        // Preserve the refresh API contract: a matching revoked current
        // token returns SessionExpired rather than looking like an unknown
        // token. The previous-token logout fallback only selects live rows.
        if previous_hash {
            query = query.filter(sessions::Column::RevokedAt.is_null());
        }

        let session = match txn.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => query
                .lock_exclusive()
                .one(txn)
                .await
                .map_err(AuthLifecycleError::from)?,
            DatabaseBackend::Sqlite => {
                // Match by digest in the write fence itself. This reserves the
                // SQLite writer before reading the row and avoids promoting a
                // stale deferred read transaction after another rotation.
                let sql = if previous_hash {
                    "UPDATE sessions SET last_used_at = last_used_at WHERE tenant_id = ?1 AND previous_token_hash = ?2 AND revoked_at IS NULL"
                } else {
                    "UPDATE sessions SET last_used_at = last_used_at WHERE tenant_id = ?1 AND token_hash = ?2"
                };
                let statement = Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    sql,
                    [tenant_id.into(), token_hash.to_string().into()],
                );
                let result = txn
                    .execute_raw(statement)
                    .await
                    .map_err(AuthLifecycleError::from)?;
                if result.rows_affected() != 1 {
                    return Ok(None);
                }
                query
                    .one(txn)
                    .await
                    .map_err(AuthLifecycleError::from)?
            }
            _ => return Err(AuthLifecycleError::Internal(Error::InternalServerError)),
        };

        Ok(session)
    }

    async fn refresh_with_config_db(
        db: &DatabaseConnection,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        refresh_token: &str,
        require_email_verification: bool,
        retry_key: Option<&AuthRefreshRetryKey>,
        expected_session_id: Option<uuid::Uuid>,
    ) -> std::result::Result<(users::Model, AuthTokens), AuthLifecycleError> {
        rustok_auth::application_clock::admit_now(db).await?;
        Self::credential_now(db, config).await?;
        let token_hash = hash_refresh_token(refresh_token);
        // Current and historical digests are only routing hints until the
        // tenant-scoped subject and session have been reloaded under locks.
        let observed = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::TokenHash.eq(&token_hash))
            .one(db)
            .await
            .map_err(AuthLifecycleError::from)?;
        let observed = match observed {
            Some(session) => session,
            None => {
                let history = session_refresh_history::Entity::find_by_id(token_hash.clone())
                    .inner_join(sessions::Entity)
                    .filter(sessions::Column::TenantId.eq(tenant_id))
                    .one(db)
                    .await
                    .map_err(AuthLifecycleError::from)?
                    .ok_or(AuthLifecycleError::InvalidRefreshToken)?;
                sessions::Entity::find_by_id(history.session_id)
                    .filter(sessions::Column::TenantId.eq(tenant_id))
                    .one(db)
                    .await
                    .map_err(AuthLifecycleError::from)?
                    .ok_or(AuthLifecycleError::InvalidRefreshToken)?
            }
        };
        // The expected ID is only a consistency precondition, never authority.
        // Reject before any reuse-detection path can revoke a different lineage.
        if expected_session_id.is_some_and(|id| id != observed.id) {
            return Err(AuthLifecycleError::SessionLineageMismatch);
        }
        let txn = db.begin().await.map_err(AuthLifecycleError::from)?;
        // All paths, including history-only recovery, use user -> session
        // ordering and SQLite acquires its writer before transactional reads.
        let user = Self::find_user_for_update_in_tx(&txn, tenant_id, observed.user_id)
            .await?
            .ok_or(AuthLifecycleError::UserNotFound)?;

        let (session, historical_rotation) = match Self::find_refresh_session_for_update_in_tx(
            &txn, tenant_id, &token_hash,
        )
        .await?
        {
            Some(session) if session.id == observed.id && session.user_id == user.id => (session, None),
            Some(_) => return Err(AuthLifecycleError::InvalidRefreshToken),
            None => {
                let eligible = Self::detect_refresh_reuse_in_tx(
                    &txn, config, tenant_id, observed.id, user.id, &token_hash,
                ).await?;
                match eligible {
                    Some((session, rotated_at)) => (session, Some(rotated_at)),
                    None => {
                        // Commit a possible lineage revocation before reporting
                        // an invalid historical credential.
                        Self::credential_now(&txn, config).await?;
                        txn.commit().await.map_err(AuthLifecycleError::from)?;
                        return Err(AuthLifecycleError::InvalidRefreshToken);
                    }
                }
            }
        };

        if expected_session_id.is_some_and(|id| id != session.id) {
            return Err(AuthLifecycleError::SessionLineageMismatch);
        }
        let now = Self::credential_now(&txn, config).await?;
        if !session.is_active_at(now) {
            return Err(AuthLifecycleError::SessionExpired);
        }
        if !user.is_active() {
            return Err(AuthLifecycleError::UserInactive);
        }
        if require_email_verification && !user.is_email_verified() {
            return Err(AuthLifecycleError::EmailNotVerified);
        }

        let absolute_deadline = session.absolute_deadline();
        if now >= absolute_deadline || now >= session.expires_at {
            return Err(AuthLifecycleError::SessionExpired);
        }

        let new_refresh_token = match retry_key {
            Some(key) => key.derive_token(config, tenant_id, session.id)
                .map_err(AuthLifecycleError::from)?,
            None if historical_rotation.is_some() => return Err(AuthLifecycleError::RefreshAlreadyRotated),
            None => generate_refresh_token().map_err(AuthLifecycleError::from)?,
        };
        let new_token_hash = hash_refresh_token(&new_refresh_token);
        let recovery_rotation = if let Some(rotated_at) = historical_rotation {
            // A correct proof alone does not authorize an older generation.
            // Only the exact immediately preceding digest may recover the
            // current successor; history is the grace authority.
            if session.previous_token_hash.as_deref() != Some(token_hash.as_str())
                || new_token_hash != session.token_hash
            {
                return Err(AuthLifecycleError::RefreshAlreadyRotated);
            }
            Some(rotated_at)
        } else if new_token_hash == session.token_hash {
            // A cookie may have been applied before the response body was
            // lost. Recognize the same operation with the new credential too,
            // but do not let a proof become a permanent refresh credential.
            if retry_key.is_none() {
                return Err(AuthLifecycleError::Internal(Error::InternalServerError));
            }
            let previous = session.previous_token_hash.as_ref()
                .ok_or(AuthLifecycleError::RefreshAlreadyRotated)?;
            let history = session_refresh_history::Entity::find_by_id(previous.clone())
                .filter(session_refresh_history::Column::SessionId.eq(session.id))
                .one(&txn).await.map_err(AuthLifecycleError::from)?;
            let rotated_at = history
                .and_then(|history| history.rotated_at)
                .ok_or(AuthLifecycleError::RefreshAlreadyRotated)?;
            Some(rotated_at)
        } else {
            // Reusing an operation key must never resurrect a historical
            // digest as a new active credential.
            if session_refresh_history::Entity::find_by_id(new_token_hash.clone())
                .one(&txn).await.map_err(AuthLifecycleError::from)?.is_some()
            {
                return Err(AuthLifecycleError::RefreshAlreadyRotated);
            }
            None
        };

        // History lookups can outlast the observed session or retry window.
        // Recheck at the transition, not at a timestamp taken before IO.
        let now = Self::credential_now(&txn, config).await?;
        if now >= absolute_deadline || now >= session.expires_at {
            return Err(AuthLifecycleError::SessionExpired);
        }
        Self::ensure_refresh_recovery_within_grace(recovery_rotation, now)?;

        let session_id = session.id;
        let expires_at = if recovery_rotation.is_some() {
            session.expires_at.with_timezone(&Utc).min(absolute_deadline)
        } else {
            let expires_at = Self::session_expiration(now, config.refresh_expiration)?
                .min(absolute_deadline);
            let replaced_token_hash = session.token_hash.clone();
            session_refresh_history::ActiveModel {
                token_hash: Set(replaced_token_hash.clone()),
                session_id: Set(session_id),
                rotated_at: Set(Some(now.into())),
            }
            .insert(&txn)
            .await
            .map_err(AuthLifecycleError::from)?;
            let mut session_model: sessions::ActiveModel = session.into();
            session_model.token_hash = Set(new_token_hash);
            session_model.previous_token_hash = Set(Some(replaced_token_hash));
            session_model.previous_token_revoked_at = Set(Some(now.into()));
            session_model.absolute_expires_at = Set(Some(absolute_deadline.into()));
            session_model.expires_at = Set(expires_at.into());
            session_model.last_used_at = Set(Some(now.into()));
            let persisted = session_model.update(&txn).await.map_err(AuthLifecycleError::from)?;
            persisted.expires_at.with_timezone(&Utc).min(persisted.absolute_deadline())
        };

        // A recovered refresh credential is idempotent session state, not a
        // replay of stale access claims. Always sign using current authority
        // and permissions, and never move the persisted refresh deadline.
        let effective_role = Self::resolve_effective_role(&txn, tenant_id, user.id).await?;
        let effective_permissions =
            Self::resolve_effective_permission_strings(&txn, tenant_id, user.id).await?;
        Self::ensure_refresh_recovery_within_grace(
            recovery_rotation, Self::credential_now(&txn, config).await?,
        )?;
        let issued = rustok_auth::issue_direct_access_token(
            config, user.id, tenant_id, effective_role.clone(), session_id, Some(expires_at),
        )?;
        // Do not disclose a recovered credential after the exclusive window
        // closes during RBAC or signing. This operation was admitted during
        // grace; only a new historical presentation outside grace revokes.
        let finished_at = Self::credential_now(&txn, config).await?;
        Self::ensure_refresh_recovery_within_grace(recovery_rotation, finished_at)?;
        let tokens = PreparedSessionTokens {
            access_token: issued.token,
            refresh_token: new_refresh_token,
            access_deadline: issued.expires_at,
            refresh_deadline: expires_at,
            effective_role,
            permissions: effective_permissions,
        };
        // Reject dead credentials before committing, but account for commit
        // latency too. A recovered operation does not change the lineage.
        tokens.ensure_live(finished_at)?;
        txn.commit().await.map_err(AuthLifecycleError::from)?;
        let committed_at = Self::response_now(config)?;
        Self::ensure_refresh_recovery_within_grace(recovery_rotation, committed_at)?;
        Ok((user, tokens.finish(committed_at)?))
    }

    fn ensure_refresh_recovery_within_grace(
        rotated_at: Option<chrono::DateTime<chrono::FixedOffset>>,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), AuthLifecycleError> {
        if rotated_at.is_some() && !Self::refresh_rotation_within_grace(rotated_at, now) {
            return Err(AuthLifecycleError::RefreshAlreadyRotated);
        }
        Ok(())
    }

    fn refresh_rotation_within_grace(
        rotated_at: Option<chrono::DateTime<chrono::FixedOffset>>,
        now: chrono::DateTime<Utc>,
    ) -> bool {
        rotated_at.is_some_and(|rotated_at| {
            let age = now.signed_duration_since(rotated_at.with_timezone(&Utc));
            age >= Duration::zero() && age < Duration::seconds(REFRESH_REUSE_GRACE_SECONDS)
        })
    }

    /// Resolve immutable replay evidence under the already-held user fence.
    /// Return a live session only during grace; outside grace revoke it
    /// durably before the caller reports an invalid credential.
    async fn detect_refresh_reuse_in_tx(
        txn: &DatabaseTransaction,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        session_id: uuid::Uuid,
        user_id: uuid::Uuid,
        token_hash: &str,
    ) -> std::result::Result<Option<(sessions::Model, chrono::DateTime<chrono::FixedOffset>)>, AuthLifecycleError> {
        let history = session_refresh_history::Entity::find_by_id(token_hash.to_owned())
            .filter(session_refresh_history::Column::SessionId.eq(session_id))
            .inner_join(sessions::Entity)
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await
            .map_err(AuthLifecycleError::from)?;
        let Some(history) = history else {
            return Ok(None);
        };
        let Some(session) = Self::find_session_by_id_for_update_in_tx(txn, tenant_id, session_id).await? else {
            return Ok(None);
        };
        let now = Self::credential_now(txn, config).await?;
        if session.user_id != user_id || !session.is_active_at(now) {
            return Ok(None);
        }
        if history.rotated_at.is_some_and(|at| at.with_timezone(&Utc) > now) {
            // Future rotation evidence cannot establish elapsed grace.
            return Err(AuthLifecycleError::Internal(Error::InternalServerError));
        }
        if let Some(rotated_at) = history.rotated_at
            && Self::refresh_rotation_within_grace(Some(rotated_at), now)
        {
            return Ok(Some((session, rotated_at)));
        }

        tracing::warn!(
            session_id = %session.id,
            user_id = %session.user_id,
            "refresh token reuse detected; revoking session lineage"
        );
        let mut active: sessions::ActiveModel = session.into();
        active.revoked_at = Set(Some(now.into()));
        active.previous_token_hash = Set(None);
        active.update(txn).await.map_err(AuthLifecycleError::from)?;
        Ok(None)
    }

    pub async fn change_password_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        current_session_id: uuid::Uuid,
        current_password: &str,
        new_password: &str,
        min_password_length: usize,
    ) -> std::result::Result<(), AuthLifecycleError> {
        Self::change_password_db(
            ctx,
            tenant_id,
            user_id,
            current_session_id,
            current_password,
            new_password,
            min_password_length,
        )
        .await
    }

    async fn find_user_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> std::result::Result<Option<users::Model>, AuthLifecycleError> {
        Self::lock_user_for_mutation(txn, tenant_id, user_id)
            .await
            .map_err(AuthLifecycleError::from)
    }

    pub(crate) async fn find_current_session_for_password_change_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        session_id: uuid::Uuid,
    ) -> std::result::Result<Option<sessions::Model>, AuthLifecycleError> {
        // Use corroborated time to avoid locking an obviously expired
        // candidate, then revalidate after the database row-lock wait.
        let candidate_now = rustok_auth::application_clock::verified_now(txn).await?;
        let query = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::UserId.eq(user_id))
            .filter(sessions::Column::Id.eq(session_id))
            .filter(sessions::Column::RevokedAt.is_null())
            .filter(sessions::Column::CreatedAt.lte(candidate_now.clone()))
            .filter(sessions::Column::ExpiresAt.gt(candidate_now.clone()))
            .filter(
                Condition::any()
                    .add(sessions::Column::AbsoluteExpiresAt.gt(candidate_now.clone()))
                    .add(sessions::Column::AbsoluteExpiresAt.is_null()),
            );

        let session = match txn.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => query
                .lock_exclusive()
                .one(txn)
                .await
                .map_err(AuthLifecycleError::from)?,
            DatabaseBackend::Sqlite => {
                // Acquire SQLite's writer lock only for a potentially live
                // row. The model check also enforces the legacy 90-day cap.
                let statement = Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "UPDATE sessions SET last_used_at = last_used_at WHERE tenant_id = ?1 AND user_id = ?2 AND id = ?3 AND revoked_at IS NULL AND created_at <= ?4 AND expires_at > ?4 AND (absolute_expires_at IS NULL OR absolute_expires_at > ?4)",
                    [
                        tenant_id.into(),
                        user_id.into(),
                        session_id.into(),
                        candidate_now.clone().into(),
                    ],
                );
                let result = txn
                    .execute_raw(statement)
                    .await
                    .map_err(AuthLifecycleError::from)?;
                if result.rows_affected() != 1 {
                    return Ok(None);
                }
                query.one(txn).await.map_err(AuthLifecycleError::from)?
            }
            _ => return Err(AuthLifecycleError::Internal(Error::InternalServerError)),
        };

        let validated_now = rustok_auth::application_clock::verified_now(txn).await?;
        Ok(session.filter(|session| session.is_active_at(validated_now)))
    }

    async fn change_password_db(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        current_session_id: uuid::Uuid,
        current_password: &str,
        new_password: &str,
        min_password_length: usize,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let db = ctx.db();
        validate_password_policy(new_password, min_password_length)
            .map_err(AuthLifecycleError::from)?;

        let initial_user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(db)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidCredentials)?;

        if !verify_password_async(
            current_password.to_string(),
            initial_user.password_hash.clone(),
        )
        .await
        .map_err(AuthLifecycleError::from)?
        {
            return Err(AuthLifecycleError::InvalidCredentials);
        }

        let new_password_hash = hash_password_async(new_password.to_string())
            .await
            .map_err(AuthLifecycleError::from)?;

        let txn = db.begin().await.map_err(AuthLifecycleError::from)?;
        let user = Self::find_user_for_update_in_tx(&txn, tenant_id, user_id)
            .await?
            .ok_or(AuthLifecycleError::InvalidCredentials)?;

        if user.password_hash != initial_user.password_hash {
            return Err(AuthLifecycleError::InvalidCredentials);
        }
        if !user.is_active() {
            return Err(AuthLifecycleError::UserInactive);
        }

        let session = Self::find_current_session_for_password_change_in_tx(
            &txn,
            tenant_id,
            user_id,
            current_session_id,
        )
        .await?
        .ok_or(AuthLifecycleError::SessionExpired)?;

        let now = rustok_auth::application_clock::verified_now(&txn).await?;
        if !session.is_active_at(now) {
            return Err(AuthLifecycleError::SessionExpired);
        }
        let mut user_active: users::ActiveModel = user.into();
        user_active.set_replacement_password(new_password_hash, now);
        user_active
            .update(&txn)
            .await
            .map_err(AuthLifecycleError::from)?;

        let revoked_sessions = Self::revoke_user_sessions_db_with_connection(
            &txn,
            tenant_id,
            user_id,
            Some(current_session_id),
        )
        .await?;
        let _revoked_oauth_tokens =
            crate::services::oauth_token_service::revoke_user_delegated_tokens_in_tx(
                &txn, tenant_id, user_id, now,
            )
            .await
            .map_err(AuthLifecycleError::from)?;
        auth_email_change_requests::Entity::delete_many()
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
            .filter(auth_email_change_requests::Column::UserId.eq(user_id))
            .exec(&txn)
            .await
            .map_err(AuthLifecycleError::from)?;
        Self::publish_user_event_in_tx(
            ctx,
            &txn,
            tenant_id,
            user_id,
            DomainEvent::UserPasswordChanged { user_id },
        )
        .await?;
        let final_now = rustok_auth::application_clock::verified_now(&txn).await?;
        if !session.is_active_at(final_now) {
            return Err(AuthLifecycleError::SessionExpired);
        }
        txn.commit().await.map_err(AuthLifecycleError::from)?;
        AUTH_CHANGE_PASSWORD_SESSIONS_REVOKED_TOTAL.fetch_add(revoked_sessions, Ordering::Relaxed);

        Ok(())
    }

    /// Revoke the session identified by the supplied refresh token.
    ///
    /// The lookup and revocation share one transaction so a concurrent refresh
    /// cannot rotate the token between an initial read and the revoke write.
    pub async fn logout_by_refresh_token_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        refresh_token: &str,
        expected_session_id: Option<uuid::Uuid>,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let txn = ctx.db().begin().await.map_err(AuthLifecycleError::from)?;
        let token_hash = hash_refresh_token(refresh_token);

        let session = if let Some(expected) = expected_session_id {
            // Lock the expected row, then independently prove possession of
            // a credential of that exact tenant and lineage. History permits
            // a confirmed retry of logout after multiple lost rotations.
            let session = Self::find_session_record_for_update_in_tx(
                &txn, tenant_id, expected, true,
            )
            .await?
            .ok_or(AuthLifecycleError::SessionLineageMismatch)?;
            let known = session.token_hash == token_hash
                || session.previous_token_hash.as_deref() == Some(token_hash.as_str())
                || session_refresh_history::Entity::find_by_id(token_hash.clone())
                    .filter(session_refresh_history::Column::SessionId.eq(expected))
                    .one(&txn).await.map_err(AuthLifecycleError::from)?.is_some();
            if !known {
                return Err(AuthLifecycleError::SessionLineageMismatch);
            }
            // Logout is security-reducing: a known credential must remain
            // able to revoke its session after the access/refresh deadline.
            if session.revoked_at.is_some() {
                return Err(AuthLifecycleError::SessionExpired);
            }
            session
        } else {
            match Self::find_refresh_session_for_update_in_tx(&txn, tenant_id, &token_hash).await? {
                Some(session) => Some(session),
                None => Self::find_previous_refresh_session_for_update_in_tx(&txn, tenant_id, &token_hash).await?,
            }
            .ok_or(AuthLifecycleError::InvalidRefreshToken)?
        };

        if session.revoked_at.is_some() {
            return Err(AuthLifecycleError::InvalidRefreshToken);
        }

        let user_id = session.user_id;
        let now = Utc::now();
        let mut session_model: sessions::ActiveModel = session.into();
        session_model.revoked_at = Set(Some(now.into()));
        session_model
            .update(&txn)
            .await
            .map_err(AuthLifecycleError::from)?;

        Self::publish_user_event_in_tx(
            ctx,
            &txn,
            tenant_id,
            user_id,
            DomainEvent::UserLoggedOut { user_id },
        )
        .await?;
        txn.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(())
    }

    /// Revoke the current session (logout).
    pub async fn logout_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        session_id: uuid::Uuid,
    ) -> std::result::Result<(), AuthLifecycleError> {
        Self::logout_db(ctx, tenant_id, session_id).await
    }

    async fn logout_db(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        session_id: uuid::Uuid,
    ) -> std::result::Result<(), AuthLifecycleError> {
        let tx = ctx.db().begin().await.map_err(AuthLifecycleError::from)?;
        let session =
            Self::find_session_by_id_for_update_in_tx(&tx, tenant_id, session_id).await?;

        // Logout stays idempotent: an unknown or already-revoked
        // session is a successful no-op and emits no event.
        if let Some(session) = session {
            let user_id = session.user_id;
            let now = Utc::now();
            let mut session_model: sessions::ActiveModel = session.into();
            session_model.revoked_at = Set(Some(now.into()));
            session_model
                .update(&tx)
                .await
                .map_err(AuthLifecycleError::from)?;
            Self::publish_user_event_in_tx(
                ctx,
                &tx,
                tenant_id,
                user_id,
                DomainEvent::UserLoggedOut { user_id },
            )
            .await?;
        }

        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(())
    }

    /// List active sessions for a user, newest first (max `limit`).
    pub async fn list_sessions_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        limit: u64,
    ) -> std::result::Result<Vec<sessions::Model>, AuthLifecycleError> {
        Self::list_sessions_db(ctx.db(), tenant_id, user_id, limit).await
    }

    async fn list_sessions_db(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        limit: u64,
    ) -> std::result::Result<Vec<sessions::Model>, AuthLifecycleError> {
        let limit = Self::clamp_session_list_limit(limit);
        let now = rustok_auth::application_clock::verified_now(db).await?;
        let original_cutoff = now
            .checked_sub_signed(Duration::days(sessions::SESSION_ABSOLUTE_LIFETIME_DAYS))
            .ok_or(AuthLifecycleError::Internal(Error::InternalServerError))?;
        let rows = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::UserId.eq(user_id))
            .filter(sessions::Column::RevokedAt.is_null())
            .filter(sessions::Column::CreatedAt.gt(original_cutoff))
            .filter(sessions::Column::CreatedAt.lte(now.clone()))
            .filter(sessions::Column::ExpiresAt.gt(now.clone()))
            .filter(
                Condition::any()
                    .add(sessions::Column::AbsoluteExpiresAt.gt(now.clone()))
                    .add(sessions::Column::AbsoluteExpiresAt.is_null()),
            )
            .order_by_desc(sessions::Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
            .map_err(AuthLifecycleError::from)?;
        Ok(rows)
    }

    /// Revoke a specific session. Returns `false` if the session was not found
    /// or did not belong to `user_id` (ownership check prevents cross-user revocation).
    pub async fn revoke_session_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        session_id: uuid::Uuid,
    ) -> std::result::Result<bool, AuthLifecycleError> {
        Self::revoke_session_db(ctx.db(), tenant_id, user_id, session_id).await
    }

    async fn revoke_session_db(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        session_id: uuid::Uuid,
    ) -> std::result::Result<bool, AuthLifecycleError> {
        let result = sessions::Entity::update_many()
            .col_expr(sessions::Column::RevokedAt, Expr::value(Utc::now()))
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::UserId.eq(user_id))
            .filter(sessions::Column::Id.eq(session_id))
            .filter(sessions::Column::RevokedAt.is_null())
            .exec(db)
            .await
            .map_err(AuthLifecycleError::from)?;
        Ok(result.rows_affected > 0)
    }

    /// Revoke all sessions for a user except the current one.
    pub async fn revoke_all_other_sessions_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        current_session_id: uuid::Uuid,
    ) -> std::result::Result<u64, AuthLifecycleError> {
        Self::revoke_user_sessions_db(ctx.db(), tenant_id, user_id, Some(current_session_id)).await
    }

    #[cfg(test)]
    async fn create_session_and_tokens_db(
        db: &DatabaseConnection,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        user: &users::Model,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> std::result::Result<AuthTokens, AuthLifecycleError> {
        rustok_auth::application_clock::admit_now(db).await?;
        let tx = db.begin().await.map_err(AuthLifecycleError::from)?;
        let prepared = Self::create_session_and_tokens_in_tx(
            &tx, config, tenant_id, user, ip_address, user_agent,
        ).await?;
        prepared.ensure_live(Self::credential_now(&tx, config).await?)?;
        tx.commit().await.map_err(AuthLifecycleError::from)?;
        prepared.finish(Self::response_now(config)?)
    }

    async fn create_session_and_tokens_in_tx<C>(
        db: &C,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        user: &users::Model,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> std::result::Result<PreparedSessionTokens, AuthLifecycleError>
    where
        C: ConnectionTrait,
    {
        let now = Self::credential_now(db, config).await?;
        let refresh_token = generate_refresh_token().map_err(AuthLifecycleError::from)?;
        let token_hash = hash_refresh_token(&refresh_token);
        let expires_at = Self::session_expiration(now, config.refresh_expiration)?;
        Self::remaining_refresh_lifetime(expires_at, now)?;
        let mut model = sessions::ActiveModel::new(
            tenant_id, user.id, token_hash, expires_at, ip_address, user_agent,
        );
        // Keep creation and absolute cap on the same checked timeline,
        // not the database default or a second unchecked wall sample.
        model.created_at = Set(now.into());
        model.updated_at = Set(now.into());
        model.absolute_expires_at = Set(Some(Self::session_expiration(
            now, (sessions::SESSION_ABSOLUTE_LIFETIME_DAYS as u64) * 86_400,
        )?.into()));
        let session = model.insert(db).await.map_err(AuthLifecycleError::from)?;

        let effective_role = Self::resolve_effective_role(db, tenant_id, user.id).await?;
        let effective_permissions =
            Self::resolve_effective_permission_strings(db, tenant_id, user.id).await?;
        let refresh_deadline = session.expires_at.with_timezone(&Utc).min(session.absolute_deadline());
        Self::remaining_refresh_lifetime(refresh_deadline, Self::credential_now(db, config).await?)?;
        let issued = rustok_auth::issue_direct_access_token(
            config, user.id, tenant_id, effective_role.clone(), session.id, Some(refresh_deadline),
        )?;

        Ok(PreparedSessionTokens {
            access_token: issued.token,
            refresh_token,
            access_deadline: issued.expires_at,
            refresh_deadline,
            effective_role,
            permissions: effective_permissions,
        })
    }

    fn remaining_refresh_lifetime(
        deadline: chrono::DateTime<Utc>,
        now: chrono::DateTime<Utc>,
    ) -> Result<u64, AuthLifecycleError> {
        // Cookie Max-Age uses whole seconds. Never publish a success with
        // a zero-lifetime refresh credential, including subsecond expiry.
        let remaining = deadline.signed_duration_since(now).num_seconds();
        if remaining <= 0 {
            return Err(AuthLifecycleError::SessionExpired);
        }
        Ok(remaining as u64)
    }

    fn session_expiration(
        now: chrono::DateTime<Utc>,
        ttl_seconds: u64,
    ) -> std::result::Result<chrono::DateTime<Utc>, AuthLifecycleError> {
        let ttl_seconds = i64::try_from(ttl_seconds)
            .map_err(|_| AuthLifecycleError::Internal(Error::InternalServerError))?;
        let ttl = Duration::try_seconds(ttl_seconds)
            .ok_or(AuthLifecycleError::Internal(Error::InternalServerError))?;
        // Bound first, then do one checked addition. The previous path added
        // the 90-day cap with unchecked addition, which can panic for a representable time
        // close to chrono's upper boundary even if the requested TTL was small.
        let bounded_ttl = ttl.min(Duration::days(sessions::SESSION_ABSOLUTE_LIFETIME_DAYS));
        now.checked_add_signed(bounded_ttl)
            .ok_or(AuthLifecycleError::Internal(Error::InternalServerError))
    }

    pub(crate) async fn resolve_effective_role<C>(
        db: &C,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> std::result::Result<rustok_core::UserRole, AuthLifecycleError>
    where
        C: ConnectionTrait,
    {
        RbacService::get_user_role_authoritative(db, &tenant_id, &user_id)
            .await
            .map_err(AuthLifecycleError::from)
    }

    /// Effective permission snapshot for the user, resolved on the same
    /// connection/transaction that issues the session — never through the
    /// process-local authorization cache, matching the authentication-time
    /// role resolution above. Sorted and deduplicated for a stable payload.
    pub(crate) async fn resolve_effective_permission_strings<C>(
        db: &C,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> std::result::Result<Vec<String>, AuthLifecycleError>
    where
        C: ConnectionTrait,
    {
        let permissions = RbacService::get_user_permissions_authoritative(db, &tenant_id, &user_id)
            .await
            .map_err(AuthLifecycleError::from)?;
        let mut values = permissions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        Ok(values)
    }

    fn clamp_session_list_limit(limit: u64) -> u64 {
        limit.clamp(1, MAX_SESSION_LIST_LIMIT)
    }

    pub(crate) async fn revoke_user_sessions_db_with_connection<C>(
        db: &C,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        except_session_id: Option<uuid::Uuid>,
    ) -> std::result::Result<u64, AuthLifecycleError>
    where
        C: sea_orm::ConnectionTrait,
    {
        let now = Utc::now();
        let mut query = sessions::Entity::update_many()
            .col_expr(sessions::Column::RevokedAt, Expr::value(now))
            .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::UserId.eq(user_id))
            .filter(sessions::Column::RevokedAt.is_null());
        if let Some(session_id) = except_session_id {
            query = query.filter(sessions::Column::Id.ne(session_id));
        }
        let result = query.exec(db).await.map_err(AuthLifecycleError::from)?;
        Ok(result.rows_affected)
    }

    async fn revoke_user_sessions_db(
        db: &DatabaseConnection,
        tenant_id: uuid::Uuid,
        user_id: uuid::Uuid,
        except_session_id: Option<uuid::Uuid>,
    ) -> std::result::Result<u64, AuthLifecycleError> {
        let tx = db.begin().await.map_err(AuthLifecycleError::from)?;
        let user = Self::find_user_for_update_in_tx(&tx, tenant_id, user_id)
            .await?
            .ok_or(AuthLifecycleError::UserNotFound)?;
        let current_session = if let Some(session_id) = except_session_id {
            if !user.is_active() {
                return Err(AuthLifecycleError::UserInactive);
            }
            // A request authenticated before its session was revoked must
            // not subsequently revoke the user's remaining sessions.
            Some(
                Self::find_current_session_for_password_change_in_tx(
                    &tx, tenant_id, user_id, session_id,
                )
                .await?
                .ok_or(AuthLifecycleError::SessionExpired)?,
            )
        } else {
            None
        };

        let count = Self::revoke_user_sessions_db_with_connection(
            &tx, tenant_id, user_id, except_session_id,
        )
        .await?;
        if let Some(current_session) = current_session {
            let final_now = rustok_auth::application_clock::verified_now(&tx).await?;
            if !current_session.is_active_at(final_now) {
                tx.rollback().await.map_err(AuthLifecycleError::from)?;
                return Err(AuthLifecycleError::SessionExpired);
            }
        }
        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(count)
    }MAX_PASSWORD_LENGTH_BYTES, decode_access_token, hash_password,
        hash_refresh_token,
    };
    use crate::models::_entities::{session_refresh_history, user_roles};
    use crate::models::{sessions, tenants, users};
    use crate::services::rbac_service::RbacService;
    use crate::services::server_runtime_context::ServerRuntimeContext;
    use chrono::{Duration, TimeZone, Utc};
    use rustok_core::UserStatus;
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
        Set, TransactionTrait,
    };
    use serial_test::serial;
    use std::sync::atomic::Ordering;
    use uuid::Uuid;

    fn test_auth_config(
        secret: &str,
        access_expiration: u64,
        refresh_expiration: u64,
    ) -> AuthConfig {
        // Each isolated test gets independent RSA purpose keys and a distinct
        // independent MAC root; tests sharing a runtime clone this config.
        crate::auth::test_auth_config(format!("{secret}:test-only-secret-padding-for-length"))
            .with_expiration(access_expiration, refresh_expiration)
            .with_issuer("rustok-test")
            .with_audience("rustok-test")
    }

    fn unleased_test_auth_config(secret: &str) -> AuthConfig {
        let (config, _publisher) = test_auth_config(secret, 3600, 3600)
            .into_cluster_jwt(1)
            .expect("prepare valid RS256 runtime without granting a lease");
        config
    }

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

    #[tokio::test]
    async fn refresh_cookie_lifetime_never_exceeds_the_issued_session_deadline() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Cookie tenant", "cookie-lifetime")
            .insert(&db)
            .await
            .expect("tenant");
        let password_hash = hash_password("Password123!").expect("hash password");
        let user = users::ActiveModel::new(tenant.id, "cookie@example.com", &password_hash)
            .insert(&db)
            .await
            .expect("user");
        let config = test_auth_config("cookie-lifetime-secret", 600, 365 * 24 * 3600);
        let (_user, login_tokens) = AuthLifecycleService::login_with_config(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            &config,
            tenant.id,
            &user.email,
            "Password123!",
            None,
            None,
            false,
        )
        .await
        .expect("login");

        let maximum = (sessions::SESSION_ABSOLUTE_LIFETIME_DAYS * 24 * 3600) as u64;
        assert!(login_tokens.refresh_expires_in > maximum - 5);
        assert!(login_tokens.refresh_expires_in <= maximum);

        let session = sessions::Entity::find()
            .filter(sessions::Column::TokenHash.eq(hash_refresh_token(&login_tokens.refresh_token)))
            .one(&db)
            .await
            .expect("read session")
            .expect("session should exist");

        // Simulate a lineage approaching its absolute deadline; rotation
        // must return only the remaining lifetime, not a fresh 90 days.
        let created_at = Utc::now() - Duration::days(89);
        let mut active: sessions::ActiveModel = session.into();
        active.created_at = Set(created_at.into());
        active.absolute_expires_at = Set(Some((created_at + Duration::days(90)).into()));
        active.update(&db).await.expect("age the session");

        let (_user, rotated) = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant.id,
            &login_tokens.refresh_token,
            false, None, None,
        )
        .await
        .expect("refresh");
        assert!(rotated.refresh_expires_in > 0);
        assert!(rotated.refresh_expires_in <= 24 * 3600);
    }

    #[test]
    fn failed_login_timing_mask_covers_short_circuits_and_legacy_hashes() {
        let current_hash = hash_password("valid current password").expect("current hash");
        assert!(AuthLifecycleService::requires_password_timing_mask("", &current_hash));
        assert!(AuthLifecycleService::requires_password_timing_mask(
            &"x".repeat(MAX_PASSWORD_LENGTH_BYTES + 1),
            &current_hash,
        ));
        assert!(AuthLifecycleService::requires_password_timing_mask(
            "candidate",
            "malformed-hash",
        ));
        assert!(!AuthLifecycleService::requires_password_timing_mask(
            "candidate",
            &current_hash,
        ));
    }

    #[test]
    fn login_lockout_deadline_uses_subsecond_precision() {
        let now = Utc
            .timestamp_opt(1_800_000_000, 500_000_000)
            .single()
            .expect("fixed timestamp");
        let mut user = users::Model::default_service_user(Uuid::new_v4(), Uuid::new_v4());
        user.locked_until = Some((now.clone() + Duration::nanoseconds(1)).into());
        assert!(AuthLifecycleService::is_login_locked_at(&user, now.clone()));
        user.locked_until = Some(now.clone().into());
        assert!(!AuthLifecycleService::is_login_locked_at(&user, now));
    }

    #[tokio::test]
    async fn unknown_login_still_returns_invalid_credentials_after_dummy_verification() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let config = test_auth_config("unknown-account-secret", 3600, 86400);
        let result = AuthLifecycleService::login_with_config(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            &config,
            uuid::Uuid::new_v4(),
            "unknown@example.com",
            "NotTheRightPassword123!",
            None,
            None,
            false,
        )
        .await;
        assert!(matches!(
            result,
            Err(AuthLifecycleError::InvalidCredentials)
        ));
    }

    #[tokio::test]
    async fn locked_login_does_not_disclose_password_validity() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "locked-login-oracle")
            .insert(&db)
            .await
            .expect("tenant");
        let password = "CorrectPassword123!";
        let password_hash = crate::auth::hash_password_async(password.to_string())
            .await
            .expect("password hash");
        let user = users::ActiveModel::new(
            tenant.id,
            "locked-login@example.com",
            password_hash,
        )
        .insert(&db)
        .await
        .expect("user");
        let mut locked: users::ActiveModel = user.into();
        locked.locked_until = Set(Some((Utc::now() + Duration::minutes(10)).into()));
        let locked_user = locked.update(&db).await.expect("lock user");
        let config = test_auth_config("locked-login-oracle", 3600, 86400);

        for candidate in [password, "WrongPassword123!"] {
            let result = AuthLifecycleService::login_with_config(
                &ServerRuntimeContext::new(db.clone(), Default::default()),
                &config,
                tenant.id,
                "locked-login@example.com",
                candidate,
                None,
                None,
                false,
            )
            .await;
            assert!(matches!(
                result,
                Err(AuthLifecycleError::InvalidCredentials)
            ));
        }

        let unchanged = users::Entity::find_by_id(locked_user.id)
            .one(&db)
            .await
            .expect("query")
            .expect("locked user");
        assert!(AuthLifecycleService::is_login_locked(&unchanged));
        assert_eq!(unchanged.failed_login_count, locked_user.failed_login_count);
    }

    #[tokio::test]
    async fn failed_attempts_lock_atomically_and_ignore_obsolete_credentials() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "failed-login-lock")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "lock@example.com", "old-hash")
            .insert(&db)
            .await
            .expect("user");

        for _ in 0..super::MAX_FAILED_LOGIN_ATTEMPTS {
            AuthLifecycleService::register_failed_login_attempt(&db, tenant.id, &user)
                .await
                .expect("record failure");
        }
        let locked = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("query")
            .expect("user");
        assert!(AuthLifecycleService::is_login_locked(&locked));
        assert_eq!(locked.failed_login_count, 0);

        users::ActiveModel {
            id: Set(user.id),
            password_hash: Set("new-hash".to_string()),
            locked_until: Set(None),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("change credentials");
        AuthLifecycleService::register_failed_login_attempt(&db, tenant.id, &user)
            .await
            .expect("ignore stale attempt");
        let changed = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("query")
            .expect("user");
        assert_eq!(changed.failed_login_count, 0);
        assert!(changed.locked_until.is_none());
    }

    #[test]
    fn session_list_limit_is_bounded() {
        assert_eq!(AuthLifecycleService::clamp_session_list_limit(0), 1);
        assert_eq!(AuthLifecycleService::clamp_session_list_limit(50), 50);
        assert_eq!(AuthLifecycleService::clamp_session_list_limit(100), 100);
        assert_eq!(AuthLifecycleService::clamp_session_list_limit(101), 100);
        assert_eq!(
            AuthLifecycleService::clamp_session_list_limit(u64::MAX),
            100
        );
    }

    #[test]
    fn refresh_lifetime_is_measured_at_finalization_and_rejects_zero_seconds() {
        let issued = Utc::now();
        let deadline = issued + Duration::seconds(10);
        assert_eq!(
            AuthLifecycleService::remaining_refresh_lifetime(
                deadline, issued + Duration::milliseconds(3400),
            ).unwrap(),
            6
        );
        for now in [
            deadline - Duration::milliseconds(1),
            deadline,
            deadline + Duration::seconds(1),
        ] {
            assert!(matches!(
                AuthLifecycleService::remaining_refresh_lifetime(deadline, now),
                Err(AuthLifecycleError::SessionExpired)
            ));
        }
    }

    #[test]
    fn prepared_session_reports_remaining_signed_and_refresh_lifetimes() {
        let now = Utc::now();
        let make = || PreparedSessionTokens {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            access_deadline: now + Duration::seconds(5),
            refresh_deadline: now + Duration::seconds(10),
            effective_role: rustok_core::UserRole::Customer,
            permissions: vec![],
        };
        let tokens = make().finish(now + Duration::milliseconds(2100)).unwrap();
        assert_eq!(tokens.expires_in, 2);
        assert_eq!(tokens.refresh_expires_in, 7);
        assert!(matches!(make().finish(now + Duration::seconds(5)), Err(AuthLifecycleError::SessionExpired)));
    }

    #[tokio::test]
    async fn expired_new_session_is_not_committed() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Expired issue tenant", "expired-issue")
            .insert(&db).await.unwrap();
        let user = users::ActiveModel::new(tenant.id, "expired-issue@example.com", "hash")
            .insert(&db).await.unwrap();
        let config = test_auth_config("expired-issue-secret", 600, 0);
        let result = AuthLifecycleService::create_session_and_tokens_db(
            &db, &config, tenant.id, &user, None, None,
        ).await;
        assert!(matches!(result, Err(AuthLifecycleError::SessionExpired)));
        assert_eq!(
            sessions::Entity::find()
                .filter(sessions::Column::TenantId.eq(tenant.id))
                .count(&db).await.unwrap(),
            0,
            "expired issuance must roll back its session row"
        );
    }

    #[test]
    fn session_expiration_rejects_out_of_range_ttl() {
        let now = Utc::now();

        assert!(AuthLifecycleService::session_expiration(now, u64::MAX).is_err());
        assert!(AuthLifecycleService::session_expiration(now, i64::MAX as u64).is_err());
        assert!(
            AuthLifecycleService::session_expiration(now, 3_600)
                .expect("normal session ttl should be representable")
                > now
        );
        assert_eq!(
            AuthLifecycleService::session_expiration(now, 365 * 24 * 60 * 60)
                .expect("long but valid TTL must be bounded"),
            now + Duration::days(sessions::SESSION_ABSOLUTE_LIFETIME_DAYS)
        );
        // Boundary values must return a typed error rather than panic.
        let extreme = chrono::DateTime::<Utc>::MAX_UTC;
        assert!(AuthLifecycleService::session_expiration(extreme, 1).is_err());
        let almost_extreme = extreme - Duration::days(1);
        assert_eq!(
            AuthLifecycleService::session_expiration(almost_extreme, 60)
                .expect("small TTL near chrono limit is still valid"),
            almost_extreme + Duration::seconds(60)
        );
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
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
            resolved_permissions.contains(&rustok_api::Permission::PRODUCTS_CREATE),
            "manager role permissions should be available"
        );
        assert!(
            !resolved_permissions.contains(&rustok_api::Permission::USERS_MANAGE),
            "manager role should not get admin-only permissions"
        );
    }

    #[tokio::test]
    async fn deactivating_user_preserves_record_and_marks_it_inactive() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Deactivation tenant", "deactivation-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");
        let user = AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "deactivate@example.com",
            "Password123!",
            None,
            rustok_core::UserRole::Customer,
            None,
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
        )
        .await
        .expect("failed to create user");

        let tx = db.begin().await.expect("failed to begin transaction");
        AuthLifecycleService::deactivate_user_in_tx(&tx, tenant.id, user.id)
            .await
            .expect("deactivation should succeed");
        tx.commit().await.expect("failed to commit deactivation");

        let persisted = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("failed to load deactivated user")
            .expect("deactivated user record should remain");
        assert_eq!(persisted.status, UserStatus::Inactive);
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
        )
        .await
        .expect("failed to create user");

        let updated = AuthLifecycleService::update_profile_db(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            tenant.id,
            user.id,
            Some("   Updated Name   ".to_string()),
        )
        .await
        .expect("update_profile should trim name");

        assert_eq!(updated.name.as_deref(), Some("Updated Name"));

        let cleared = AuthLifecycleService::update_profile_db(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            tenant.id,
            user.id,
            Some("   ".to_string()),
        )
        .await
        .expect("blank profile name should clear value");

        assert_eq!(cleared.name, None);
    }

    #[tokio::test]
    async fn profile_update_rolls_back_when_registered_event_transport_fails() {
        use rustok_core::events::{EventTransport, ReliabilityLevel};
        use rustok_events::EventEnvelope;
        use std::sync::{Arc, atomic::AtomicUsize};

        #[derive(Default)]
        struct FailingTransport(AtomicUsize);

        #[async_trait::async_trait]
        impl EventTransport for FailingTransport {
            async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
                panic!("transactional Auth writes must not publish outside the transaction");
            }

            fn supports_transactional_writes(&self) -> bool {
                true
            }

            async fn write_root_in_transaction(
                &self,
                _transaction: &sea_orm::DatabaseTransaction,
                _envelope: EventEnvelope,
            ) -> rustok_core::Result<()> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err(rustok_core::Error::Validation("injected outbox failure".to_string()))
            }

            fn reliability_level(&self) -> ReliabilityLevel {
                ReliabilityLevel::Outbox
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Transport tenant", "transport-tenant")
            .insert(&db)
            .await
            .expect("tenant");
        let user = AuthLifecycleService::create_user_db(
            &db,
            tenant.id,
            "transport@example.com",
            "Password123!",
            Some("Original".to_string()),
            rustok_core::UserRole::Customer,
            None,
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
        )
        .await
        .expect("user");
        let ctx = ServerRuntimeContext::new(db.clone(), Default::default());
        let failing = Arc::new(FailingTransport::default());
        ctx.shared_insert(failing.clone() as Arc<dyn EventTransport>);

        let result = AuthLifecycleService::update_profile_runtime(
            &ctx, tenant.id, user.id, Some("Changed".to_string()),
        )
        .await;
        assert!(matches!(result, Err(AuthLifecycleError::Internal(_))));
        assert_eq!(failing.0.load(Ordering::SeqCst), 1);
        let persisted = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("read user")
            .expect("user");
        assert_eq!(persisted.name.as_deref(), Some("Original"));
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
        )
        .await
        .expect("failed to create user in tenant A");

        let result = AuthLifecycleService::update_profile_db(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
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
            lifecycle_permissions.contains(&rustok_api::Permission::PRODUCTS_CREATE),
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

        let config = test_auth_config("relation-role-secret", 600, 3600);

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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
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

        let config = test_auth_config("test-secret", 3600, 3600);

        let metrics_before = AuthLifecycleService::metrics_snapshot();
        let result = AuthLifecycleService::login_with_config(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            &config,
            tenant.id,
            "inactive@example.com",
            "Password123!",
            None,
            None,
            false,
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
    async fn register_rolls_back_user_and_session_when_token_issuance_fails() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Atomic register tenant", "atomic-register-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");
        let ctx = ServerRuntimeContext::new(
            db.clone(),
            crate::common::settings::RustokSettings::default(),
        );
        let config = unleased_test_auth_config("register-atomic-secret-key-for-tests!");

        let result = AuthLifecycleService::register_runtime(
            &ctx,
            &config,
            tenant.id,
            "atomic@example.com",
            "Password123!",
            Some("Atomic Register".to_string()),
        )
        .await;

        assert!(matches!(result, Err(AuthLifecycleError::Internal(_))));

        let users = users::Entity::find()
            .filter(users::Column::TenantId.eq(tenant.id))
            .filter(users::Column::Email.eq("atomic@example.com"))
            .all(&db)
            .await
            .expect("failed to inspect rolled-back users");
        assert!(users.is_empty());

        let session_count = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .count(&db)
            .await
            .expect("failed to inspect sessions");
        assert_eq!(session_count, 0);
    }

    #[tokio::test]
    async fn login_rolls_back_last_login_and_session_when_token_issuance_fails() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Atomic login tenant", "atomic-login-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");
        let password_hash = hash_password("Password123!").expect("failed to hash password");
        let user = users::ActiveModel::new(tenant.id, "atomic-login@example.com", &password_hash)
            .insert(&db)
            .await
            .expect("failed to create user");

        let config = unleased_test_auth_config("login-atomic-secret-key-for-tests-32b!");

        let result = AuthLifecycleService::login_with_config(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            &config,
            tenant.id,
            "atomic-login@example.com",
            "Password123!",
            None,
            None,
            false,
        )
        .await;

        assert!(matches!(result, Err(AuthLifecycleError::Internal(_))));

        let persisted_user = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("failed to inspect rolled-back login user")
            .expect("login user should still exist");
        assert!(
            persisted_user.last_login_at.is_none(),
            "failed token issuance must not persist login timestamp"
        );

        let session_count = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user.id))
            .count(&db)
            .await
            .expect("failed to inspect login sessions");
        assert_eq!(session_count, 0);
    }

    #[tokio::test]
    #[serial]
    async fn current_session_fence_rejects_expired_absolute_deadline_with_live_sliding_expiry() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "absolute-session-fence")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "absolute-fence@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let now = Utc::now();
        let mut active_sliding = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "absolute-expired-refresh-hash".to_string(),
            now + Duration::hours(1),
            None,
            None,
        );
        active_sliding.absolute_expires_at = Set(Some((now - Duration::minutes(1)).into()));
        let session = active_sliding.insert(&db).await.expect("session");

        let tx = db.begin().await.expect("transaction");
        let found = AuthLifecycleService::find_current_session_for_password_change_in_tx(
            &tx, tenant.id, user.id, session.id,
        )
        .await
        .expect("session fence");
        assert!(
            found.is_none(),
            "an expired absolute deadline must override a live sliding expiry"
        );
        tx.rollback().await.expect("rollback");
    }

    #[tokio::test]
    async fn change_password_rejects_expired_current_session() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "password-expired-session")
            .insert(&db)
            .await
            .expect("tenant");
        let old_hash = hash_password("OldPassword123!").expect("password hash");
        let user = users::ActiveModel::new(tenant.id, "expired-session@example.com", &old_hash)
            .insert(&db)
            .await
            .expect("user");
        let session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "expired-refresh-token-hash".to_string(),
            Utc::now() - Duration::minutes(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("expired session");

        let result = AuthLifecycleService::change_password_db(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            tenant.id,
            user.id,
            session.id,
            "OldPassword123!",
            "NewPassword123!",
            8,
        )
        .await;

        assert!(matches!(result, Err(AuthLifecycleError::SessionExpired)));
        let unchanged = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("read user")
            .expect("user");
        assert_eq!(unchanged.password_hash, old_hash);
    }

    #[tokio::test]
    async fn change_password_rejects_revoked_current_session() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant =
            tenants::ActiveModel::new("Password session tenant", "password-session-tenant")
                .insert(&db)
                .await
                .expect("failed to create tenant");
        let password_hash = hash_password("OldPassword123!").expect("failed to hash password");
        let user =
            users::ActiveModel::new(tenant.id, "password-session@example.com", &password_hash)
                .insert(&db)
                .await
                .expect("failed to create user");

        let session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "password-session-token".to_string(),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("failed to create session");

        sessions::Entity::update_many()
            .col_expr(
                sessions::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(Utc::now()),
            )
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::Id.eq(session.id))
            .exec(&db)
            .await
            .expect("failed to revoke current session");

        let result = AuthLifecycleService::change_password_db(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            tenant.id,
            user.id,
            session.id,
            "OldPassword123!",
            "NewPassword123!",
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
        )
        .await;

        assert!(matches!(result, Err(AuthLifecycleError::SessionExpired)));
    }

    #[tokio::test]
    async fn logout_can_find_previous_rotated_token_without_crossing_tenant() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Logout rotation", "logout-rotation")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "logout-rotation@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let old_hash = hash_refresh_token("previous-logout-cookie");
        let mut model = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token("new-active-cookie"),
            Utc::now() + Duration::hours(1),
            None,
            None,
        );
        model.previous_token_hash = Set(Some(old_hash.clone()));
        model.previous_token_revoked_at = Set(Some(Utc::now().into()));
        let session = model.insert(&db).await.expect("session");

        let tx = db.begin().await.expect("transaction");
        assert!(
            AuthLifecycleService::find_refresh_session_for_update_in_tx(&tx, tenant.id, &old_hash)
                .await
                .expect("current-token query")
                .is_none()
        );
        assert!(
            AuthLifecycleService::find_previous_refresh_session_for_update_in_tx(
                &tx,
                Uuid::new_v4(),
                &old_hash,
            )
            .await
            .expect("other-tenant query")
            .is_none()
        );
        let selected = AuthLifecycleService::find_previous_refresh_session_for_update_in_tx(
            &tx, tenant.id, &old_hash,
        )
        .await
        .expect("previous-token lookup")
        .expect("previous token must resolve the active lineage");
        assert_eq!(selected.id, session.id);
        assert!(selected.is_active());

        let mut active: sessions::ActiveModel = selected.into();
        active.revoked_at = Set(Some(Utc::now().into()));
        active.update(&tx).await.expect("revoke locked session");
        tx.commit().await.expect("commit");

        let stored = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .expect("read")
            .expect("session");
        assert!(!stored.is_active());
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

        let config = test_auth_config("refresh-secret", 600, 3600);

        let result = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant.id,
            expired_token,
            false, None, None,
        )
        .await;

        assert!(matches!(result, Err(AuthLifecycleError::SessionExpired)));
    }

    #[test]
    fn concurrent_refresh_conflict_maps_to_http_conflict() {
        assert!(matches!(
            Error::from(AuthLifecycleError::RefreshAlreadyRotated),
            Error::Conflict(_)
        ));
    }

    #[tokio::test]
    async fn identical_cookie_envelopes_recover_one_committed_successor() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Cookie recovery", "cookie-recovery")
            .insert(&db).await.unwrap();
        let user = users::ActiveModel::new(tenant.id, "cookie@example.com", "hash")
            .insert(&db).await.unwrap();
        let token = "a1".repeat(32);
        let cookie = rustok_auth::AuthRefreshCookie::new(&token, "b2".repeat(32)).unwrap();
        let session = sessions::ActiveModel::new(
            tenant.id, user.id, hash_refresh_token(&token),
            Utc::now() + Duration::hours(1), None, None,
        ).insert(&db).await.unwrap();
        let config = test_auth_config("cookie-recovery-secret", 600, 3600);
        let proof = cookie.retry_key(tenant.id).unwrap();
        let (_, first) = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, cookie.token(), false, Some(&proof), Some(session.id),
        ).await.unwrap();
        let persisted = sessions::Entity::find_by_id(session.id).one(&db).await.unwrap().unwrap();
        let replay = rustok_auth::AuthRefreshCookie::parse(&cookie.cookie_value()).unwrap();
        let proof = replay.retry_key(tenant.id).unwrap();
        let (_, recovered) = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, replay.token(), false, Some(&proof), Some(session.id),
        ).await.unwrap();
        assert_eq!(first.refresh_token, recovered.refresh_token);
        let after = sessions::Entity::find_by_id(session.id).one(&db).await.unwrap().unwrap();
        assert_eq!(persisted.token_hash, after.token_hash);
        assert_eq!(persisted.expires_at, after.expires_at);
        assert_eq!(persisted.previous_token_revoked_at, after.previous_token_revoked_at);
    }

    #[tokio::test]
    async fn expected_lineage_refuses_foreign_current_and_historical_credentials_before_mutation() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Lineage tenant", "lineage-tenant")
            .insert(&db).await.unwrap();
        let user = users::ActiveModel::new(tenant.id, "lineage@example.com", "hash")
            .insert(&db).await.unwrap();
        let current = "a1".repeat(32);
        let previous = "b2".repeat(32);
        let expected = sessions::ActiveModel::new(tenant.id, user.id,
            hash_refresh_token(&"c3".repeat(32)), Utc::now() + Duration::hours(1),
            None, None).insert(&db).await.unwrap();
        let actual = sessions::ActiveModel::new(tenant.id, user.id,
            hash_refresh_token(&current), Utc::now() + Duration::hours(1),
            None, None).insert(&db).await.unwrap();
        session_refresh_history::ActiveModel {
            token_hash: Set(hash_refresh_token(&previous)),
            session_id: Set(actual.id),
            rotated_at: Set(Some((Utc::now() - Duration::seconds(60)).into())),
        }.insert(&db).await.unwrap();
        let config = test_auth_config("lineage-secret", 600, 3600);
        let runtime = ServerRuntimeContext::new(db.clone(), crate::common::RustokSettings::default());
        for token in [&current, &previous] {
            let result = AuthLifecycleService::refresh_with_config_db(
                &db, &config, tenant.id, token, false, None, Some(expected.id),
            ).await;
            assert!(matches!(result, Err(AuthLifecycleError::SessionLineageMismatch)));
            let result = AuthLifecycleService::logout_by_refresh_token_runtime(
                &runtime, tenant.id, token, Some(expected.id),
            ).await;
            assert!(matches!(result, Err(AuthLifecycleError::SessionLineageMismatch)));
        }
        for row in [&actual, &expected] {
            let after = sessions::Entity::find_by_id(row.id).one(&db).await.unwrap().unwrap();
            assert!(after.is_active());
            assert_eq!(after.token_hash, row.token_hash);
            assert_eq!(after.expires_at, row.expires_at);
        }
        AuthLifecycleService::logout_by_refresh_token_runtime(
            &runtime, tenant.id, &previous, Some(actual.id),
        ).await.unwrap();
        let revoked = sessions::Entity::find_by_id(actual.id)
            .one(&db).await.unwrap().unwrap();
        assert!(revoked.revoked_at.is_some());
        for token in [&current, &previous] {
            assert!(matches!(
                AuthLifecycleService::logout_by_refresh_token_runtime(
                    &runtime, tenant.id, token, Some(actual.id),
                ).await,
                Err(AuthLifecycleError::SessionExpired)
            ));
        }
        for (requested_tenant, token) in [
            (tenant.id, "not-a-lineage-credential"),
            (Uuid::new_v4(), current.as_str()),
        ] {
            assert!(matches!(
                AuthLifecycleService::logout_by_refresh_token_runtime(
                    &runtime, requested_tenant, token, Some(actual.id),
                ).await,
                Err(AuthLifecycleError::SessionLineageMismatch)
            ));
        }
        let after_retry = sessions::Entity::find_by_id(actual.id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(after_retry.revoked_at, revoked.revoked_at);
        assert_eq!(after_retry.token_hash, revoked.token_hash);

        let tx = db.begin().await.unwrap();
        assert!(
            AuthLifecycleService::find_session_by_id_for_update_in_tx(
                &tx, tenant.id, actual.id,
            ).await.unwrap().is_none(),
            "active-only callers must not see revoked session records"
        );
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn refresh_reuse_inside_grace_is_rejected_without_revoking_live_session() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Refresh grace tenant", "refresh-grace")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "refresh-grace@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let previous_token = "refresh-token-inside-grace";
        let current_token = "current-refresh-token-inside-grace";
        let mut session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token(current_token),
            Utc::now() + Duration::hours(1),
            None,
            None,
        );
        session.previous_token_hash = Set(Some(hash_refresh_token(previous_token)));
        session.previous_token_revoked_at =
            Set(Some((Utc::now() - Duration::seconds(5)).into()));
        let session = session.insert(&db).await.expect("session");
        session_refresh_history::ActiveModel {
            token_hash: Set(hash_refresh_token(previous_token)),
            session_id: Set(session.id),
            rotated_at: Set(Some((Utc::now() - Duration::seconds(5)).into())),
        }
        .insert(&db)
        .await
        .expect("historical rotation");
        let config = test_auth_config("refresh-grace-secret", 600, 3600);

        let result = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant.id,
            previous_token,
            false, None, None,
        )
        .await;

        assert!(matches!(
            result,
            Err(AuthLifecycleError::RefreshAlreadyRotated)
        ));
        let stored = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .expect("read session")
            .expect("session remains present");
        assert!(stored.is_active());
        assert_eq!(stored.token_hash, hash_refresh_token(current_token));
        assert_eq!(
            stored.previous_token_hash,
            Some(hash_refresh_token(previous_token))
        );
    }

    #[tokio::test]
    async fn refresh_reuse_after_grace_revokes_live_session_lineage() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Refresh reuse tenant", "refresh-reuse")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "refresh-reuse@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let previous_token = "refresh-token-after-grace";
        let current_token = "current-refresh-token-after-grace";
        let mut session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token(current_token),
            Utc::now() + Duration::hours(1),
            None,
            None,
        );
        session.previous_token_hash = Set(Some(hash_refresh_token(previous_token)));
        session.previous_token_revoked_at =
            Set(Some((Utc::now() - Duration::seconds(60)).into()));
        let session = session.insert(&db).await.expect("session");
        session_refresh_history::ActiveModel {
            token_hash: Set(hash_refresh_token(previous_token)),
            session_id: Set(session.id),
            rotated_at: Set(Some((Utc::now() - Duration::seconds(60)).into())),
        }
        .insert(&db)
        .await
        .expect("historical rotation");
        let config = test_auth_config("refresh-reuse-secret", 600, 3600);

        let result = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant.id,
            previous_token,
            false, None, None,
        )
        .await;

        assert!(matches!(
            result,
            Err(AuthLifecycleError::InvalidRefreshToken)
        ));
        let stored = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .expect("read session")
            .expect("session remains as revocation evidence");
        assert!(!stored.is_active());
        assert!(stored.revoked_at.is_some());
        assert!(stored.previous_token_hash.is_none());
        assert_eq!(stored.token_hash, hash_refresh_token(current_token));
    }

    #[tokio::test]
    async fn refresh_reuse_revocation_failure_is_propagated_without_false_success() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new(
            "Refresh reuse failure tenant",
            "refresh-reuse-failure",
        )
        .insert(&db)
        .await
        .expect("tenant");
        let user = users::ActiveModel::new(
            tenant.id,
            "refresh-reuse-failure@example.com",
            "hash",
        )
        .insert(&db)
        .await
        .expect("user");
        let previous_token = "refresh-token-revocation-failure";
        let current_token = "current-refresh-token-revocation-failure";
        let mut session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token(current_token),
            Utc::now() + Duration::hours(1),
            None,
            None,
        );
        session.previous_token_hash = Set(Some(hash_refresh_token(previous_token)));
        session.previous_token_revoked_at =
            Set(Some((Utc::now() - Duration::seconds(60)).into()));
        let session = session.insert(&db).await.expect("session");
        session_refresh_history::ActiveModel {
            token_hash: Set(hash_refresh_token(previous_token)),
            session_id: Set(session.id),
            rotated_at: Set(Some((Utc::now() - Duration::seconds(60)).into())),
        }
        .insert(&db)
        .await
        .expect("historical rotation");

        db.execute_unprepared(
            "CREATE TRIGGER fail_refresh_reuse_revocation
             BEFORE UPDATE ON sessions
             WHEN NEW.revoked_at IS NOT NULL
             BEGIN
                 SELECT RAISE(ABORT, 'simulated refresh-reuse revocation failure');
             END;",
        )
        .await
        .expect("install revocation failure trigger");
        let config = test_auth_config("refresh-reuse-failure-secret", 600, 3600);

        let result = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant.id,
            previous_token,
            false, None, None,
        )
        .await;

        assert!(matches!(
            result,
            Err(AuthLifecycleError::Internal(_))
        ));
        db.execute_unprepared("DROP TRIGGER fail_refresh_reuse_revocation")
            .await
            .expect("remove revocation failure trigger");
        let stored = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .expect("read session after failed revocation")
            .expect("session remains present");
        assert!(stored.is_active());
        assert_eq!(stored.token_hash, hash_refresh_token(current_token));
        assert_eq!(
            stored.previous_token_hash,
            Some(hash_refresh_token(previous_token))
        );
    }

    #[tokio::test]
    async fn replay_of_an_older_generation_revokes_only_its_session() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("History tenant", "refresh-history")
            .insert(&db)
            .await
            .expect("tenant");
        let other = tenants::ActiveModel::new("Other tenant", "refresh-history-other")
            .insert(&db)
            .await
            .expect("other tenant");
        let user = users::ActiveModel::new(
            tenant.id,
            "history@example.com",
            &hash_password("Password123!").expect("hash"),
        )
        .insert(&db)
        .await
        .expect("user");
        let config = test_auth_config("refresh-history-secret", 600, 3600);
        let (_, first) = AuthLifecycleService::login_with_config(
            &ServerRuntimeContext::new(db.clone(), Default::default()), &config, tenant.id, &user.email, "Password123!", None, None, false,
        )
        .await
        .expect("login");
        let (_, second) = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, &first.refresh_token, false, None, None,
        )
        .await
        .expect("first rotation");
        let (_, third) = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, &second.refresh_token, false, None, None,
        )
        .await
        .expect("second rotation");

        let first_hash = hash_refresh_token(&first.refresh_token);
        let history = session_refresh_history::Entity::find_by_id(first_hash.clone())
            .one(&db)
            .await
            .expect("read history")
            .expect("first generation retained");
        let session_id = history.session_id;
        let mut aged: session_refresh_history::ActiveModel = history.into();
        aged.rotated_at = Set(Some((Utc::now() - Duration::seconds(60)).into()));
        aged.update(&db).await.expect("age history fixture");

        let wrong_tenant = AuthLifecycleService::refresh_with_config_db(
            &db, &config, other.id, &first.refresh_token, false, None, None,
        )
        .await;
        assert!(matches!(wrong_tenant, Err(AuthLifecycleError::InvalidRefreshToken)));
        let session = sessions::Entity::find_by_id(session_id)
            .one(&db).await.expect("read session").expect("session");
        assert!(session.is_active());
        assert_eq!(session.previous_token_hash, Some(hash_refresh_token(&second.refresh_token)));

        let replay = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, &first.refresh_token, false, None, None,
        )
        .await;
        assert!(matches!(replay, Err(AuthLifecycleError::InvalidRefreshToken)));
        let session = sessions::Entity::find_by_id(session_id)
            .one(&db).await.expect("read session").expect("session");
        assert!(!session.is_active());
        assert_eq!(session.token_hash, hash_refresh_token(&third.refresh_token));

        sessions::Entity::delete_by_id(session_id)
            .exec(&db).await.expect("purge session");
        assert!(
            session_refresh_history::Entity::find_by_id(first_hash)
                .one(&db).await.expect("read history after purge").is_none()
        );
    }

    #[tokio::test]
    async fn future_refresh_rotation_evidence_does_not_revoke_lineage() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Future rotation tenant", "future-rotation")
            .insert(&db).await.unwrap();
        let user = users::ActiveModel::new(tenant.id, "future-rotation@example.com", "hash")
            .insert(&db).await.unwrap();
        let current = "a1".repeat(32);
        let previous = "b2".repeat(32);
        let future = Utc::now() + Duration::minutes(1);
        let mut model = sessions::ActiveModel::new(
            tenant.id, user.id, hash_refresh_token(&current),
            Utc::now() + Duration::hours(1), None, None,
        );
        model.previous_token_hash = Set(Some(hash_refresh_token(&previous)));
        model.previous_token_revoked_at = Set(Some(future.into()));
        let session = model.insert(&db).await.unwrap();
        session_refresh_history::ActiveModel {
            token_hash: Set(hash_refresh_token(&previous)),
            session_id: Set(session.id),
            rotated_at: Set(Some(future.into())),
        }.insert(&db).await.unwrap();
        let config = test_auth_config("future-rotation-secret", 600, 3600);
        let result = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, &previous, false, None, Some(session.id),
        ).await;
        assert!(matches!(result, Err(AuthLifecycleError::Internal(_))));
        let persisted = sessions::Entity::find_by_id(session.id)
            .one(&db).await.unwrap().unwrap();
        assert_eq!(persisted.token_hash, session.token_hash);
        assert_eq!(persisted.previous_token_hash, session.previous_token_hash);
        assert!(persisted.revoked_at.is_none());
    }

    #[test]
    fn refresh_recovery_rechecks_the_exclusive_window_at_completion() {
        let rotated_at = Utc::now();
        let evidence = Some(rotated_at.fixed_offset());
        let cutoff = rotated_at + Duration::seconds(REFRESH_REUSE_GRACE_SECONDS);

        for now in [rotated_at, cutoff - Duration::nanoseconds(1)] {
            assert!(AuthLifecycleService::ensure_refresh_recovery_within_grace(evidence, now).is_ok());
        }
        for now in [rotated_at - Duration::nanoseconds(1), cutoff, cutoff + Duration::seconds(1)] {
            assert!(matches!(
                AuthLifecycleService::ensure_refresh_recovery_within_grace(evidence, now),
                Err(AuthLifecycleError::RefreshAlreadyRotated)
            ));
        }
        // A new operation has no recovery window to enforce.
        assert!(AuthLifecycleService::ensure_refresh_recovery_within_grace(None, cutoff).is_ok());
    }

    #[tokio::test]
    async fn refresh_retry_proof_recovers_both_cookie_outcomes_without_rotating_again() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Retry tenant", "refresh-retry")
            .insert(&db).await.expect("tenant");
        let user = users::ActiveModel::new(
            tenant.id, "retry@example.com", &hash_password("Password123!").expect("hash"),
        ).insert(&db).await.expect("user");
        let config = test_auth_config("refresh-retry-secret", 600, 3600);
        let (_, first) = AuthLifecycleService::login_with_config(
            &ServerRuntimeContext::new(db.clone(), Default::default()), &config, tenant.id, &user.email, "Password123!", None, None, false,
        ).await.expect("login");
        let proof_raw = "a1".repeat(32);
        let proof = rustok_auth::AuthRefreshRetryKey::from_header_values(
            [proof_raw.as_bytes()].into_iter(),
        ).unwrap().unwrap();
        let wrong_raw = "b2".repeat(32);
        let wrong = rustok_auth::AuthRefreshRetryKey::from_header_values(
            [wrong_raw.as_bytes()].into_iter(),
        ).unwrap().unwrap();
        let (_, rotated) = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, &first.refresh_token, false, Some(&proof), None,
        ).await.expect("rotate");
        let first_hash = hash_refresh_token(&first.refresh_token);
        let history = session_refresh_history::Entity::find_by_id(first_hash.clone())
            .one(&db).await.unwrap().unwrap();
        let session_id = history.session_id;
        let before = sessions::Entity::find_by_id(session_id).one(&db).await.unwrap().unwrap();
        for wrong_proof in [None, Some(&wrong)] {
            let result = AuthLifecycleService::refresh_with_config_db(
                &db, &config, tenant.id, &first.refresh_token, false, wrong_proof, None,
            ).await;
            assert!(matches!(result, Err(AuthLifecycleError::RefreshAlreadyRotated)));
        }
        for presented in [&first.refresh_token, &rotated.refresh_token] {
            let (_, recovered) = AuthLifecycleService::refresh_with_config_db(
                &db, &config, tenant.id, presented, false, Some(&proof), None,
            ).await.expect("recover committed rotation");
            assert_eq!(recovered.refresh_token, rotated.refresh_token);
            assert!(recovered.refresh_expires_in <= rotated.refresh_expires_in);
        }
        let after = sessions::Entity::find_by_id(session_id).one(&db).await.unwrap().unwrap();
        assert_eq!(before.token_hash, after.token_hash);
        assert_eq!(before.expires_at, after.expires_at);
        assert_eq!(before.previous_token_revoked_at, after.previous_token_revoked_at);
        assert_eq!(session_refresh_history::Entity::find()
            .filter(session_refresh_history::Column::SessionId.eq(session_id))
            .count(&db).await.unwrap(), 1);

        let (_, next) = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, &rotated.refresh_token, false, Some(&wrong), None,
        ).await.expect("new operation");
        for (presented, key) in [
            (&next.refresh_token, &proof),
            (&first.refresh_token, &wrong),
        ] {
            let result = AuthLifecycleService::refresh_with_config_db(
                &db, &config, tenant.id, presented, false, Some(key), None,
            ).await;
            assert!(matches!(result, Err(AuthLifecycleError::RefreshAlreadyRotated)));
        }
        let mut aged: session_refresh_history::ActiveModel = history.into();
        aged.rotated_at = Set(Some((Utc::now() - Duration::seconds(60)).into()));
        aged.update(&db).await.unwrap();
        // The current credential and its used proof must conflict outside
        // grace without revoking the lineage. Age its own history evidence.
        let second_hash = hash_refresh_token(&rotated.refresh_token);
        let mut second_history: session_refresh_history::ActiveModel =
            session_refresh_history::Entity::find_by_id(second_hash)
                .one(&db).await.unwrap().unwrap().into();
        second_history.rotated_at = Set(Some((Utc::now() - Duration::seconds(60)).into()));
        second_history.update(&db).await.unwrap();
        let result = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, &next.refresh_token, false, Some(&wrong), None,
        ).await;
        assert!(matches!(result, Err(AuthLifecycleError::RefreshAlreadyRotated)));
        let live = sessions::Entity::find_by_id(session_id).one(&db).await.unwrap().unwrap();
        assert!(live.is_active());
        let result = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, &first.refresh_token, false, Some(&proof), None,
        ).await;
        assert!(matches!(result, Err(AuthLifecycleError::InvalidRefreshToken)));
        let after = sessions::Entity::find_by_id(session_id).one(&db).await.unwrap().unwrap();
        assert!(!after.is_active());
    }

    #[tokio::test]
    async fn refresh_rejects_unknown_refresh_token() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Unknown refresh tenant", "unknown-refresh-tenant")
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let config = test_auth_config("refresh-secret", 600, 3600);

        let result = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant.id,
            "unknown-refresh-token",
            false, None, None,
        )
        .await;

        assert!(matches!(
            result,
            Err(AuthLifecycleError::InvalidRefreshToken)
        ));
    }

    #[tokio::test]
    async fn refresh_never_issues_token_for_user_from_other_tenant() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_a = tenants::ActiveModel::new("Refresh Tenant A", "refresh-boundary-a")
            .insert(&db)
            .await
            .expect("create tenant A");
        let tenant_b = tenants::ActiveModel::new("Refresh Tenant B", "refresh-boundary-b")
            .insert(&db)
            .await
            .expect("create tenant B");
        let user_b = users::ActiveModel::new(tenant_b.id, "refresh-boundary@example.com", "hash")
            .insert(&db)
            .await
            .expect("create tenant B user");

        let refresh_token = "cross-tenant-refresh-token";
        sessions::ActiveModel::new(
            tenant_a.id,
            user_b.id,
            hash_refresh_token(refresh_token),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("insert mismatched session for boundary test");

        let config = test_auth_config("refresh-boundary-secret", 600, 3600);
        let result = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant_a.id,
            refresh_token,
            false, None, None,
        )
        .await;

        assert!(
            matches!(result, Err(AuthLifecycleError::UserNotFound)),
            "a refresh session must never authorize a user from another tenant"
        );
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

        let config = test_auth_config("refresh-secret", 600, 3600);

        let result = AuthLifecycleService::refresh_with_config_db(
            &db,
            &config,
            tenant.id,
            refresh_token,
            false, None, None,
        )
        .await;

        assert!(matches!(result, Err(AuthLifecycleError::UserInactive)));
    }

    #[test]
    fn confirm_password_reset_rejects_invalid_token_payload() {
        let config = test_auth_config("reset-secret", 3600, 7200);

        let err = crate::auth::decode_password_reset_token(&config, "not-a-jwt")
            .expect_err("invalid token payload must be rejected");

        assert!(matches!(err, crate::error::Error::Unauthorized(_)));
    }

    #[tokio::test]
    async fn refresh_acquires_user_fence_before_session_rotation() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Fence tenant", "refresh-user-fence")
            .insert(&db).await.expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "fence@example.com", "hash")
            .insert(&db).await.expect("user");
        let raw_token = "refresh-user-fence-token";
        let digest = hash_refresh_token(raw_token);
        let session = sessions::ActiveModel::new(
            tenant.id, user.id, digest.clone(), Utc::now() + Duration::hours(1), None, None,
        )
        .insert(&db).await.expect("session");

        db.execute_unprepared("CREATE TABLE refresh_fence_probe (seen INTEGER NOT NULL)")
            .await.expect("create probe");
        db.execute_unprepared(
            "CREATE TRIGGER record_refresh_user_fence AFTER UPDATE ON users \
             BEGIN INSERT INTO refresh_fence_probe VALUES (1); END;",
        )
        .await.expect("record user fence");
        db.execute_unprepared(
            "CREATE TRIGGER require_refresh_user_fence BEFORE UPDATE ON sessions \
             WHEN NOT EXISTS (SELECT 1 FROM refresh_fence_probe) \
             BEGIN SELECT RAISE(ABORT, 'session locked before user'); END;",
        )
        .await.expect("require user fence");
        let config = test_auth_config("refresh-user-fence", 600, 3600);
        let (_, tokens) = AuthLifecycleService::refresh_with_config_db(
            &db, &config, tenant.id, raw_token, false, None, None,
        )
        .await.expect("user fence precedes session lock");
        let persisted = sessions::Entity::find_by_id(session.id)
            .one(&db).await.expect("read").expect("session");
        assert_eq!(persisted.token_hash, hash_refresh_token(&tokens.refresh_token));
        assert_eq!(persisted.previous_token_hash, Some(digest));
    }

    #[tokio::test]
    async fn revoked_current_session_cannot_revoke_remaining_sessions() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Stale tenant", "stale-bulk-revoke")
            .insert(&db).await.expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "stale@example.com", "hash")
            .insert(&db).await.expect("user");
        let mut stale = sessions::ActiveModel::new(
            tenant.id, user.id, "stale-token".to_string(),
            Utc::now() + Duration::hours(1), None, None,
        );
        stale.revoked_at = Set(Some(Utc::now().into()));
        let stale = stale.insert(&db).await.expect("revoked session");
        let other = sessions::ActiveModel::new(
            tenant.id, user.id, "other-token".to_string(),
            Utc::now() + Duration::hours(1), None, None,
        )
        .insert(&db).await.expect("active session");

        let result = AuthLifecycleService::revoke_user_sessions_db(
            &db, tenant.id, user.id, Some(stale.id),
        )
        .await;
        assert!(matches!(result, Err(AuthLifecycleError::SessionExpired)));
        let persisted = sessions::Entity::find_by_id(other.id)
            .one(&db).await.expect("read").expect("session");
        assert!(persisted.is_active());
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
        let read_sessions = || {
            sessions::Entity::find()
                .filter(sessions::Column::TenantId.eq(tenant.id))
                .filter(sessions::Column::UserId.eq(user.id))
                .order_by_asc(sessions::Column::Id)
        };
        let revoked = read_sessions().all(&db).await.expect("revoked sessions");
        assert_eq!(revoked.len(), 2);
        for session in &revoked {
            assert_eq!(session.revoked_at, Some(session.updated_at));
        }
        let second = AuthLifecycleService::revoke_user_sessions_db(&db, tenant.id, user.id, None)
            .await
            .expect("second revoke should succeed");

        assert_eq!(first, 2);
        assert_eq!(second, 0);
        assert_eq!(read_sessions().all(&db).await.expect("unchanged sessions"), revoked);
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
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
            rustok_auth::MIN_PASSWORD_LENGTH_FLOOR_CHARS,
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

    // ── Phase 1.5: logout / list_sessions / revoke_session / revoke_all_other_sessions ──

    #[tokio::test]
    #[serial]
    async fn logout_marks_session_revoked() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Logout Tenant", "logout-tenant")
            .insert(&db)
            .await
            .expect("create tenant");
        let user = users::ActiveModel::new(tenant.id, "logout@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user");
        let now = Utc::now();
        let session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-logout".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create session");

        // Exercise the same revocation predicate as logout_db and verify idempotency.
        let result = sessions::Entity::update_many()
            .col_expr(
                sessions::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::Id.eq(session.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .exec(&db)
            .await
            .expect("update succeeded");

        assert_eq!(result.rows_affected, 1, "session should be revoked");

        // A second call on already-revoked session should affect 0 rows (idempotent)
        let result2 = sessions::Entity::update_many()
            .col_expr(
                sessions::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::Id.eq(session.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .exec(&db)
            .await
            .expect("second update succeeded");

        assert_eq!(
            result2.rows_affected, 0,
            "already-revoked session is skipped"
        );
    }

    #[tokio::test]
    #[serial]
    async fn logout_by_session_id_revokes_under_lock_and_is_idempotent() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Logout by session id", "logout-by-session-id")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "logout-by-id@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token("logout-by-id-refresh-token"),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("session");

        AuthLifecycleService::logout_db(&ServerRuntimeContext::new(db.clone(), Default::default()), tenant.id, session.id)
            .await
            .expect("logout active session");

        let revoked = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .expect("read revoked session")
            .expect("session remains as revocation evidence");
        let revoked_at = revoked.revoked_at.expect("session was revoked");
        assert_eq!(
            revoked.token_hash,
            hash_refresh_token("logout-by-id-refresh-token"),
            "logout must not overwrite the current token digest"
        );

        AuthLifecycleService::logout_db(&ServerRuntimeContext::new(db.clone(), Default::default()), tenant.id, session.id)
            .await
            .expect("repeated logout is idempotent");
        let after_repeat = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .expect("read session after repeated logout")
            .expect("session remains present");
        assert_eq!(after_repeat.revoked_at, Some(revoked_at));
    }

    #[tokio::test]
    #[serial]
    async fn logout_with_previous_cookie_revokes_the_rotated_active_session() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Lost refresh response", "lost-refresh-response")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "lost-response@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");

        let old_cookie = "cookie-before-rotation";
        let mut active = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token("new-cookie-not-yet-received"),
            Utc::now() + Duration::hours(1),
            None,
            None,
        );
        active.previous_token_hash = Set(Some(hash_refresh_token(old_cookie)));
        active.previous_token_revoked_at = Set(Some(Utc::now().into()));
        let session = active.insert(&db).await.expect("rotated session");

        let runtime =
            ServerRuntimeContext::new(db.clone(), crate::common::RustokSettings::default());
        AuthLifecycleService::logout_by_refresh_token_runtime(&runtime, tenant.id, old_cookie, None)
            .await
            .expect("previous cookie still revokes the current session");

        let stored = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .expect("read session")
            .expect("session exists");
        assert!(stored.revoked_at.is_some());

        assert!(matches!(
            AuthLifecycleService::logout_by_refresh_token_runtime(&runtime, tenant.id, old_cookie, None)
                .await,
            Err(AuthLifecycleError::InvalidRefreshToken)
        ));
    }

    #[tokio::test]
    #[serial]
    async fn logout_by_refresh_token_revokes_expired_unrevoked_sessions() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Expired Logout Tenant", "expired-logout-tenant")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "expired-logout@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        let now = Utc::now();

        let sliding_token = "expired-sliding-refresh-token";
        let sliding = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token(sliding_token),
            now - Duration::minutes(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("expired sliding session");

        let absolute_token = "expired-absolute-refresh-token";
        let mut absolute_model = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            hash_refresh_token(absolute_token),
            now + Duration::hours(1),
            None,
            None,
        );
        absolute_model.absolute_expires_at = Set(Some((now - Duration::minutes(1)).into()));
        let absolute = absolute_model
            .insert(&db)
            .await
            .expect("absolute-expired session");

        let runtime = ServerRuntimeContext::new(
            db.clone(),
            crate::common::RustokSettings::default(),
        );
        AuthLifecycleService::logout_by_refresh_token_runtime(
            &runtime, tenant.id, sliding_token, None,
        )
        .await
        .expect("expired sliding session remains revocable");

        AuthLifecycleService::logout_by_refresh_token_runtime(
            &runtime, tenant.id, absolute_token, Some(absolute.id),
        )
        .await
        .expect("expired absolute session remains revocable");

        for session_id in [sliding.id, absolute.id] {
            let stored = sessions::Entity::find_by_id(session_id)
                .one(&db)
                .await
                .expect("read session")
                .expect("session");
            assert!(stored.revoked_at.is_some(), "expired session must be revoked");
        }
    }

    #[tokio::test]
    #[serial]
    async fn logout_by_refresh_token_is_tenant_scoped_and_single_transaction() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_a = tenants::ActiveModel::new("Logout Token Tenant A", "logout-token-a")
            .insert(&db)
            .await
            .expect("create tenant A");
        let tenant_b = tenants::ActiveModel::new("Logout Token Tenant B", "logout-token-b")
            .insert(&db)
            .await
            .expect("create tenant B");

        let user_a = users::ActiveModel::new(tenant_a.id, "logout-token-a@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user A");
        let user_b = users::ActiveModel::new(tenant_b.id, "logout-token-b@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user B");

        let refresh_token = "logout-refresh-token";
        let token_hash = hash_refresh_token(refresh_token);
        let now = Utc::now();

        let session_a = sessions::ActiveModel::new(
            tenant_a.id,
            user_a.id,
            token_hash.clone(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create tenant A session");
        let session_b = sessions::ActiveModel::new(
            tenant_b.id,
            user_b.id,
            token_hash,
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create tenant B session");

        let runtime =
            ServerRuntimeContext::new(db.clone(), crate::common::RustokSettings::default());

        AuthLifecycleService::logout_by_refresh_token_runtime(&runtime, tenant_a.id, refresh_token, None)
            .await
            .expect("tenant A logout should succeed");

        let session_a = sessions::Entity::find_by_id(session_a.id)
            .one(&db)
            .await
            .expect("read tenant A session")
            .expect("tenant A session should exist");
        let session_b = sessions::Entity::find_by_id(session_b.id)
            .one(&db)
            .await
            .expect("read tenant B session")
            .expect("tenant B session should exist");

        assert!(session_a.revoked_at.is_some());
        assert!(session_b.revoked_at.is_none());

        assert!(matches!(
            AuthLifecycleService::logout_by_refresh_token_runtime(
                &runtime,
                tenant_a.id,
                refresh_token,
                None,
            )
            .await,
            Err(AuthLifecycleError::InvalidRefreshToken)
        ));
    }

    #[tokio::test]
    #[serial]
    async fn list_sessions_returns_only_active_non_expired() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("List Sessions Tenant", "list-sessions-tenant")
            .insert(&db)
            .await
            .expect("create tenant");
        let user = users::ActiveModel::new(tenant.id, "sessions@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user");
        let now = Utc::now();

        // Active session (future expiry, not revoked)
        let active = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-active".to_string(),
            now + Duration::hours(2),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create active session");

        // Expired session (past expiry)
        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-expired".to_string(),
            now - Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create expired session");

        // Revoked session
        let revoked_session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-revoked".to_string(),
            now + Duration::hours(3),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create to-be-revoked session");
        sessions::Entity::update_many()
            .col_expr(
                sessions::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(sessions::Column::Id.eq(revoked_session.id))
            .exec(&db)
            .await
            .expect("revoke session");

        // An explicitly expired absolute lineage must not appear as active,
        // even if its sliding expiry is still in the future.
        let mut expired_absolute = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-expired-absolute".to_string(),
            now + Duration::hours(2),
            None,
            None,
        );
        expired_absolute.absolute_expires_at = Set(Some((now - Duration::days(1)).into()));
        expired_absolute
            .insert(&db)
            .await
            .expect("create absolute-expired session");

        // Future-created rows are not active, even when their sliding and
        // persisted absolute deadlines are both in the future.
        let future_created_at = now + Duration::minutes(1);
        let mut future_created = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-future-created".to_string(),
            now + Duration::hours(2),
            None,
            None,
        );
        future_created.created_at = Set(future_created_at.clone().into());
        future_created.updated_at = Set(future_created_at.into());
        future_created
            .insert(&db)
            .await
            .expect("create future-dated session");

        // Pre-migration rows had no absolute deadline column; they still
        // expire 90 days after their original creation.
        let created_at = now - Duration::days(sessions::SESSION_ABSOLUTE_LIFETIME_DAYS + 1);
        let mut expired_legacy = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-expired-legacy".to_string(),
            now + Duration::hours(2),
            None,
            None,
        );
        expired_legacy.absolute_expires_at = Set(None);
        expired_legacy.created_at = Set(created_at.into());
        expired_legacy.updated_at = Set(created_at.into());
        expired_legacy
            .insert(&db)
            .await
            .expect("create legacy expired session");

        // The test must exercise the actual service, not duplicate its SQL
        // filters: otherwise newly introduced validity fences go untested.
        let rows = AuthLifecycleService::list_sessions_db(&db, tenant.id, user.id, 50)
            .await
            .expect("list sessions");

        assert_eq!(rows.len(), 1, "only the active session should be returned");
        assert_eq!(rows[0].id, active.id);
    }

    #[tokio::test]
    #[serial]
    async fn list_sessions_enforces_limit() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Limit Tenant", "list-sessions-limit-tenant")
            .insert(&db)
            .await
            .expect("create tenant");
        let user = users::ActiveModel::new(tenant.id, "limit@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user");
        let now = Utc::now();

        for i in 0..5u32 {
            sessions::ActiveModel::new(
                tenant.id,
                user.id,
                format!("tok-{i}"),
                now + Duration::hours(i as i64 + 1),
                None,
                None,
            )
            .insert(&db)
            .await
            .unwrap_or_else(|_| panic!("create session {i}"));
        }

        let rows = AuthLifecycleService::list_sessions_db(&db, tenant.id, user.id, 3)
            .await
            .expect("list sessions with limit");

        assert_eq!(rows.len(), 3, "limit=3 should return at most 3 rows");
    }

    #[tokio::test]
    #[serial]
    async fn revoke_session_returns_true_when_found_and_false_when_not() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Revoke Session Tenant", "revoke-session-tenant")
            .insert(&db)
            .await
            .expect("create tenant");
        let user = users::ActiveModel::new(tenant.id, "revoke@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user");
        let now = Utc::now();
        let session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-revoke".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create session");

        // First revocation: should affect 1 row
        let result = sessions::Entity::update_many()
            .col_expr(
                sessions::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::Id.eq(session.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .exec(&db)
            .await
            .expect("revoke session");
        assert!(
            result.rows_affected > 0,
            "first revocation should return true"
        );

        // Second revocation of already-revoked session: 0 rows affected
        let result2 = sessions::Entity::update_many()
            .col_expr(
                sessions::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::Id.eq(session.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .exec(&db)
            .await
            .expect("re-revoke session");
        assert_eq!(
            result2.rows_affected, 0,
            "second revocation should return false"
        );
    }

    #[tokio::test]
    #[serial]
    async fn revoke_session_cross_user_ownership_check() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Xuser Tenant", "revoke-xuser-tenant")
            .insert(&db)
            .await
            .expect("create tenant");
        let owner = users::ActiveModel::new(tenant.id, "owner@example.com", "hash")
            .insert(&db)
            .await
            .expect("create owner");
        let attacker = users::ActiveModel::new(tenant.id, "attacker@example.com", "hash2")
            .insert(&db)
            .await
            .expect("create attacker");
        let now = Utc::now();
        let session = sessions::ActiveModel::new(
            tenant.id,
            owner.id,
            "tok-owner".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create session");

        // Attacker tries to revoke owner's session (user_id filter mismatch)
        let result = sessions::Entity::update_many()
            .col_expr(
                sessions::Column::RevokedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(attacker.id)) // wrong user
            .filter(sessions::Column::Id.eq(session.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .exec(&db)
            .await
            .expect("cross-user revoke attempt");

        assert_eq!(
            result.rows_affected, 0,
            "attacker must not revoke another user's session"
        );

        // Owner's session should still be active
        let still_active = sessions::Entity::find()
            .filter(sessions::Column::Id.eq(session.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .one(&db)
            .await
            .expect("query session");
        assert!(
            still_active.is_some(),
            "session should remain active after failed cross-user revocation"
        );
    }

    #[tokio::test]
    #[serial]
    async fn revoke_all_other_sessions_preserves_current_session() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant = tenants::ActiveModel::new("Revoke All Tenant", "revoke-all-sessions-tenant")
            .insert(&db)
            .await
            .expect("create tenant");
        let user = users::ActiveModel::new(tenant.id, "revoke-all@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user");
        let now = Utc::now();

        let current = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-current".to_string(),
            now + Duration::hours(3),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create current session");

        let other1 = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-other-1".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create other session 1");

        sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "tok-other-2".to_string(),
            now + Duration::hours(2),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create other session 2");

        let revoked = AuthLifecycleService::revoke_user_sessions_db(
            &db,
            tenant.id,
            user.id,
            Some(current.id),
        )
        .await
        .expect("revoke all other sessions through service");
        assert_eq!(revoked, 2, "both other sessions should be revoked");

        // Current session must still be active
        let still_active = sessions::Entity::find()
            .filter(sessions::Column::Id.eq(current.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .one(&db)
            .await
            .expect("query current session");
        assert!(
            still_active.is_some(),
            "current session must survive revoke_all_other_sessions"
        );

        // other1 must be revoked
        let revoked = sessions::Entity::find()
            .filter(sessions::Column::Id.eq(other1.id))
            .filter(sessions::Column::RevokedAt.is_not_null())
            .one(&db)
            .await
            .expect("query revoked session");
        assert!(revoked.is_some(), "other sessions must be revoked");
    }

    #[tokio::test]
    #[serial]
    async fn revoke_all_other_sessions_isolates_across_tenants() {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let tenant_a = tenants::ActiveModel::new("Tenant A Sessions", "revoke-all-tenant-a")
            .insert(&db)
            .await
            .expect("create tenant A");
        let tenant_b = tenants::ActiveModel::new("Tenant B Sessions", "revoke-all-tenant-b")
            .insert(&db)
            .await
            .expect("create tenant B");
        let user_a = users::ActiveModel::new(tenant_a.id, "usera@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user A");
        let user_b = users::ActiveModel::new(tenant_b.id, "userb@example.com", "hash")
            .insert(&db)
            .await
            .expect("create user B");
        let now = Utc::now();

        let session_a = sessions::ActiveModel::new(
            tenant_a.id,
            user_a.id,
            "tok-a".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create session A");

        let session_b = sessions::ActiveModel::new(
            tenant_b.id,
            user_b.id,
            "tok-b".to_string(),
            now + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("create session B");

        // Exercise the tenant-scoped service rather than duplicating its SQL.
        let revoked = AuthLifecycleService::revoke_user_sessions_db(
            &db, tenant_a.id, user_a.id, None,
        )
        .await
        .expect("revoke tenant A sessions through service");
        assert_eq!(revoked, 1);

        let a_revoked = sessions::Entity::find_by_id(session_a.id)
            .one(&db)
            .await
            .expect("query session A")
            .expect("session A");
        assert!(a_revoked.revoked_at.is_some());

        // session_b must still be active
        let b_still_active = sessions::Entity::find()
            .filter(sessions::Column::Id.eq(session_b.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .one(&db)
            .await
            .expect("query session B");
        assert!(
            b_still_active.is_some(),
            "tenant B session must not be affected"
        );

        // session_a should be revoked
        let a_revoked = sessions::Entity::find()
            .filter(sessions::Column::Id.eq(session_a.id))
            .filter(sessions::Column::RevokedAt.is_not_null())
            .one(&db)
            .await
            .expect("query session A");
        assert!(a_revoked.is_some(), "tenant A session should be revoked");
    }
}
