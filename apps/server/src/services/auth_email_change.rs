use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait,
    sea_query::Expr,
};
use uuid::Uuid;

use crate::auth::{
    generate_refresh_token, hash_refresh_token, normalize_email, verify_password_async,
};
use crate::models::{
    auth_email_change_requests::{self, Entity as AuthEmailChangeRequests},
    users::{self, Entity as Users},
};
use crate::services::email::{
    EMAIL_CHANGE_TOKEN_TTL_SECS, auth_email_delivery_enabled, email_service_from_ctx,
    spawn_email_change_confirmation_send, spawn_email_change_notification_send,
};
use crate::services::server_runtime_context::ServerRuntimeContext;

use super::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfirmedEmailChange {
    pub old_email: String,
    pub new_email: String,
}

impl AuthLifecycleService {
    /// Re-authenticate with the current password, then issue a short-lived,
    /// one-time opaque challenge to the requested mailbox.
    pub async fn request_email_change_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        current_password: &str,
        requested_email: &str,
        locale: &str,
    ) -> Result<(), AuthLifecycleError> {
        rustok_auth::application_clock::admit_now(ctx.db()).await?;
        let new_email = normalize_email(requested_email).map_err(AuthLifecycleError::from)?;

        let initial_user = Users::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(ctx.db())
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidCredentials)?;

        if !initial_user.is_active() {
            return Err(AuthLifecycleError::UserInactive);
        }
        if initial_user.email == new_email {
            return Err(AuthLifecycleError::Validation(
                "New email must differ from the current email".to_string(),
            ));
        }

        if !verify_password_async(
            current_password.to_string(),
            initial_user.password_hash.clone(),
        )
        .await
        .map_err(AuthLifecycleError::from)?
        {
            return Err(AuthLifecycleError::InvalidCredentials);
        }

        if !auth_email_delivery_enabled(ctx) {
            return Err(AuthLifecycleError::EmailDeliveryUnavailable);
        }
        let email_service = email_service_from_ctx(ctx, locale)
            .map_err(|_| AuthLifecycleError::EmailDeliveryUnavailable)?;

        let token = generate_refresh_token().map_err(AuthLifecycleError::from)?;
        let token_hash = hash_refresh_token(&token);

        let expires_at = Self::create_email_change_request_db(
            ctx.db(),
            tenant_id,
            user_id,
            session_id,
            &initial_user,
            &new_email,
            &token_hash,
        )
        .await?;

        let _task = spawn_email_change_confirmation_send(
            ctx,
            email_service,
            new_email,
            token,
            expires_at,
            tenant_id,
            user_id,
        );
        Ok(())
    }

    /// Confirm the exact challenge while holding the user mutation fence.
    /// The old address, password-change timestamp and recovery revision bind
    /// the challenge to the identity state observed when it was requested.
    pub async fn confirm_email_change_runtime(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
        token: &str,
        authenticated_user_id: Option<Uuid>,
        current_session_id: Option<Uuid>,
        locale: &str,
    ) -> Result<String, AuthLifecycleError> {
        if !is_canonical_email_change_token(token) {
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }

        let token_hash = hash_refresh_token(token);
        let confirmed = Self::confirm_email_change_db(
            ctx,
            tenant_id,
            &token_hash,
            authenticated_user_id,
            current_session_id,
        )
        .await?;

        // The identity transition is already committed. Notification failure
        // must not roll it back or make a consumed token appear reusable.
        if auth_email_delivery_enabled(ctx) {
            match email_service_from_ctx(ctx, locale) {
                Ok(email_service) => {
                    let _task = spawn_email_change_notification_send(
                        ctx,
                        email_service,
                        confirmed.old_email,
                        confirmed.new_email.clone(),
                    );
                }
                Err(_) => {
                    tracing::warn!(
                        error_code = "auth_email_change_notification_unavailable",
                        "Could not prepare the previous-mailbox security notification"
                    );
                }
            }
        }

        Ok(confirmed.new_email)
    }

    async fn create_email_change_request_db(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        user_id: Uuid,
        session_id: Uuid,
        observed_user: &users::Model,
        new_email: &str,
        token_hash: &str,
    ) -> Result<chrono::DateTime<Utc>, AuthLifecycleError> {
        // Reclaim only the requested mailbox, before locking the requester.
        // Never hold two user mutation fences in one issuance transaction.
        reclaim_stale_mailbox_reservation(db, tenant_id, user_id, new_email).await?;

        let tx = db.begin().await.map_err(AuthLifecycleError::from)?;
        let locked_user = Self::lock_user_for_mutation(&tx, tenant_id, user_id)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidCredentials)?;

        if !locked_user.is_active() {
            return Err(AuthLifecycleError::UserInactive);
        }
        if locked_user.email != observed_user.email
            || locked_user.password_hash != observed_user.password_hash
            || locked_user.password_changed_at != observed_user.password_changed_at
            || locked_user.recovery_revision != observed_user.recovery_revision
        {
            return Err(AuthLifecycleError::InvalidCredentials);
        }

        let session = Self::find_current_session_for_password_change_in_tx(
            &tx, tenant_id, user_id, session_id,
        )
        .await?
        .ok_or(AuthLifecycleError::SessionExpired)?;
        let now = rustok_auth::application_clock::verified_now(&tx).await?;
        if !session.is_active_at(now) {
            return Err(AuthLifecycleError::SessionExpired);
        }

        if locked_user.email == new_email {
            return Err(AuthLifecycleError::Validation(
                "New email must differ from the current email".to_string(),
            ));
        }

        let existing_user = Users::find()
            .filter(users::Column::TenantId.eq(tenant_id))
            .filter(users::Column::Email.eq(new_email))
            .one(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;
        if existing_user.is_some_and(|user| user.id != user_id) {
            return Err(AuthLifecycleError::EmailAlreadyExists);
        }

        // Replace only this user's previous challenge. The retention worker
        // owns broad expiry cleanup; the unique tenant/email index still
        // serializes concurrent issuance for the same address.
        AuthEmailChangeRequests::delete_many()
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
            .filter(auth_email_change_requests::Column::UserId.eq(user_id))
            .exec(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;

        // Cleanup and uniqueness checks can wait on other transactions. Start
        // the challenge lifetime only after those waits, and recheck the
        // locked session before issuing a new credential.
        let now = rustok_auth::application_clock::verified_now(&tx).await?;
        if !session.is_active_at(now) {
            return Err(AuthLifecycleError::SessionExpired);
        }
        let ttl_seconds = i64::try_from(EMAIL_CHANGE_TOKEN_TTL_SECS)
            .map_err(|_| AuthLifecycleError::Internal(crate::error::Error::InternalServerError))?;
        let expires_at = now
            .clone()
            .checked_add_signed(Duration::seconds(ttl_seconds))
            .ok_or(AuthLifecycleError::Internal(
                crate::error::Error::InternalServerError,
            ))?;
        let request = auth_email_change_requests::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            user_id: Set(user_id),
            current_email: Set(locked_user.email),
            new_email: Set(new_email.to_string()),
            password_changed_at: Set(locked_user.password_changed_at),
            recovery_revision: Set(Some(locked_user.recovery_revision)),
            token_hash: Set(token_hash.to_string()),
            expires_at: Set(expires_at.clone().into()),
            created_at: Set(now.into()),
        };

        let stored = request.insert(&tx).await.map_err(|error| {
            if matches!(
                error.sql_err(),
                Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
            ) {
                AuthLifecycleError::EmailAlreadyExists
            } else {
                AuthLifecycleError::from(error)
            }
        })?;

        let expires_at = expires_at.min(stored.expires_at.with_timezone(&Utc));
        // A competing mailbox claim may also delay the insert itself. Do
        // not commit a challenge that is already unusable or was issued by
        // a session that expired during the wait. Rollback preserves any
        // earlier pending request deleted in this transaction.
        let final_now = rustok_auth::application_clock::verified_now(&tx).await?;
        if !session.is_active_at(final_now) {
            return Err(AuthLifecycleError::SessionExpired);
        }
        if final_now >= expires_at {
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }
        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(expires_at)
    }

    async fn confirm_email_change_db(
        ctx: &ServerRuntimeContext,
        tenant_id: Uuid,
        token_hash: &str,
        authenticated_user_id: Option<Uuid>,
        current_session_id: Option<Uuid>,
    ) -> Result<ConfirmedEmailChange, AuthLifecycleError> {
        let db = ctx.db();
        let observed_at = rustok_auth::application_clock::admit_now(db).await?;

        // Read only the owner ID before opening the transaction. Every
        // mutation then locks user first and challenge second, matching the
        // request path's lock order and avoiding a user/challenge deadlock.
        let observed_request = AuthEmailChangeRequests::find()
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
            .filter(auth_email_change_requests::Column::TokenHash.eq(token_hash))
            .filter(auth_email_change_requests::Column::CreatedAt.lte(observed_at))
            .filter(auth_email_change_requests::Column::ExpiresAt.gt(observed_at))
            .one(db)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidEmailChangeToken)?;

        if authenticated_user_id.is_some() != current_session_id.is_some() {
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }

        if authenticated_user_id.is_some_and(|user_id| user_id != observed_request.user_id) {
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }

        let tx = db.begin().await.map_err(AuthLifecycleError::from)?;
        let user = Self::lock_user_for_mutation(&tx, tenant_id, observed_request.user_id)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidEmailChangeToken)?;

        // Re-evaluate expiry after acquiring the user mutation fence. A
        // request that waited behind another mutation must not be accepted
        // using the timestamp captured before that lock wait.
        let confirmation_now = rustok_auth::application_clock::verified_now(&tx).await?;
        let request = lock_email_change_request(
            &tx,
            tenant_id,
            observed_request.id,
            observed_request.user_id,
            token_hash,
            confirmation_now,
        )
        .await?
        .ok_or(AuthLifecycleError::InvalidEmailChangeToken)?;

        if !email_change_request_matches_user(&request, &user) {
            delete_email_change_request(&tx, request.id, token_hash).await?;
            tx.commit().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }

        let proving_session = if let Some(session_id) = current_session_id {
            Some(
                Self::find_current_session_for_password_change_in_tx(
                    &tx, tenant_id, user.id, session_id,
                )
                .await?
                .ok_or(AuthLifecycleError::SessionExpired)?,
            )
        } else {
            None
        };

        // Expiry is checked again after all row-lock waits and session
        // validation. The unique tenant/email index decides mailbox ownership
        // at the actual write, not at a racy availability preflight.
        let transition_at = rustok_auth::application_clock::verified_now(&tx).await?;
        if proving_session
            .as_ref()
            .is_some_and(|session| !session.is_active_at(transition_at))
        {
            return Err(AuthLifecycleError::SessionExpired);
        }
        if request.created_at.with_timezone(&Utc) > transition_at {
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }
        if request.expires_at.with_timezone(&Utc) <= transition_at {
            delete_email_change_request(&tx, request.id, token_hash).await?;
            tx.commit().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }

        let old_email = user.email.clone();
        let new_email = request.new_email.clone();
        let mut active: users::ActiveModel = user.clone().into();
        active.email = Set(new_email.clone());
        let updated_at: chrono::DateTime<chrono::FixedOffset> = transition_at.clone().into();
        active.email_verified_at = Set(Some(updated_at.clone()));
        active.updated_at = Set(updated_at);
        // PostgreSQL aborts the current transaction on a constraint error.
        // Isolate the mailbox write in a savepoint so a competing claim can
        // consume this exact challenge without committing any identity change.
        // Only email (the mutable part of the tenant/email unique key) changes
        // among the user's unique columns.
        let mailbox_tx = tx.begin().await.map_err(AuthLifecycleError::from)?;
        match active.update(&mailbox_tx).await {
            Ok(_) => mailbox_tx
                .commit()
                .await
                .map_err(AuthLifecycleError::from)?,
            Err(error) => {
                mailbox_tx
                    .rollback()
                    .await
                    .map_err(AuthLifecycleError::from)?;
                if !matches!(
                    error.sql_err(),
                    Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
                ) {
                    return Err(AuthLifecycleError::from(error));
                }
                // The unique-index wait may have outlived either credential.
                // Do not let an expired proving session consume the challenge.
                let conflict_now = rustok_auth::application_clock::verified_now(&tx).await?;
                if proving_session
                    .as_ref()
                    .is_some_and(|session| !session.is_active_at(conflict_now))
                {
                    return Err(AuthLifecycleError::SessionExpired);
                }
                if request.created_at.with_timezone(&Utc) > conflict_now {
                    return Err(AuthLifecycleError::InvalidEmailChangeToken);
                }
                delete_email_change_request(&tx, request.id, token_hash).await?;
                tx.commit().await.map_err(AuthLifecycleError::from)?;
                return Err(if request.expires_at.with_timezone(&Utc) <= conflict_now {
                    AuthLifecycleError::InvalidEmailChangeToken
                } else {
                    AuthLifecycleError::EmailAlreadyExists
                });
            }
        }

        // Keep only the session that proved the current user identity. A
        // token-only confirmation revokes every session. Delegated OAuth
        // grants are always revoked because their subject email changed.
        let _revoked_sessions = Self::revoke_user_sessions_db_with_connection(
            &tx,
            tenant_id,
            user.id,
            current_session_id,
        )
        .await?;
        let _revoked_oauth_tokens =
            crate::services::oauth_token_service::revoke_user_delegated_tokens_in_tx(
                &tx,
                tenant_id,
                user.id,
                transition_at,
            )
            .await
            .map_err(AuthLifecycleError::from)?;

        delete_email_change_request(&tx, request.id, token_hash).await?;
        Self::publish_user_event_in_tx(
            ctx,
            &tx,
            tenant_id,
            user.id,
            rustok_events::DomainEvent::UserUpdated { user_id: user.id },
        )
        .await?;

        // The mailbox uniqueness check, revocations and outbox writes may
        // block. Roll back every side effect if admission expired meanwhile.
        let final_now = rustok_auth::application_clock::verified_now(&tx).await?;
        if request.created_at.with_timezone(&Utc) > final_now
            || request.expires_at.with_timezone(&Utc) <= final_now
        {
            tx.rollback().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }
        if proving_session
            .as_ref()
            .is_some_and(|session| !session.is_active_at(final_now))
        {
            tx.rollback().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::SessionExpired);
        }
        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(ConfirmedEmailChange {
            old_email,
            new_email,
        })
    }
}

/// Read the exact still-pending challenge and its owner in one statement.
/// Delivery is best-effort: confirmation remains the locked authority.
pub(super) async fn pending_email_change_window(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
    recipient: &str,
    token_hash: &str,
) -> Result<Option<(chrono::DateTime<Utc>, chrono::DateTime<Utc>)>, sea_orm::DbErr> {
    let record = AuthEmailChangeRequests::find()
        .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
        .filter(auth_email_change_requests::Column::UserId.eq(user_id))
        .filter(auth_email_change_requests::Column::NewEmail.eq(recipient))
        .filter(auth_email_change_requests::Column::TokenHash.eq(token_hash))
        .find_also_related(Users)
        .one(db)
        .await?;
    Ok(record.and_then(|(request, owner)| {
        owner
            .filter(|user| email_change_request_matches_user(&request, user))
            .map(|_| {
                (
                    request.created_at.with_timezone(&Utc),
                    request.expires_at.with_timezone(&Utc),
                )
            })
    }))
}

/// The same account-state predicate governs confirmation, reclamation and dispatch.
fn email_change_request_matches_user(
    request: &auth_email_change_requests::Model,
    user: &users::Model,
) -> bool {
    request.tenant_id == user.tenant_id
        && request.user_id == user.id
        && user.is_active()
        && user.email == request.current_email
        && user.password_changed_at == request.password_changed_at
        && request.recovery_revision == Some(user.recovery_revision)
}

/// Release an expired or irreversibly invalid challenge for the exact target
/// address. Lock the observed owner first, then re-read the exact challenge;
/// a replacement issued during the preflight must never be deleted.
/// This commits independently of the subsequent request, which still checks
/// the requester's password snapshot, session and mailbox uniqueness.
async fn reclaim_stale_mailbox_reservation(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    requester_id: Uuid,
    new_email: &str,
) -> Result<(), AuthLifecycleError> {
    let observed = AuthEmailChangeRequests::find()
        .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
        .filter(auth_email_change_requests::Column::NewEmail.eq(new_email))
        .one(db)
        .await
        .map_err(AuthLifecycleError::from)?;
    let Some(observed) = observed.filter(|request| request.user_id != requester_id) else {
        return Ok(());
    };

    let tx = db.begin().await.map_err(AuthLifecycleError::from)?;
    let owner = AuthLifecycleService::lock_user_for_mutation(&tx, tenant_id, observed.user_id)
        .await
        .map_err(AuthLifecycleError::from)?;

    // All issuers and confirmers hold the owner fence before changing a
    // challenge. Retention can only delete it, not modify or replace it.
    let exact_request = AuthEmailChangeRequests::find_by_id(observed.id)
        .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
        .filter(auth_email_change_requests::Column::UserId.eq(observed.user_id))
        .filter(auth_email_change_requests::Column::NewEmail.eq(new_email))
        .filter(auth_email_change_requests::Column::TokenHash.eq(&observed.token_hash))
        .one(&tx)
        .await
        .map_err(AuthLifecycleError::from)?;
    if let Some(request) = exact_request {
        let now = rustok_auth::application_clock::verified_now(&tx).await?;
        if request.expires_at.with_timezone(&Utc) <= now
            || !owner
                .as_ref()
                .is_some_and(|user| email_change_request_matches_user(&request, user))
        {
            // Retention may already have removed the same row. Zero affected
            // rows is harmless here, unlike one-shot credential consumption.
            AuthEmailChangeRequests::delete_many()
                .filter(auth_email_change_requests::Column::Id.eq(request.id))
                .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
                .filter(auth_email_change_requests::Column::UserId.eq(request.user_id))
                .filter(auth_email_change_requests::Column::NewEmail.eq(new_email))
                .filter(auth_email_change_requests::Column::TokenHash.eq(&request.token_hash))
                .exec(&tx)
                .await
                .map_err(AuthLifecycleError::from)?;
        }
    }
    tx.commit().await.map_err(AuthLifecycleError::from)?;
    Ok(())
}

fn is_canonical_email_change_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

async fn lock_email_change_request(
    tx: &DatabaseTransaction,
    tenant_id: Uuid,
    request_id: Uuid,
    user_id: Uuid,
    token_hash: &str,
    now: chrono::DateTime<Utc>,
) -> Result<Option<auth_email_change_requests::Model>, AuthLifecycleError> {
    let query = || {
        AuthEmailChangeRequests::find_by_id(request_id)
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
            .filter(auth_email_change_requests::Column::UserId.eq(user_id))
            .filter(auth_email_change_requests::Column::TokenHash.eq(token_hash))
            .filter(auth_email_change_requests::Column::CreatedAt.lte(now))
            .filter(auth_email_change_requests::Column::ExpiresAt.gt(now))
    };

    match tx.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => query()
            .lock_exclusive()
            .one(tx)
            .await
            .map_err(AuthLifecycleError::from),
        DatabaseBackend::Sqlite => {
            let updated = AuthEmailChangeRequests::update_many()
                .col_expr(
                    auth_email_change_requests::Column::TokenHash,
                    Expr::col(auth_email_change_requests::Column::TokenHash),
                )
                .filter(auth_email_change_requests::Column::Id.eq(request_id))
                .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
                .filter(auth_email_change_requests::Column::UserId.eq(user_id))
                .filter(auth_email_change_requests::Column::TokenHash.eq(token_hash))
                .filter(auth_email_change_requests::Column::CreatedAt.lte(now))
                .filter(auth_email_change_requests::Column::ExpiresAt.gt(now))
                .exec(tx)
                .await
                .map_err(AuthLifecycleError::from)?;
            if updated.rows_affected != 1 {
                return Ok(None);
            }
            query().one(tx).await.map_err(AuthLifecycleError::from)
        }
        _ => Err(AuthLifecycleError::Internal(
            crate::error::Error::InternalServerError,
        )),
    }
}

async fn delete_email_change_request(
    tx: &DatabaseTransaction,
    request_id: Uuid,
    token_hash: &str,
) -> Result<(), AuthLifecycleError> {
    let deleted = AuthEmailChangeRequests::delete_many()
        .filter(auth_email_change_requests::Column::Id.eq(request_id))
        .filter(auth_email_change_requests::Column::TokenHash.eq(token_hash))
        .exec(tx)
        .await
        .map_err(AuthLifecycleError::from)?;
    if deleted.rows_affected != 1 {
        return Err(AuthLifecycleError::InvalidEmailChangeToken);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{sessions, tenants, users};
    use rustok_migrations::SqliteTestMigrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, EntityTrait, Set};

    async fn ensure_email_change_table(db: &DatabaseConnection) {
        use sea_orm::ConnectionTrait;

        db.execute_raw(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "CREATE TABLE IF NOT EXISTS auth_email_change_requests (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                current_email TEXT NOT NULL,
                new_email TEXT NOT NULL,
                password_changed_at TEXT NULL,
                recovery_revision BIGINT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create auth email change requests table");
        db.execute_raw(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_email_change_requests_tenant_user
                ON auth_email_change_requests (tenant_id, user_id)"
                .to_string(),
        ))
        .await
        .expect("create email change user index");
        db.execute_raw(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_email_change_requests_tenant_email
                ON auth_email_change_requests (tenant_id, new_email)"
                .to_string(),
        ))
        .await
        .expect("create email change address index");
    }

    async fn insert_pending_request(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        user: &users::Model,
        new_email: &str,
        token_hash: &str,
    ) {
        auth_email_change_requests::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            user_id: Set(user.id),
            current_email: Set(user.email.clone()),
            new_email: Set(new_email.to_string()),
            password_changed_at: Set(user.password_changed_at.clone()),
            recovery_revision: Set(Some(user.recovery_revision)),
            token_hash: Set(token_hash.to_string()),
            expires_at: Set((Utc::now() + Duration::minutes(30)).into()),
            created_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert pending email change");
    }

    #[tokio::test]
    async fn stale_foreign_reservation_is_reclaimed_but_live_and_other_tenant_are_preserved() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "mailbox-reclaim")
            .insert(&db)
            .await
            .expect("tenant");
        let other_tenant = tenants::ActiveModel::new("Other", "mailbox-reclaim-other")
            .insert(&db)
            .await
            .expect("other tenant");
        let owner = users::ActiveModel::new(tenant.id, "owner@example.com", "hash")
            .insert(&db)
            .await
            .expect("owner");
        let requester = users::ActiveModel::new(tenant.id, "requester@example.com", "hash")
            .insert(&db)
            .await
            .expect("requester");
        let other = users::ActiveModel::new(other_tenant.id, "other@example.com", "hash")
            .insert(&db)
            .await
            .expect("other tenant owner");
        let session = sessions::ActiveModel::new(
            tenant.id,
            requester.id,
            "reclaim-session".to_string(),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("session");
        let target = "target@example.com";
        let stale_hash = "a".repeat(64);
        let new_hash = "b".repeat(64);
        let other_hash = "c".repeat(64);
        let previous_hash = "d".repeat(64);
        insert_pending_request(&db, tenant.id, &owner, target, &stale_hash).await;
        insert_pending_request(&db, other_tenant.id, &other, target, &other_hash).await;
        insert_pending_request(
            &db,
            tenant.id,
            &requester,
            "previous@example.com",
            &previous_hash,
        )
        .await;

        // A live foreign reservation must win. Failed issuance also rolls
        // back deletion of the requester's existing challenge.
        assert!(matches!(
            AuthLifecycleService::create_email_change_request_db(
                &db,
                tenant.id,
                requester.id,
                session.id,
                &requester,
                target,
                &new_hash,
            )
            .await,
            Err(AuthLifecycleError::EmailAlreadyExists)
        ));
        for hash in [&stale_hash, &other_hash, &previous_hash] {
            assert!(
                AuthEmailChangeRequests::find()
                    .filter(auth_email_change_requests::Column::TokenHash.eq(hash.as_str()))
                    .one(&db)
                    .await
                    .unwrap()
                    .is_some()
            );
        }

        // ABA restores the email but not the recovery revision. Reclaim the
        // still-unexpired challenge before issuing the replacement.
        for email in ["temporary@example.com", "owner@example.com"] {
            users::ActiveModel {
                id: Set(owner.id),
                email: Set(email.to_string()),
                ..Default::default()
            }
            .update(&db)
            .await
            .expect("change owner mailbox");
        }
        AuthLifecycleService::create_email_change_request_db(
            &db,
            tenant.id,
            requester.id,
            session.id,
            &requester,
            target,
            &new_hash,
        )
        .await
        .expect("reclaim invalid reservation");
        for hash in [&stale_hash, &previous_hash] {
            assert!(
                AuthEmailChangeRequests::find()
                    .filter(auth_email_change_requests::Column::TokenHash.eq(hash.as_str()))
                    .one(&db)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        let replacement = AuthEmailChangeRequests::find()
            .filter(auth_email_change_requests::Column::TokenHash.eq(&new_hash))
            .one(&db)
            .await
            .unwrap()
            .expect("replacement");
        assert_eq!(replacement.user_id, requester.id);
        assert_eq!(replacement.new_email, target);
        assert!(
            AuthEmailChangeRequests::find()
                .filter(auth_email_change_requests::Column::TokenHash.eq(&other_hash))
                .one(&db)
                .await
                .unwrap()
                .is_some()
        );
        assert!(matches!(
            AuthLifecycleService::confirm_email_change_db(
                &ServerRuntimeContext::new(db.clone(), Default::default()),
                tenant.id,
                &stale_hash,
                None,
                None,
            )
            .await,
            Err(AuthLifecycleError::InvalidEmailChangeToken)
        ));
    }

    #[tokio::test]
    async fn request_replaces_pending_challenge_with_full_lifetime() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-change-issuance")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "password-hash")
            .insert(&db)
            .await
            .expect("user");
        let session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "issuance-session-hash".to_string(),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("session");
        let old_hash = "a".repeat(64);
        let new_hash = "b".repeat(64);
        insert_pending_request(&db, tenant.id, &user, "previous@example.com", &old_hash).await;

        let before = Utc::now();
        let dispatch_deadline = AuthLifecycleService::create_email_change_request_db(
            &db,
            tenant.id,
            user.id,
            session.id,
            &user,
            "new@example.com",
            &new_hash,
        )
        .await
        .expect("issue replacement challenge");
        let after = Utc::now();

        let requests = AuthEmailChangeRequests::find()
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant.id))
            .filter(auth_email_change_requests::Column::UserId.eq(user.id))
            .all(&db)
            .await
            .expect("pending requests");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.token_hash, new_hash);
        assert!(dispatch_deadline <= request.expires_at.with_timezone(&Utc));
        assert!(
            pending_email_change_window(&db, tenant.id, user.id, "previous@example.com", &old_hash)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            pending_email_change_window(&db, tenant.id, user.id, "new@example.com", &new_hash)
                .await
                .unwrap(),
            Some((
                request.created_at.with_timezone(&Utc),
                request.expires_at.with_timezone(&Utc)
            ))
        );
        assert!(
            pending_email_change_window(&db, tenant.id, user.id, "wrong@example.com", &new_hash)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            pending_email_change_window(&db, Uuid::new_v4(), user.id, "new@example.com", &new_hash)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(request.new_email, "new@example.com");
        assert_eq!(request.recovery_revision, Some(user.recovery_revision));
        assert!(request.created_at.with_timezone(&Utc) >= before - Duration::milliseconds(1));
        assert!(request.created_at.with_timezone(&Utc) <= after);
        assert_eq!(
            request.expires_at.signed_duration_since(request.created_at),
            Duration::seconds(EMAIL_CHANGE_TOKEN_TTL_SECS as i64),
        );
        // A mailbox ABA invalidates the retained pending row.
        for email in ["temporary@example.com", "old@example.com"] {
            users::ActiveModel {
                id: Set(user.id),
                email: Set(email.to_string()),
                ..Default::default()
            }
            .update(&db)
            .await
            .unwrap();
        }
        assert!(
            pending_email_change_window(&db, tenant.id, user.id, "new@example.com", &new_hash)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn future_dated_email_change_cannot_mutate_account_or_consume_challenge() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-change-future")
            .insert(&db)
            .await
            .unwrap();
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "hash")
            .insert(&db)
            .await
            .unwrap();
        let token_hash = "a".repeat(64);
        insert_pending_request(&db, tenant.id, &user, "new@example.com", &token_hash).await;
        let challenge = AuthEmailChangeRequests::find()
            .filter(auth_email_change_requests::Column::TokenHash.eq(&token_hash))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        auth_email_change_requests::ActiveModel {
            id: Set(challenge.id),
            created_at: Set((Utc::now() + Duration::minutes(5)).into()),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        let ctx = ServerRuntimeContext::new(db.clone(), Default::default());
        assert!(matches!(AuthLifecycleService::confirm_email_change_db(
            &ctx, tenant.id, &token_hash, None, None,
        ).await, Err(AuthLifecycleError::InvalidEmailChangeToken)));
        let stored = Users::find_by_id(user.id).one(&db).await.unwrap().unwrap();
        assert_eq!(stored.email, user.email);
        assert_eq!(stored.recovery_revision, user.recovery_revision);
        assert!(
            AuthEmailChangeRequests::find_by_id(challenge.id)
                .one(&db)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn confirmation_changes_and_verifies_mailbox_and_consumes_token() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-change")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "password-hash")
            .insert(&db)
            .await
            .expect("user");
        let token_hash = "a".repeat(64);
        let _first_session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "first-session-hash".to_string(),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("first session");
        let _second_session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "second-session-hash".to_string(),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("second session");
        insert_pending_request(&db, tenant.id, &user, "new@example.com", &token_hash).await;

        let changed = AuthLifecycleService::confirm_email_change_db(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            tenant.id,
            &token_hash,
            None,
            None,
        )
        .await
        .expect("confirm email change");

        assert_eq!(changed.old_email, "old@example.com");
        assert_eq!(changed.new_email, "new@example.com");
        let updated = Users::find_by_id(user.id)
            .one(&db)
            .await
            .expect("query user")
            .expect("updated user");
        assert_eq!(updated.email, "new@example.com");
        assert!(updated.email_verified_at.is_some());

        let remaining_sessions = sessions::Entity::find()
            .filter(sessions::Column::TenantId.eq(tenant.id))
            .filter(sessions::Column::UserId.eq(user.id))
            .all(&db)
            .await
            .expect("query revoked sessions");
        assert_eq!(remaining_sessions.len(), 2);
        assert!(
            remaining_sessions
                .iter()
                .all(|session| session.revoked_at.is_some())
        );

        let remaining = AuthEmailChangeRequests::find()
            .filter(auth_email_change_requests::Column::TokenHash.eq(&token_hash))
            .one(&db)
            .await
            .expect("query consumed token");
        assert!(remaining.is_none());

        assert!(matches!(
            AuthLifecycleService::confirm_email_change_db(
                &ServerRuntimeContext::new(db.clone(), Default::default()),
                tenant.id,
                &token_hash,
                None,
                None,
            )
            .await,
            Err(AuthLifecycleError::InvalidEmailChangeToken)
        ));
    }

    #[tokio::test]
    async fn confirmation_requires_authenticated_identity_and_session_together() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-change-context")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "password-hash")
            .insert(&db)
            .await
            .expect("user");
        let token_hash = "d".repeat(64);
        insert_pending_request(&db, tenant.id, &user, "new@example.com", &token_hash).await;

        for (authenticated_user_id, current_session_id) in [
            (Some(user.id), None),
            (None, Some(Uuid::new_v4())),
            (Some(Uuid::new_v4()), Some(Uuid::new_v4())),
        ] {
            assert!(matches!(
                AuthLifecycleService::confirm_email_change_db(
                    &ServerRuntimeContext::new(db.clone(), Default::default()),
                    tenant.id,
                    &token_hash,
                    authenticated_user_id,
                    current_session_id,
                )
                .await,
                Err(AuthLifecycleError::InvalidEmailChangeToken)
            ));
        }

        assert!(
            AuthEmailChangeRequests::find()
                .filter(auth_email_change_requests::Column::TokenHash.eq(&token_hash))
                .one(&db)
                .await
                .expect("query pending request")
                .is_some(),
            "invalid caller context must not consume the email-change challenge"
        );
    }

    #[tokio::test]
    async fn authenticated_confirmation_preserves_only_the_proving_session() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-change-session")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "password-hash")
            .insert(&db)
            .await
            .expect("user");
        let current = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "current-session-hash".to_string(),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("current session");
        let other = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "other-session-hash".to_string(),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("other session");
        let token_hash = "e".repeat(64);
        insert_pending_request(&db, tenant.id, &user, "new@example.com", &token_hash).await;

        AuthLifecycleService::confirm_email_change_db(
            &ServerRuntimeContext::new(db.clone(), Default::default()),
            tenant.id,
            &token_hash,
            Some(user.id),
            Some(current.id),
        )
        .await
        .expect("confirm email change");

        let current = sessions::Entity::find_by_id(current.id)
            .one(&db)
            .await
            .expect("query current session")
            .expect("current session exists");
        let other = sessions::Entity::find_by_id(other.id)
            .one(&db)
            .await
            .expect("query other session")
            .expect("other session exists");
        assert!(current.revoked_at.is_none());
        assert!(other.revoked_at.is_some());
    }

    #[tokio::test]
    async fn password_change_invalidates_pending_email_change() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-change-password")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "password-hash")
            .insert(&db)
            .await
            .expect("user");
        let token_hash = "b".repeat(64);
        insert_pending_request(&db, tenant.id, &user, "new@example.com", &token_hash).await;

        users::ActiveModel {
            id: Set(user.id),
            password_changed_at: Set(Some(Utc::now().into())),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("change credential timestamp");

        assert!(matches!(
            AuthLifecycleService::confirm_email_change_db(
                &ServerRuntimeContext::new(db.clone(), Default::default()),
                tenant.id,
                &token_hash,
                None,
                None,
            )
            .await,
            Err(AuthLifecycleError::InvalidEmailChangeToken)
        ));
        let unchanged = Users::find_by_id(user.id)
            .one(&db)
            .await
            .expect("query user")
            .expect("user");
        assert_eq!(unchanged.email, "old@example.com");
        assert!(unchanged.email_verified_at.is_none());
    }

    #[tokio::test]
    async fn mailbox_aba_invalidates_pending_email_change_and_stale_issuance() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-change-revision")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "password-hash")
            .insert(&db)
            .await
            .expect("user");
        let token_hash = "f".repeat(64);
        insert_pending_request(&db, tenant.id, &user, "new@example.com", &token_hash).await;
        for email in ["temporary@example.com", "old@example.com"] {
            users::ActiveModel {
                id: Set(user.id),
                email: Set(email.to_string()),
                ..Default::default()
            }
            .update(&db)
            .await
            .expect("change mailbox");
        }
        let current = Users::find_by_id(user.id).one(&db).await.unwrap().unwrap();
        assert!(current.recovery_revision > user.recovery_revision);
        assert_eq!(current.email, user.email);
        assert_eq!(current.password_changed_at, user.password_changed_at);
        let session = sessions::ActiveModel::new(
            tenant.id,
            user.id,
            "revision-session".to_string(),
            Utc::now() + Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .expect("session");
        assert!(matches!(
            AuthLifecycleService::create_email_change_request_db(
                &db,
                tenant.id,
                user.id,
                session.id,
                &user,
                "new@example.com",
                &"e".repeat(64),
            )
            .await,
            Err(AuthLifecycleError::InvalidCredentials)
        ));
        assert!(matches!(
            AuthLifecycleService::confirm_email_change_db(
                &ServerRuntimeContext::new(db.clone(), Default::default()),
                tenant.id,
                &token_hash,
                None,
                None,
            )
            .await,
            Err(AuthLifecycleError::InvalidEmailChangeToken)
        ));
        assert!(
            AuthEmailChangeRequests::find()
                .filter(auth_email_change_requests::Column::TokenHash.eq(&token_hash))
                .one(&db)
                .await
                .unwrap()
                .is_none()
        );
        let unchanged = Users::find_by_id(user.id).one(&db).await.unwrap().unwrap();
        assert_eq!(unchanged.email, user.email);
    }

    #[tokio::test]
    async fn target_address_already_claimed_consumes_pending_request() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        ensure_email_change_table(&db).await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-change-conflict")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "password-hash")
            .insert(&db)
            .await
            .expect("user");
        users::ActiveModel::new(tenant.id, "taken@example.com", "password-hash")
            .insert(&db)
            .await
            .expect("other user");
        let token_hash = "c".repeat(64);
        insert_pending_request(&db, tenant.id, &user, "taken@example.com", &token_hash).await;

        assert!(matches!(
            AuthLifecycleService::confirm_email_change_db(
                &ServerRuntimeContext::new(db.clone(), Default::default()),
                tenant.id,
                &token_hash,
                None,
                None,
            )
            .await,
            Err(AuthLifecycleError::EmailAlreadyExists)
        ));
        let pending = AuthEmailChangeRequests::find()
            .filter(auth_email_change_requests::Column::TokenHash.eq(&token_hash))
            .one(&db)
            .await
            .expect("query request");
        assert!(pending.is_none());
        let unchanged = Users::find_by_id(user.id).one(&db).await.unwrap().unwrap();
        assert_eq!(unchanged.email, user.email);
        assert_eq!(unchanged.email_verified_at, user.email_verified_at);
        assert_eq!(unchanged.recovery_revision, user.recovery_revision);
        assert!(matches!(
            AuthLifecycleService::confirm_email_change_db(
                &ServerRuntimeContext::new(db.clone(), Default::default()),
                tenant.id,
                &token_hash,
                None,
                None,
            )
            .await,
            Err(AuthLifecycleError::InvalidEmailChangeToken)
        ));
    }
}
