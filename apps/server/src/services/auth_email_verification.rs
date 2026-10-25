use chrono::Utc;
use rustok_events::DomainEvent;
use sea_orm::{
    ColumnTrait, EntityTrait, QueryFilter, TransactionTrait, sea_query::Expr,
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
        let updated = users::Entity::update_many()
            .col_expr(users::Column::EmailVerifiedAt, Expr::value(Utc::now()))
            .filter(users::Column::Id.eq(user.id))
            .filter(users::Column::TenantId.eq(tenant_id))
            .filter(users::Column::EmailVerifiedAt.is_null())
            .exec(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;

        if updated.rows_affected == 1 {
            Self::publish_user_event_in_tx(
                ctx.db_clone(),
                &tx,
                tenant_id,
                user.id,
                DomainEvent::UserEmailVerified { user_id: user.id },
            )
            .await?;
        }

        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(())
    }
}
