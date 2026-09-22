use chrono::Utc;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, TransactionTrait, sea_query::Expr};

use crate::auth::{
    AuthConfig, decode_password_reset_token, hash_password, password_reset_credential_matches,
};
use crate::models::{sessions, users};

use super::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use super::server_runtime_context::ServerRuntimeContext;

impl AuthLifecycleService {
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

        let previous_hash = user.password_hash;
        let new_hash = hash_password(new_password).map_err(AuthLifecycleError::from)?;
        let now = Utc::now();
        let tx = ctx.db().begin().await.map_err(AuthLifecycleError::from)?;

        let updated = users::Entity::update_many()
            .col_expr(users::Column::PasswordHash, Expr::value(new_hash))
            .col_expr(users::Column::UpdatedAt, Expr::value(now))
            .filter(users::Column::Id.eq(user.id))
            .filter(users::Column::TenantId.eq(tenant_id))
            .filter(users::Column::PasswordHash.eq(previous_hash))
            .exec(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;
        if updated.rows_affected != 1 {
            tx.rollback().await.map_err(AuthLifecycleError::from)?;
            return Err(AuthLifecycleError::InvalidResetToken);
        }

        sessions::Entity::update_many()
            .col_expr(sessions::Column::RevokedAt, Expr::value(now))
            .col_expr(sessions::Column::UpdatedAt, Expr::value(now))
            .filter(sessions::Column::TenantId.eq(tenant_id))
            .filter(sessions::Column::UserId.eq(user.id))
            .filter(sessions::Column::RevokedAt.is_null())
            .exec(&tx)
            .await
            .map_err(AuthLifecycleError::from)?;

        tx.commit().await.map_err(AuthLifecycleError::from)?;
        Ok(())
    }
}
