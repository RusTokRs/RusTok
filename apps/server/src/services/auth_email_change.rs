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
        let new_email = normalize_email(requested_email)
            .map_err(AuthLifecycleError::from)?;

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

        Self::create_email_change_request_db(
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
        );
        Ok(())
    }

    /// Confirm the exact challenge while holding the user mutation fence.
    /// The old address and password-change timestamp bind the challenge to
    /// the identity state observed when it was requested.
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
            ctx.db(),
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
    ) -> Result<(), AuthLifecycleError> {
        let now = Utc::now();
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
        {
            return Err(AuthLifecycleError::InvalidCredentials);
        }

        Self::find_current_session_for_password_change_in_tx(
            &tx,
            tenant_id,
            user_id,
            session_id,
        )
        .await?
        .filter(|session| session.is_active())
        .ok_or(AuthLifecycleError::SessionExpired)?;

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

        // Expired requests must not reserve an address indefinitely. The
        // unique tenant/email index serializes competing pending claims.
        AuthEmailChangeRequests::delete_many()
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
            .filter(auth_email_change_requests::Column::ExpiresAt.lte(now.clone()))
            .exec(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;

        AuthEmailChangeRequests::delete_many()
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
            .filter(auth_email_change_requests::Column::UserId.eq(user_id))
            .exec(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;

        let request = auth_email_change_requests::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            user_id: Set(user_id),
            current_email: Set(locked_user.email),
            new_email: Set(new_email.to_string()),
            password_changed_at: Set(locked_user.password_changed_at),
            token_hash: Set(token_hash.to_string()),
            expires_at: Set(
                (now.clone() + Duration::seconds(EMAIL_CHANGE_TOKEN_TTL_SECS as i64)).into(),
            ),
            created_at: Set(now.into()),
        };

        request.insert(&tx).await.map_err(|error| {
            if matches!(
                error.sql_err(),
                Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
            ) {
                AuthLifecycleError::EmailAlreadyExists
            } else {
                AuthLifecycleError::from(error)
            }
        })?;

        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(())
    }

    async fn confirm_email_change_db(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        token_hash: &str,
        authenticated_user_id: Option<Uuid>,
        current_session_id: Option<Uuid>,
    ) -> Result<ConfirmedEmailChange, AuthLifecycleError> {
        let now = Utc::now();

        // Read only the owner ID before opening the transaction. Every
        // mutation then locks user first and challenge second, matching the
        // request path's lock order and avoiding a user/challenge deadlock.
        let observed_request = AuthEmailChangeRequests::find()
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
            .filter(auth_email_change_requests::Column::TokenHash.eq(token_hash))
            .filter(auth_email_change_requests::Column::ExpiresAt.gt(now.clone()))
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
        let user = Self::lock_user_for_mutation(
            &tx,
            tenant_id,
            observed_request.user_id,
        )
        .await
        .map_err(AuthLifecycleError::from)?
        .ok_or(AuthLifecycleError::InvalidEmailChangeToken)?;

        let request = lock_email_change_request(
            &tx,
            tenant_id,
            observed_request.id,
            observed_request.user_id,
            token_hash,
            now.clone(),
        )
        .await?
        .ok_or(AuthLifecycleError::InvalidEmailChangeToken)?;

        if !user.is_active()
            || user.email != request.current_email
            || user.password_changed_at != request.password_changed_at
        {
            delete_email_change_request(&tx, request.id, token_hash).await?;
            tx.commit().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::InvalidEmailChangeToken);
        }

        if let Some(session_id) = current_session_id {
            Self::find_current_session_for_password_change_in_tx(
                &tx,
                tenant_id,
                user.id,
                session_id,
            )
            .await?
            .filter(|session| session.is_active())
            .ok_or(AuthLifecycleError::SessionExpired)?;
        }

        let existing_user = Users::find()
            .filter(users::Column::TenantId.eq(tenant_id))
            .filter(users::Column::Email.eq(&request.new_email))
            .one(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;
        if existing_user.is_some_and(|existing| existing.id != user.id) {
            delete_email_change_request(&tx, request.id, token_hash).await?;
            tx.commit().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::EmailAlreadyExists);
        }

        let old_email = user.email.clone();
        let new_email = request.new_email.clone();
        let mut active: users::ActiveModel = user.clone().into();
        active.email = Set(new_email.clone());
        let updated_at: chrono::DateTime<chrono::FixedOffset> = now.clone().into();
        active.email_verified_at = Set(Some(updated_at.clone()));
        active.updated_at = Set(updated_at);
        active.update(&tx).await.map_err(|error| {
            if matches!(
                error.sql_err(),
                Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
            ) {
                AuthLifecycleError::EmailAlreadyExists
            } else {
                AuthLifecycleError::from(error)
            }
        })?;

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
                &tx, tenant_id, user.id, now,
            )
            .await
            .map_err(AuthLifecycleError::from)?;

        delete_email_change_request(&tx, request.id, token_hash).await?;
        Self::publish_user_event_in_tx(
            db.clone(),
            &tx,
            tenant_id,
            user.id,
            rustok_events::DomainEvent::UserUpdated { user_id: user.id },
        )
        .await?;

        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(ConfirmedEmailChange {
            old_email,
            new_email,
        })
    }
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
            .filter(auth_email_change_requests::Column::ExpiresAt.gt(now.clone()))
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
                .filter(auth_email_change_requests::Column::ExpiresAt.gt(now.clone()))
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

        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "CREATE TABLE IF NOT EXISTS auth_email_change_requests (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                current_email TEXT NOT NULL,
                new_email TEXT NOT NULL,
                password_changed_at TEXT NULL,
                token_hash TEXT NOT NULL UNIQUE,
                expires_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            )"
            .to_string(),
        ))
        .await
        .expect("create auth email change requests table");
        db.execute(sea_orm::Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_auth_email_change_requests_tenant_user
                ON auth_email_change_requests (tenant_id, user_id)"
                .to_string(),
        ))
        .await
        .expect("create email change user index");
        db.execute(sea_orm::Statement::from_string(
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
            token_hash: Set(token_hash.to_string()),
            expires_at: Set((Utc::now() + Duration::minutes(30)).into()),
            created_at: Set(Utc::now().into()),
        }
        .insert(db)
        .await
        .expect("insert pending email change");
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
        let first_session = sessions::ActiveModel::new(
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
        insert_pending_request(
            &db,
            tenant.id,
            &user,
            "new@example.com",
            &token_hash,
        )
        .await;

        let changed = AuthLifecycleService::confirm_email_change_db(
            &db,
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
        assert!(remaining_sessions.iter().all(|session| session.revoked_at.is_some()));

        let remaining = AuthEmailChangeRequests::find()
            .filter(auth_email_change_requests::Column::TokenHash.eq(&token_hash))
            .one(&db)
            .await
            .expect("query consumed token");
        assert!(remaining.is_none());

        assert!(matches!(
            AuthLifecycleService::confirm_email_change_db(
                &db,
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
        insert_pending_request(
            &db,
            tenant.id,
            &user,
            "new@example.com",
            &token_hash,
        )
        .await;

        for (authenticated_user_id, current_session_id) in [
            (Some(user.id), None),
            (None, Some(Uuid::new_v4())),
            (Some(Uuid::new_v4()), Some(Uuid::new_v4())),
        ] {
            assert!(matches!(
                AuthLifecycleService::confirm_email_change_db(
                    &db,
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
        insert_pending_request(
            &db,
            tenant.id,
            &user,
            "new@example.com",
            &token_hash,
        )
        .await;

        AuthLifecycleService::confirm_email_change_db(
            &db,
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
        insert_pending_request(
            &db,
            tenant.id,
            &user,
            "new@example.com",
            &token_hash,
        )
        .await;

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
                &db,
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
        insert_pending_request(
            &db,
            tenant.id,
            &user,
            "taken@example.com",
            &token_hash,
        )
        .await;

        assert!(matches!(
            AuthLifecycleService::confirm_email_change_db(
                &db,
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
    }
}
