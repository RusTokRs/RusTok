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
    /// the user and email it was issued for. Confirmation is idempotent:
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
        let claims = decode_email_verification_token(config, token)
            .map_err(|_| AuthLifecycleError::InvalidVerificationToken)?;
        if claims.tenant_id != tenant_id {
            return Err(AuthLifecycleError::InvalidVerificationToken);
        }

        let user = Users::find_by_id(claims.user_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .one(ctx.db())
            .await
            .map_err(AuthLifecycleError::from)?
            .filter(|record| record.email.eq_ignore_ascii_case(&claims.sub))
            .ok_or(AuthLifecycleError::InvalidVerificationToken)?;

        if user.email_verified_at.is_some() {
            return Ok(());
        }

        let tx = ctx.db().begin().await.map_err(AuthLifecycleError::from)?;
        let updated = verify_unchanged_email(&tx, tenant_id, user.id, &user.email).await?;

        if updated {
            Self::publish_user_event_in_tx(
                ctx.db_clone(),
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
                record.email == user.email && record.email_verified_at.is_some()
            }) {
                return Err(AuthLifecycleError::InvalidVerificationToken);
            }
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
) -> Result<bool, AuthLifecycleError> {
    let updated = users::Entity::update_many()
        .col_expr(users::Column::EmailVerifiedAt, Expr::value(Utc::now()))
        .filter(users::Column::Id.eq(user_id))
        .filter(users::Column::TenantId.eq(tenant_id))
        .filter(users::Column::Email.eq(email))
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
            email: Set("new@example.com".to_string()),
            email_verified_at: Set(None),
            ..Default::default()
        }
        .update(&db)
        .await
        .expect("change address");

        assert!(
            !verify_unchanged_email(&db, tenant.id, user.id, &user.email)
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
            verify_unchanged_email(&db, tenant.id, user.id, &current.email)
                .await
                .expect("verify current address")
        );
        assert!(
            !verify_unchanged_email(&db, tenant.id, user.id, &current.email)
                .await
                .expect("already verified")
        );
    }
}
