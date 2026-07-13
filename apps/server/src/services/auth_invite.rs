use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use sha2::{Digest, Sha256};

use crate::auth::{decode_invite_token, AuthConfig};
use crate::error::Error;
use crate::models::{auth_invite_consumptions, users};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Debug)]
pub enum InviteAcceptanceError {
    InvalidToken,
    EmailAlreadyExists,
    Internal(Error),
}

impl From<InviteAcceptanceError> for Error {
    fn from(value: InviteAcceptanceError) -> Self {
        match value {
            InviteAcceptanceError::InvalidToken => {
                Error::Unauthorized("Invalid or already consumed invite token".to_string())
            }
            InviteAcceptanceError::EmailAlreadyExists => {
                Error::BadRequest("A user with this email already exists".to_string())
            }
            InviteAcceptanceError::Internal(error) => error,
        }
    }
}

pub struct AcceptedInvite {
    pub user: users::Model,
    pub email: String,
    pub role: rustok_core::UserRole,
}

impl AuthLifecycleService {
    /// Validate and consume an invitation exactly once while creating the user
    /// in the same database transaction.
    ///
    /// The SHA-256 token digest remains persisted after the created account is
    /// deleted, so an old stateless invite cannot be replayed to recreate the
    /// account before the JWT expires.
    pub async fn accept_invite_once_runtime(
        ctx: &ServerRuntimeContext,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        token: &str,
        password: &str,
        name: Option<String>,
    ) -> Result<AcceptedInvite, InviteAcceptanceError> {
        let claims = decode_invite_token(config, token)
            .map_err(|_| InviteAcceptanceError::InvalidToken)?;
        if claims.tenant_id != tenant_id {
            return Err(InviteAcceptanceError::InvalidToken);
        }

        let expires_at = DateTime::<Utc>::from_timestamp(claims.exp as i64, 0)
            .ok_or(InviteAcceptanceError::InvalidToken)?;
        if expires_at <= Utc::now() {
            return Err(InviteAcceptanceError::InvalidToken);
        }

        let email = claims.sub.trim().to_lowercase();
        if email.is_empty() {
            return Err(InviteAcceptanceError::InvalidToken);
        }
        let role = claims.role;
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));

        let tx = ctx
            .db()
            .begin()
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

        let already_consumed = auth_invite_consumptions::Entity::find()
            .filter(auth_invite_consumptions::Column::TokenHash.eq(&token_hash))
            .one(&tx)
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
            .is_some();
        if already_consumed {
            tx.rollback()
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
            return Err(InviteAcceptanceError::InvalidToken);
        }

        let user = AuthLifecycleService::create_user_in_tx(
            &tx,
            tenant_id,
            &email,
            password,
            name,
            role.clone(),
            Some(rustok_core::UserStatus::Active),
        )
        .await
        .map_err(map_lifecycle_error)?;

        let consumption = auth_invite_consumptions::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            token_hash: Set(token_hash.clone()),
            email: Set(email.clone()),
            role: Set(role.to_string()),
            user_id: Set(Some(user.id)),
            expires_at: Set(expires_at.into()),
            consumed_at: Set(Utc::now().into()),
        };

        if let Err(insert_error) = consumption.insert(&tx).await {
            tx.rollback()
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

            let replay_won_elsewhere = auth_invite_consumptions::Entity::find()
                .filter(auth_invite_consumptions::Column::TokenHash.eq(&token_hash))
                .one(ctx.db())
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
                .is_some();
            return if replay_won_elsewhere {
                Err(InviteAcceptanceError::InvalidToken)
            } else {
                Err(InviteAcceptanceError::Internal(insert_error.into()))
            };
        }

        tx.commit()
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

        Ok(AcceptedInvite { user, email, role })
    }
}

fn map_lifecycle_error(error: AuthLifecycleError) -> InviteAcceptanceError {
    match error {
        AuthLifecycleError::EmailAlreadyExists => InviteAcceptanceError::EmailAlreadyExists,
        other => InviteAcceptanceError::Internal(Error::from(other)),
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    #[test]
    fn invite_digest_is_stable_and_does_not_store_plaintext() {
        let token = "signed.invite.token";
        let digest = hex::encode(Sha256::digest(token.as_bytes()));

        assert_eq!(digest.len(), 64);
        assert_ne!(digest, token);
        assert_eq!(digest, hex::encode(Sha256::digest(token.as_bytes())));
    }
}
