use chrono::Utc;
use rustok_events::DomainEvent;
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, TransactionTrait, sea_query::Expr,
};

use crate::auth::{AuthConfig, decode_email_verification_token};
use crate::models::users::{self, Entity as Users};

use super::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use super::server_runtime_context::ServerRuntimeContext;

impl AuthLifecycleService {
    /// Confirm an email-verification token and mark the address verified.
    ///
    /// The token is purpose-bound (separate JWT claim set) and bound to
    /// the user, email and durable recovery revision it was issued for.
    /// Confirmation is idempotent within that revision:
    /// replaying a token for an already-verified address succeeds
    /// without rewriting the timestamp or re-emitting the audit event.
    /// The first successful confirmation performs a compare-and-swap
    /// update (`email_verified_at IS NULL`) inside the same transaction
    /// that publishes `UserEmailVerified`, so concurrent confirmations
    /// cannot double-emit.
    pub async fn confirm_email_verification_runtime(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        token: &str,
    ) -> Result<(), AuthLifecycleError> {
        rustok_auth::application_clock::admit_now(ctx.db()).await?;
        let claims = decode_email_verification_token(config, token)
            .map_err(|_| AuthLifecycleError::InvalidVerificationToken)?;
        // INVARIANT: In-memory verification ensuring the decoded email verification token belongs to the target tenant.
        if claims.tenant_id != tenant_id {
            return Err(AuthLifecycleError::InvalidVerificationToken);
        }

        let user = Users::find_by_id(claims.user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(ctx.db())
            .await
            .map_err(AuthLifecycleError::from)?
            .filter(|record| record.is_active() && record.matches_recovery_identity(
                tenant_id, claims.user_id, &claims.sub, claims.recovery_revision,
            ))
            .ok_or(AuthLifecycleError::InvalidVerificationToken)?;

        // Do not short-circuit an already-verified preflight read. An email
        // change may commit between this read and the transaction; the CAS
        // and in-transaction check below must decide whether this token is
        // still valid for the current address.
        let tx = ctx.db().begin().await.map_err(AuthLifecycleError::from)?;
        let (current, verified_at) = Self::lock_user_for_recovery(
            &tx, tenant_id, user.id, claims.expires_at,
        )
        .await
        .map_err(AuthLifecycleError::from)?
        .ok_or(AuthLifecycleError::InvalidVerificationToken)?;
        if decode_email_verification_token(config, token).is_err()
            || !current.is_active() || !current.matches_recovery_identity(
            tenant_id, claims.user_id, &claims.sub, claims.recovery_revision,
        ) {
            return Err(AuthLifecycleError::InvalidVerificationToken);
        }
        let updated = verify_unchanged_email(
            &tx,
            tenant_id,
            user.id,
            &user.email,
            claims.recovery_revision,
            verified_at,
        )
        .await?;

        if updated {
            Self::publish_user_event_in_tx(
                ctx,
                &tx,
                tenant_id,
                user.id,
                DomainEvent::UserEmailVerified { user_id: user.id },
            )
            .await?;
        } else {
            // A concurrent confirmation is idempotent only if it verified the
            // same address. A changed or deleted account must not succeed.
            let current = Users::find_by_id(user.id)
                .filter(users::Column::TenantId.eq(tenant_id))
                .one(&tx)
                .await
                .map_err(AuthLifecycleError::from)?;
            if !current.is_some_and(|record| {
                record.is_active() && record.matches_recovery_identity(
                    tenant_id, claims.user_id, &claims.sub, claims.recovery_revision,
                ) && record.email_verified_at.is_some()
            }) {
                return Err(AuthLifecycleError::InvalidVerificationToken);
            }
        }

        // Idempotent success also requires a currently admissible JWT.
        if rustok_auth::application_clock::verified_now(&tx).await? >= claims.expires_at
            || decode_email_verification_token(config, token).is_err()
        {
            tx.rollback().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::InvalidVerificationToken);
        }
        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(())
    }
}

/// Compare the exact stored address read before the transaction. This keeps a
/// token for an old mailbox from verifying a concurrently changed address.
async fn verify_unchanged_email<C: ConnectionTrait>(
    db: &C,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    email: &str,
    recovery_revision: i64,
    verified_at: chrono::DateTime<Utc>,
) -> Result<bool, AuthLifecycleError> {
    let updated = users::Entity::update_many()
        .col_expr(users::Column::EmailVerifiedAt, Expr::value(verified_at.clone()))
        .col_expr(users::Column::UpdatedAt, Expr::value(verified_at))
        .filter(users::Column::Id.eq(user_id))
        .filter(users::Column::TenantId.eq(tenant_id))
        .filter(users::Column::Email.eq(email))
        .filter(users::Column::Status.eq(rustok_core::UserStatus::Active))
        .filter(users::Column::RecoveryRevision.eq(recovery_revision))
        .filter(users::Column::EmailVerifiedAt.is_null())
        .exec(db)
        .await
        .map_err(AuthLifecycleError::from)?;
    Ok(updated.rows_affected == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::tenants;
    use rustok_migrations::SqliteTestMigrator;
    use rustok_test_utils::db::setup_test_db_with_migrations;
    use sea_orm::{ActiveModelTrait, Set};

    #[tokio::test]
    async fn recovery_fence_rejects_expired_deadline_before_account_mutation() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "recovery-deadline")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "user@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");

        let tx = db.begin().await.expect("transaction");
        assert!(
            AuthLifecycleService::lock_user_for_recovery(
                &tx, tenant.id, user.id, Utc::now(),
            )
            .await
            .expect("expired recovery fence")
            .is_none()
        );
        tx.rollback().await.expect("rollback");
        let stored = Users::find_by_id(user.id)
            .one(&db)
            .await
            .expect("query")
            .expect("user");
        assert_eq!(stored.password_hash, user.password_hash);
        assert!(stored.email_verified_at.is_none());

        let tx = db.begin().await.expect("transaction");
        let deadline = Utc::now() + chrono::Duration::minutes(5);
        let (locked, admitted_at) = AuthLifecycleService::lock_user_for_recovery(
            &tx, tenant.id, user.id, deadline,
        )
        .await
        .expect("live recovery fence")
        .expect("live user");
        assert_eq!(locked.id, user.id);
        assert!(admitted_at < deadline);
        tx.rollback().await.expect("rollback");
    }

    #[tokio::test]
    async fn stale_address_cannot_verify_a_changed_mailbox() {
        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "email-cas")
            .insert(&db)
            .await
            .expect("tenant");
        let user = users::ActiveModel::new(tenant.id, "old@example.com", "hash")
            .insert(&db)
            .await
            .expect("user");
        users::ActiveModel {
            id: Set(user.id),
            email_verified_at: Set(Some(Utc::now().into())),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("verify old address");
        users::ActiveModel {
            id: Set(user.id),
            email: Set("new@example.com".to_string()),
            email_verified_at: Set(None),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("change address");

        let verified_at = chrono::DateTime::<Utc>::from_timestamp(1_800_000_000, 0)
            .expect("fixed verification timestamp");
        assert!(
            !verify_unchanged_email(&db, tenant.id, user.id, &user.email, user.recovery_revision, verified_at.clone())
                .await
                .expect("compare and swap")
        );
        let current = Users::find_by_id(user.id)
            .one(&db)
            .await
            .expect("query")
            .expect("user");
        assert_eq!(current.email, "new@example.com");
        assert!(current.email_verified_at.is_none());
        assert!(
            verify_unchanged_email(&db, tenant.id, user.id, &current.email, current.recovery_revision, verified_at.clone())
                .await
                .expect("verify current address")
        );
        let verified = Users::find_by_id(user.id).one(&db).await.unwrap().unwrap();
        assert_eq!(verified.recovery_revision, current.recovery_revision);
        assert_eq!(verified.email_verified_at, Some(verified_at.clone().into()));
        assert_eq!(
            verified.email_verified_at,
            Some(verified.updated_at.clone()),
            "email verification and user update timestamps must be identical"
        );
        assert!(
            !verify_unchanged_email(&db, tenant.id, user.id, &current.email, current.recovery_revision, verified_at + chrono::Duration::seconds(1))
                .await
                .expect("already verified")
        );
        let replayed = Users::find_by_id(user.id).one(&db).await.unwrap().unwrap();
        assert_eq!(replayed.email_verified_at, verified.email_verified_at);
        assert_eq!(replayed.updated_at, verified.updated_at);
    }
}
