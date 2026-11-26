use chrono::Utc;
use rustok_events::DomainEvent;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set,
    TransactionTrait, sea_query::Expr,
};

use crate::auth::{
    AuthConfig, decode_password_reset_token, encode_password_reset_token, hash_password_async,
    mask_auth_lookup_timing, password_reset_credential_matches, validate_password_policy,
};
use crate::models::{auth_email_change_requests, sessions, users};
use crate::services::email::{
    PASSWORD_RESET_TOKEN_TTL_SECS, email_service_from_ctx, password_reset_url,
    spawn_password_reset_email,
};

use super::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use super::server_runtime_context::ServerRuntimeContext;

impl AuthLifecycleService {
    /// Shared REST/GraphQL/native recovery request. The public response must
    /// not reveal account existence through account-dependent preparation
    /// errors. Storage and timing-mask failures remain real service errors.
    pub async fn request_password_reset_runtime(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        email: &str,
        locale: &str,
    ) -> Result<(), AuthLifecycleError> {
        let user = users::Entity::find_by_email(ctx.db(), tenant_id, email.trim())
            .await
            .map_err(AuthLifecycleError::from)?;
        mask_auth_lookup_timing().await.map_err(AuthLifecycleError::from)?;

        let Some(user) = user.filter(users::Model::is_active) else {
            return Ok(());
        };
        let prepared = (|| {
            let token = encode_password_reset_token(
                config,
                tenant_id,
                &user.email,
                &user.password_hash,
                PASSWORD_RESET_TOKEN_TTL_SECS,
            )?;
            let service = email_service_from_ctx(ctx, locale)?;
            let url = password_reset_url(ctx, &token)?;
            Ok::<_, crate::error::Error>((service, url))
        })();
        match prepared {
            Ok((service, url)) => {
                let _task = spawn_password_reset_email(ctx, service, user.email, url);
            }
            Err(_) => {
                tracing::warn!(
                    "Password reset email could not be prepared; keeping the public response generic"
                );
            }
        }
        Ok(())
    }

    /// Confirm a password reset token that is bound to the credential state
    /// present at issuance time.
    ///
    /// The password update uses a compare-and-swap predicate on the previous
    /// password hash inside the same transaction that revokes all sessions.
    /// Consequently, only one concurrent confirmation can consume the token.
    pub async fn confirm_bound_password_reset_runtime(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        token: &str,
        new_password: &str,
    ) -> Result<(), AuthLifecycleError> {
        let claims = decode_password_reset_token(config, token)
            .map_err(|_| AuthLifecycleError::InvalidResetToken)?;
        // INVARIANT: Validate that the reset token was signed for the requested tenant before looking up the user.
        if claims.tenant_id != tenant_id {
            return Err(AuthLifecycleError::InvalidResetToken);
        }

        let user = users::Entity::find_by_email(ctx.db(), tenant_id, &claims.sub)
            .await
            .map_err(AuthLifecycleError::from)?
            .ok_or(AuthLifecycleError::InvalidResetToken)?;
        if !user.is_active() {
            return Err(AuthLifecycleError::UserInactive);
        }
        if !password_reset_credential_matches(
            config,
            &user.password_hash,
            &claims.credential_fingerprint,
        ) {
            return Err(AuthLifecycleError::InvalidResetToken);
        }

        validate_password_policy(new_password, config.min_password_length)
            .map_err(AuthLifecycleError::from)?;

        let new_hash = hash_password_async(new_password.to_string())
            .await
            .map_err(AuthLifecycleError::from)?;
        let tx = ctx.db().begin().await.map_err(AuthLifecycleError::from)?;
        let (current, now) = AuthLifecycleService::lock_user_for_recovery(
            &tx, tenant_id, user.id, claims.expires_at,
        )
        .await
        .map_err(AuthLifecycleError::from)?
        .ok_or(AuthLifecycleError::InvalidResetToken)?;

        // Hashing runs outside the transaction. Recheck the account, mailbox,
        // status and original credential after the user lock and expiry check.
        if !current.is_active()
            || current.email != user.email
            || current.password_hash != user.password_hash
            || !update_password_if_active(&tx, &current, &new_hash, now).await?
        {
            tx.rollback().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::InvalidResetToken);
        }

        let revoked_sessions = sessions::Entity::update_many()
            .col_expr(sessions::Column::RevokedAt, Expr::value(now))
            .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .exec(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;

        let _revoked_oauth_tokens =
            crate::services::oauth_token_service::revoke_user_delegated_tokens_in_tx(
                &tx, tenant_id, user.id, now,
            )
            .await
            .map_err(AuthLifecycleError::from)?;
        auth_email_change_requests::Entity::delete_many()
            .filter(auth_email_change_requests::Column::TenantId.eq(tenant_id))
            .filter(auth_email_change_requests::Column::UserId.eq(user.id))
            .exec(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;

        // A successful reset proves mailbox control: clear any brute-force
        // lockout so the legitimate owner is not kept out afterwards.
        users::ActiveModel {
            id: Set(user.id),
            failed_login_count: Set(0),
            locked_until: Set(None),
            ..Default::default()
        }
        .update(&tx)
        .await
        .map_err(AuthLifecycleError::from)?;

        Self::publish_user_event_in_tx(
            ctx,
            &tx,
            tenant_id,
            user.id,
            DomainEvent::UserPasswordResetCompleted { user_id: user.id },
        )
        .await?;

        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Self::record_password_reset_sessions_revoked(revoked_sessions.rows_affected);
        Ok(())
    }
}

/// Serialize reset with account deactivation, mailbox changes and credential
/// changes. The exact stored email is the mailbox observed before hashing.
async fn update_password_if_active<C: ConnectionTrait>(
    db: &C,
    observed_user: &users::Model,
    new_hash: &str,
    now: chrono::DateTime<Utc>,
) -> Result<bool, AuthLifecycleError> {
    let updated = users::Entity::update_many()
        .col_expr(
            users::Column::PasswordHash,
            Expr::value(new_hash.to_owned()),
        )
        .col_expr(users::Column::PasswordChangedAt, Expr::value(now))
        .col_expr(users::Column::UpdatedAt, Expr::value(now))
        .filter(users::Column::Id.eq(observed_user.id))
        .filter(users::Column::TenantId.eq(observed_user.tenant_id))
        .filter(users::Column::Status.eq(rustok_core::UserStatus::Active))
        .filter(users::Column::Email.eq(&observed_user.email))
        .filter(users::Column::PasswordHash.eq(&observed_user.password_hash))
        .exec(db)
        .await
        .map_err(AuthLifecycleError::from)?;
    Ok(updated.rows_affected == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait};
    use serial_test::serial;

    use crate::auth::{encode_password_reset_token, hash_password, verify_password};
    use crate::common::RustokSettings;
    use crate::models::{sessions, tenants, users};
    use rustok_migrations::SqliteTestMigrator as Migrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;

    const OLD_PASSWORD: &str = "OldPassword123!";
    const NEW_PASSWORD: &str = "NewPassword123!";

    async fn setup_user_with_sessions(
        slug: &str,
        email: &str,
        session_tokens: &[&str],
    ) -> (
        sea_orm::DatabaseConnection,
        ServerRuntimeContext,
        AuthConfig,
        tenants::Model,
        users::Model,
    ) {
        let db = setup_test_db_with_migrations::<Migrator>().await;
        let ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let config = crate::auth::test_auth_config("reset-service-secret-key-for-tests-32b".to_string());
        let tenant = tenants::ActiveModel::new("Reset Tenant", slug)
            .insert(&db)
            .await
            .expect("failed to create tenant");

        let old_hash = hash_password(OLD_PASSWORD).expect("failed to hash old password");
        let mut user_model = users::ActiveModel::new(tenant.id, email, &old_hash);
        user_model.failed_login_count = Set(4);
        user_model.locked_until = Set(Some((Utc::now() + Duration::minutes(10)).into()));
        let user = user_model.insert(&db).await.expect("failed to create user");

        for token in session_tokens {
            sessions::ActiveModel::new(
                tenant.id,
                user.id,
                token.to_string(),
                Utc::now() + Duration::hours(1),
                None,
                None,
            )
            .insert(&db)
            .await
            .expect("failed to create session");
        }

        (db, ctx, config, tenant, user)
    }

    #[tokio::test]
    async fn reset_cas_rejects_deactivation_after_credential_read() {
        let (db, _ctx, _config, _tenant, user) = setup_user_with_sessions(
            "reset-tenant-deactivated",
            "deactivated@example.com",
            &["deactivated-session"],
        )
        .await;

        users::ActiveModel {
            id: Set(user.id),
            status: Set(rustok_core::UserStatus::Inactive),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("deactivate user");

        let updated = update_password_if_active(
            &db,
            &user,
            "unexpected-new-hash",
            Utc::now(),
        )
        .await
        .expect("compare-and-swap");
        assert!(!updated);

        let stored = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("reload user")
            .expect("user must exist");
        assert_eq!(stored.password_hash, user.password_hash);
        assert!(stored.password_changed_at.is_none());
    }

    #[tokio::test]
    async fn reset_cas_rejects_mailbox_change_after_credential_read() {
        let (db, _ctx, _config, _tenant, user) = setup_user_with_sessions(
            "reset-mailbox-change",
            "old@example.com",
            &["mailbox-session"],
        )
        .await;
        users::ActiveModel {
            id: Set(user.id),
            email: Set("new@example.com".to_string()),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("change mailbox");

        assert!(
            !update_password_if_active(&db, &user, "unexpected-new-hash", Utc::now())
                .await
                .expect("compare and swap")
        );
        let current = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("reload user")
            .expect("user exists");
        assert_eq!(current.email, "new@example.com");
        assert_eq!(current.password_hash, user.password_hash);
        assert!(current.password_changed_at.is_none());
    }

    #[tokio::test]
    #[serial]
    async fn confirm_reset_updates_password_revokes_sessions_and_clears_lockout() {
        let (db, ctx, config, tenant, user) = setup_user_with_sessions(
            "reset-tenant-happy",
            "reset@example.com",
            &["reset-session-1", "reset-session-2"],
        )
        .await;

        let token =
            encode_password_reset_token(&config, tenant.id, &user.email, &user.password_hash, 900)
                .expect("failed to encode reset token");
        let metrics_before = AuthLifecycleService::metrics_snapshot();

        AuthLifecycleService::confirm_bound_password_reset_runtime(
            &ctx,
            &config,
            tenant.id,
            &token,
            NEW_PASSWORD,
        )
        .await
        .expect("confirm reset should succeed");

        let updated = users::Entity::find_by_id(user.id)
            .one(&db)
            .await
            .expect("failed to query user")
            .expect("user should exist");
        assert!(
            verify_password(NEW_PASSWORD, &updated.password_hash)
                .expect("new password should verify")
        );
        assert!(
            !verify_password(OLD_PASSWORD, &updated.password_hash)
                .expect("old password must not verify")
        );
        assert_eq!(updated.failed_login_count, 0);
        assert!(updated.locked_until.is_none());

        let active_sessions = sessions::Entity::find()
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .count(&db)
            .await
            .expect("failed to count sessions");
        assert_eq!(active_sessions, 0);

        let metrics_after = AuthLifecycleService::metrics_snapshot();
        assert!(
            metrics_after.password_reset_sessions_revoked_total
                >= metrics_before.password_reset_sessions_revoked_total + 2
        );
    }

    #[tokio::test]
    async fn confirm_reset_token_cannot_be_reused_after_password_change() {
        let (_db, ctx, config, tenant, user) = setup_user_with_sessions(
            "reset-tenant-reuse",
            "reuse@example.com",
            &["reuse-session"],
        )
        .await;

        let token =
            encode_password_reset_token(&config, tenant.id, &user.email, &user.password_hash, 900)
                .expect("failed to encode reset token");

        AuthLifecycleService::confirm_bound_password_reset_runtime(
            &ctx,
            &config,
            tenant.id,
            &token,
            NEW_PASSWORD,
        )
        .await
        .expect("first confirm should succeed");

        let second = AuthLifecycleService::confirm_bound_password_reset_runtime(
            &ctx,
            &config,
            tenant.id,
            &token,
            "AnotherPassword123!",
        )
        .await;
        assert!(matches!(second, Err(AuthLifecycleError::InvalidResetToken)));
    }

    #[tokio::test]
    async fn confirm_reset_rejects_token_issued_for_another_tenant() {
        let (db, ctx, config, tenant, user) = setup_user_with_sessions(
            "reset-tenant-owner",
            "owner@example.com",
            &["owner-session"],
        )
        .await;
        let other_tenant = tenants::ActiveModel::new("Other Tenant", "reset-tenant-other")
            .insert(&db)
            .await
            .expect("failed to create other tenant");

        let token =
            encode_password_reset_token(&config, tenant.id, &user.email, &user.password_hash, 900)
                .expect("failed to encode reset token");

        let result = AuthLifecycleService::confirm_bound_password_reset_runtime(
            &ctx,
            &config,
            other_tenant.id,
            &token,
            NEW_PASSWORD,
        )
        .await;
        assert!(matches!(result, Err(AuthLifecycleError::InvalidResetToken)));
    }

    #[tokio::test]
    async fn confirm_reset_rejects_weak_password_without_burning_the_token() {
        let (_db, ctx, config, tenant, user) =
            setup_user_with_sessions("reset-tenant-weak", "weak@example.com", &["weak-session"])
                .await;

        let token =
            encode_password_reset_token(&config, tenant.id, &user.email, &user.password_hash, 900)
                .expect("failed to encode reset token");

        let weak = AuthLifecycleService::confirm_bound_password_reset_runtime(
            &ctx, &config, tenant.id, &token, "short",
        )
        .await;
        assert!(matches!(weak, Err(AuthLifecycleError::Validation(_))));

        AuthLifecycleService::confirm_bound_password_reset_runtime(
            &ctx,
            &config,
            tenant.id,
            &token,
            NEW_PASSWORD,
        )
        .await
        .expect("retry with a compliant password should succeed");
    }
}
