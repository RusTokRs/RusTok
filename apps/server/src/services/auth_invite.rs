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

#[derive(Debug)]
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
        // Canonical feature gate for every transport (REST middleware, REST
        // handler, GraphQL/native provider). Keep the rejection deliberately
        // indistinguishable from an invalid token so disabled invites do not
        // become an oracle for token validity.
        if !ctx.settings().features.invites_enabled {
            return Err(InviteAcceptanceError::InvalidToken);
        }

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
        let email =
            normalize_email(&claims.sub).map_err(|_| InviteAcceptanceError::InvalidToken)?;
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
        let ledger_role = custom_role_slug.clone().unwrap_or_else(|| role.to_string());
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
        let issued = match issued {
            Some(invite) if invite.revoked_at.is_none() => invite,
            _ => return Err(InviteAcceptanceError::InvalidToken),
        };
        if issued.role != ledger_role {
            return Err(InviteAcceptanceError::InvalidToken);
        }
        let invite_id = issued.id;

        // Role grants, module availability, and the issuer's authority can
        // change while an invite is outstanding. Re-check the issuer's active
        // invite permission, hierarchy, and current grant ceiling before
        // accepting it, so a later role edit cannot widen the invitation.
        // This happens before reservation, leaving policy-rejected invites
        // unconsumed.
        let issuer_id = issued
            .issued_by
            .ok_or(InviteAcceptanceError::InvalidToken)?;
        let issuer = users::Entity::find_by_id(issuer_id)
            .filter(users::Column::TenantId.eq(tenant_id))
            .filter(users::Column::Status.eq(rustok_core::UserStatus::Active))
            .one(&tx)
            .await
            .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
        if issuer.is_none() {
            return Err(InviteAcceptanceError::InvalidToken);
        }
        let mut issuer_permissions = RbacService::get_user_permissions_authoritative(
            &tx,
            &tenant_id,
            &issuer_id,
        )
        .await
        .map_err(InviteAcceptanceError::Internal)?;
        // Permission grants owned by disabled modules are persisted but are
        // not current request authority. Apply the same effective-permission
        // boundary used at invite issuance and regular authentication before
        // checking whether the issuer may delegate this role.
        let availability = crate::services::rbac_module_availability::permission_availability_for(
            ctx, tenant_id,
        )
        .await
        .map_err(|error| InviteAcceptanceError::Internal(error.into()))?;
        if let Some(availability) = availability {
            issuer_permissions = availability.active_permissions(&issuer_permissions);
        }
        let issuer_role = RbacService::get_user_role_authoritative(&tx, &tenant_id, &issuer_id)
            .await
            .map_err(InviteAcceptanceError::Internal)?;
        let may_issue_invite = issuer_may_issue_invite(
            &issuer_permissions,
            &role,
            custom_role_slug.is_some(),
        );
        if !may_issue_invite {
            tracing::warn!(
                tenant_id = %tenant_id,
                issuer_id = %issuer_id,
                role_slug = %ledger_role,
                "invite issuer no longer has authority to issue this invite"
            );
            return Err(InviteAcceptanceError::InvalidToken);
        }
        let authority_check = if let Some(slug) = custom_role_slug.as_deref() {
            rustok_rbac::validate_custom_role_grant_on(
                &tx,
                tenant_id,
                &issuer_role,
                &issuer_permissions,
                slug,
            )
            .await
        } else {
            async {
                // Issuance enforces both the persisted role hierarchy and
                // permission ceiling. Re-check both: an issuer demoted while
                // the invite was pending must not retain the old role's
                // assignment authority just because grants still overlap.
                if role != rustok_core::UserRole::Customer {
                    rustok_rbac::require_role_assignment(&issuer_role, &role)?;
                }
                rustok_rbac::validate_persisted_role_grant_on(
                    &tx,
                    tenant_id,
                    &issuer_permissions,
                    &role.to_string(),
                )
                .await
            }
            .await
        };
        if let Err(error) = authority_check {
            tracing::warn!(
                %error,
                tenant_id = %tenant_id,
                issuer_id = %issuer_id,
                role_slug = %ledger_role,
                "invite role no longer satisfies issuer authority"
            );
            return Err(InviteAcceptanceError::InvalidToken);
        }

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
        // revalidated against the current role grants before reservation.
        // If the role changes again concurrently, the transactional role
        // replacement still fails closed when the role no longer exists;
        // the transaction rollback leaves the invite unconsumed.
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
            Arc::new(OutboxTransport::new(ctx.db_clone())) as Arc<dyn EventTransport>
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

fn issuer_may_issue_invite(
    permissions: &[rustok_api::Permission],
    role: &rustok_core::UserRole,
    is_custom_role: bool,
) -> bool {
    use rustok_api::Permission;

    if is_custom_role || role != &rustok_core::UserRole::Customer {
        rustok_rbac::has_effective_permission_in_set(permissions, &Permission::USERS_MANAGE)
    } else {
        rustok_rbac::has_effective_permission_in_set(permissions, &Permission::USERS_CREATE)
            || rustok_rbac::has_effective_permission_in_set(
                permissions,
                &Permission::USERS_MANAGE,
            )
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
    use super::*;
    use crate::common::RustokSettings;
    use sha2::{Digest, Sha256};

    #[test]
    fn pending_invites_require_the_issuer_to_retain_invitation_authority() {
        use rustok_api::Permission;
        use rustok_core::UserRole;

        assert!(issuer_may_issue_invite(
            &[Permission::USERS_CREATE],
            &UserRole::Customer,
            false,
        ));
        assert!(!issuer_may_issue_invite(
            &[],
            &UserRole::Customer,
            false,
        ));
        assert!(!issuer_may_issue_invite(
            &[Permission::USERS_CREATE],
            &UserRole::Admin,
            false,
        ));
        assert!(issuer_may_issue_invite(
            &[Permission::USERS_MANAGE],
            &UserRole::Manager,
            true,
        ));
        assert!(!issuer_may_issue_invite(
            &[Permission::USERS_CREATE],
            &UserRole::Manager,
            true,
        ));

        let disabled_create = rustok_rbac::PermissionAvailability::new(
            std::collections::BTreeMap::from([("users:create".to_string(), "auth".to_string())]),
            std::collections::BTreeSet::new(),
        );
        let current_permissions =
            disabled_create.active_permissions(&[Permission::USERS_CREATE]);
        assert!(!issuer_may_issue_invite(
            &current_permissions,
            &UserRole::Customer,
            false,
        ));
    }

    #[tokio::test]
    async fn disabled_invites_reject_before_token_decode() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("sqlite test database");
        let mut settings = RustokSettings::default();
        settings.features.invites_enabled = false;
        let ctx = ServerRuntimeContext::new(db, settings);
        let config = AuthConfig::new("invite-feature-gate-secret".to_string());

        let error = AuthLifecycleService::accept_invite_once_runtime(
            &ctx,
            &config,
            uuid::Uuid::new_v4(),
            "not-a-jwt",
            "Password123!",
            None,
        )
        .await
        .expect_err("disabled invites must reject");

        assert!(matches!(error, InviteAcceptanceError::InvalidToken));
    }

    #[test]
    fn invite_digest_is_stable_and_does_not_store_plaintext() {
        let token = "signed.invite.token";
        let digest = hex::encode(Sha256::digest(token.as_bytes()));

        assert_eq!(digest.len(), 64);
        assert_ne!(digest, token);
        assert_eq!(digest, hex::encode(Sha256::digest(token.as_bytes())));
    }
}
