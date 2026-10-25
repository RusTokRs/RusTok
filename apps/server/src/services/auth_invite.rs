use std::sync::Arc;

use chrono::{DateTime, Utc};
use rustok_core::events::EventTransport;
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use sha2::{Digest, Sha256};

use crate::auth::{AuthConfig, decode_invite_token, normalize_email};
use crate::error::Error;
use crate::models::{auth_invite_consumptions, auth_invites, users};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Debug)]
pub enum InviteAcceptanceError {
    InvalidToken,
    EmailAlreadyExists,
    Validation(String),
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
            InviteAcceptanceError::Validation(message) => Error::Validation(message),
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
        let claims =
            decode_invite_token(config, token).map_err(|_| InviteAcceptanceError::InvalidToken)?;
        if claims.tenant_id != tenant_id {
            return Err(InviteAcceptanceError::InvalidToken);
        }

        let expires_at = DateTime::<Utc>::from_timestamp(claims.exp as i64, 0)
            .ok_or(InviteAcceptanceError::InvalidToken)?;
        if expires_at <= Utc::now() {
            return Err(InviteAcceptanceError::InvalidToken);
        }

        // A signed invite carrying a malformed address is an invalid token,
        // not caller input worth a distinct validation response. Validate
        // and hash the credentials *before* opening the transaction and
        // reserving the token digest, so a policy failure never burns a
        // single-use invite and Argon2 never runs inside the transaction.
        let email = normalize_email(&claims.sub)
            .map_err(|_| InviteAcceptanceError::InvalidToken)?;
        let prepared = AuthLifecycleService::prepare_new_user(
            &email,
            password,
            name,
            config.min_password_length,
        )
        .await
        .map_err(map_lifecycle_error)?;
        let role = claims.role;
        let custom_role_slug = claims.custom_role_slug.clone();
        let ledger_role = custom_role_slug
            .clone()
            .unwrap_or_else(|| role.to_string());
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        let consumption_id = uuid::Uuid::new_v4();
        let consumed_at = Utc::now();

        let tx = ctx
            .db()
            .begin()
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

        // Fail closed on the issuance ledger: only tokens an
        // administrator actually issued (and has not revoked) are
        // acceptable, even if the signature verifies.
        let issued = auth_invites::Entity::find()
            .filter(auth_invites::Column::TokenHash.eq(&token_hash))
            .filter(auth_invites::Column::TenantId.eq(tenant_id))
            .one(&tx)
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
        let invite_id = match issued {
            Some(invite) if invite.revoked_at.is_none() => invite.id,
            _ => return Err(InviteAcceptanceError::InvalidToken),
        };

        // Reserve the digest first. The unique token_hash constraint is the
        // serialization point, so every concurrent replay is rejected before
        // user creation regardless of email uniqueness or transaction timing.
        let reservation = auth_invite_consumptions::ActiveModel {
            id: Set(consumption_id),
            tenant_id: Set(tenant_id),
            token_hash: Set(token_hash.clone()),
            email: Set(email.clone()),
            role: Set(ledger_role),
            user_id: Set(None),
            expires_at: Set(expires_at.into()),
            consumed_at: Set(consumed_at.into()),
        };
        if let Err(insert_error) = reservation.insert(&tx).await {
            tx.rollback()
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

            let already_consumed = auth_invite_consumptions::Entity::find()
                .filter(auth_invite_consumptions::Column::TokenHash.eq(&token_hash))
                .one(ctx.db())
                .await
                .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
                .is_some();
            return if already_consumed {
                Err(InviteAcceptanceError::InvalidToken)
            } else {
                Err(InviteAcceptanceError::Internal(insert_error.into()))
            };
        }

        let user = match AuthLifecycleService::create_user_in_tx(
            &tx,
            tenant_id,
            prepared,
            role.clone(),
            Some(rustok_core::UserStatus::Active),
        )
        .await
        {
            Ok(user) => user,
            Err(error) => {
                tx.rollback()
                    .await
                    .map_err(|rollback| InviteAcceptanceError::Internal(rollback.into()))?;
                return Err(map_lifecycle_error(error));
            }
        };

        // Layer the tenant-defined role over the baseline customer
        // membership created above. The issuer's authority ceiling was
        // validated at issuance; acceptance only needs the role to
        // still exist. If it was deleted meanwhile this fails closed
        // and the transaction rollback leaves the invite unconsumed,
        // so it can be retried after repair or revoked by an admin.
        let role = if let Some(slug) = custom_role_slug.as_deref() {
            rustok_rbac::replace_persisted_user_role_by_slug_on(&tx, tenant_id, user.id, slug)
                .await
                .map_err(|error| {
                    tracing::error!(
                        %error,
                        role_slug = slug,
                        "invite custom role assignment failed"
                    );
                    InviteAcceptanceError::Internal(Error::InternalServerError)
                })?;
            let permissions =
                RbacService::get_user_permissions_authoritative(&tx, &tenant_id, &user.id)
                    .await
                    .map_err(InviteAcceptanceError::Internal)?;
            rustok_core::infer_user_role_from_permissions(&permissions)
        } else {
            role
        };

        let reservation = auth_invite_consumptions::Entity::find_by_id(consumption_id)
            .one(&tx)
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?
            .ok_or_else(|| {
                InviteAcceptanceError::Internal(Error::Message(
                    "invite consumption reservation disappeared".to_string(),
                ))
            })?;
        let mut active: auth_invite_consumptions::ActiveModel = reservation.into();
        active.user_id = Set(Some(user.id));
        active
            .update(&tx)
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

        let event_bus = TransactionalEventBus::new(
            Arc::new(OutboxTransport::new(ctx.db_clone())) as Arc<dyn EventTransport>,
        );
        event_bus
            .publish_in_tx(
                &tx,
                tenant_id,
                Some(user.id),
                DomainEvent::UserAccountRegistered { user_id: user.id },
            )
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

        event_bus
            .publish_in_tx(
                &tx,
                tenant_id,
                Some(user.id),
                DomainEvent::UserInviteAccepted {
                    user_id: user.id,
                    invite_id,
                },
            )
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

        tx.commit()
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;

        Ok(AcceptedInvite { user, email, role })
    }
}

fn map_lifecycle_error(error: AuthLifecycleError) -> InviteAcceptanceError {
    match error {
        AuthLifecycleError::EmailAlreadyExists => InviteAcceptanceError::EmailAlreadyExists,
        AuthLifecycleError::Validation(message) => InviteAcceptanceError::Validation(message),
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
