use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{Permission, has_effective_permission};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, UpdateUserCommand,
    UserAdminMutationPort, UserMutationRecord,
};
use rustok_core::{Rbac, UserRole};
use uuid::Uuid;

use super::rbac_request_scope::{permissions_for, role_for};

/// Cross-transport guard for user administration.
///
/// The inner provider performs authoritative database checks and serializes
/// role mutations. This decorator additionally preserves the immutable
/// request-effective authority snapshot, so an OAuth token cannot regain
/// permissions removed by its scopes when the provider reads the actor from DB.
pub struct GuardedUserAdminMutationProvider {
    inner: Arc<dyn UserAdminMutationPort>,
}

impl GuardedUserAdminMutationProvider {
    pub fn new(inner: Arc<dyn UserAdminMutationPort>) -> Self {
        Self { inner }
    }

    fn request_permissions(
        context: &AuthAdminMutationContext,
    ) -> Result<Vec<Permission>, AuthAdminMutationError> {
        permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            AuthAdminMutationError::Forbidden(
                "user administration requires a request-bound effective permission snapshot"
                    .to_string(),
            )
        })
    }

    fn request_role(
        context: &AuthAdminMutationContext,
    ) -> Result<UserRole, AuthAdminMutationError> {
        role_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            AuthAdminMutationError::Forbidden(
                "user administration requires a request-bound role snapshot".to_string(),
            )
        })
    }

    fn require_any(
        context: &AuthAdminMutationContext,
        required: &[Permission],
        message: &str,
    ) -> Result<Vec<Permission>, AuthAdminMutationError> {
        let authority = Self::request_permissions(context)?;
        if required
            .iter()
            .any(|permission| has_effective_permission(&authority, permission))
        {
            Ok(authority)
        } else {
            Err(AuthAdminMutationError::Forbidden(message.to_string()))
        }
    }

    fn parse_role(value: &str) -> Result<UserRole, AuthAdminMutationError> {
        UserRole::from_str(&value.trim().to_ascii_lowercase())
            .map_err(|error| AuthAdminMutationError::Validation(error.to_string()))
    }

    fn validate_role_grant(
        context: &AuthAdminMutationContext,
        authority: &[Permission],
        requested_role: &UserRole,
    ) -> Result<(), AuthAdminMutationError> {
        let actor_role = Self::request_role(context)?;
        if !actor_role.can_assign_role(requested_role) {
            return Err(AuthAdminMutationError::Forbidden(
                "cannot assign a peer or higher-privileged role".to_string(),
            ));
        }

        // Customer is the platform's baseline account role. Provisioning a
        // customer remains part of users:create/users:manage. Any privileged
        // role, however, delegates cross-domain authority and must fit inside
        // the current token's effective permission ceiling.
        if requested_role == &UserRole::Customer {
            return Ok(());
        }

        for permission in Rbac::permissions_for_role(requested_role) {
            if !has_effective_permission(authority, permission) {
                return Err(AuthAdminMutationError::Forbidden(format!(
                    "cannot assign role `{requested_role}` because permission `{permission}` exceeds the current request authority"
                )));
            }
        }
        Ok(())
    }
}

fn self_lifecycle_change_requested(
    context: &AuthAdminMutationContext,
    command: &UpdateUserCommand,
) -> bool {
    if context.actor_id != command.id {
        return false;
    }

    command.role.is_some()
        || command
            .status
            .as_deref()
            .is_some_and(|status| !status.trim().eq_ignore_ascii_case("active"))
}

#[async_trait]
impl UserAdminMutationPort for GuardedUserAdminMutationProvider {
    async fn create_user(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateUserCommand,
    ) -> Result<UserMutationRecord, AuthAdminMutationError> {
        let authority = Self::require_any(
            context,
            &[Permission::USERS_CREATE, Permission::USERS_MANAGE],
            "users:create or users:manage required",
        )?;
        let role = Self::parse_role(command.role.as_deref().unwrap_or("customer"))?;
        let creates_non_active_user = command
            .status
            .as_deref()
            .is_some_and(|status| !status.trim().eq_ignore_ascii_case("active"));

        if (role != UserRole::Customer || creates_non_active_user)
            && !has_effective_permission(&authority, &Permission::USERS_MANAGE)
        {
            return Err(AuthAdminMutationError::Forbidden(
                "users:manage required to create privileged or disabled users".to_string(),
            ));
        }
        if role != UserRole::Customer {
            Self::validate_role_grant(context, &authority, &role)?;
        }

        self.inner.create_user(context, command).await
    }

    async fn update_user(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateUserCommand,
    ) -> Result<UserMutationRecord, AuthAdminMutationError> {
        if self_lifecycle_change_requested(context, &command) {
            return Err(AuthAdminMutationError::Forbidden(
                "cannot change your own role or disable your own account".to_string(),
            ));
        }

        let authority = Self::require_any(
            context,
            &[Permission::USERS_UPDATE, Permission::USERS_MANAGE],
            "users:update or users:manage required",
        )?;
        if (command.role.is_some() || command.status.is_some())
            && !has_effective_permission(&authority, &Permission::USERS_MANAGE)
        {
            return Err(AuthAdminMutationError::Forbidden(
                "users:manage required to change user role or status".to_string(),
            ));
        }
        if let Some(role) = command.role.as_deref() {
            let role = Self::parse_role(role)?;
            if role != UserRole::Customer {
                Self::validate_role_grant(context, &authority, &role)?;
            }
        }

        self.inner.update_user(context, command).await
    }

    async fn delete_user(
        &self,
        context: &AuthAdminMutationContext,
        user_id: Uuid,
    ) -> Result<(), AuthAdminMutationError> {
        if context.actor_id == user_id {
            return Err(AuthAdminMutationError::Forbidden(
                "cannot delete your own account through the administrative API".to_string(),
            ));
        }
        Self::require_any(
            context,
            &[Permission::USERS_MANAGE],
            "users:manage required",
        )?;

        self.inner.delete_user(context, user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::self_lifecycle_change_requested;
    use rustok_auth::{AuthAdminMutationContext, UpdateUserCommand};
    use uuid::Uuid;

    fn context(actor_id: Uuid) -> AuthAdminMutationContext {
        AuthAdminMutationContext {
            actor_id,
            tenant_id: Uuid::new_v4(),
            request_id: None,
            locale: None,
        }
    }

    fn command(id: Uuid) -> UpdateUserCommand {
        UpdateUserCommand {
            id,
            email: None,
            password: None,
            name: None,
            role: None,
            status: None,
            custom_fields: None,
        }
    }

    #[test]
    fn self_role_change_is_rejected() {
        let actor_id = Uuid::new_v4();
        let context = context(actor_id);
        let mut command = command(actor_id);
        command.role = Some("customer".to_string());

        assert!(self_lifecycle_change_requested(&context, &command));
    }

    #[test]
    fn self_disable_is_rejected_but_profile_update_is_allowed() {
        let actor_id = Uuid::new_v4();
        let context = context(actor_id);
        let mut disabled = command(actor_id);
        disabled.status = Some("inactive".to_string());
        assert!(self_lifecycle_change_requested(&context, &disabled));

        let mut profile = command(actor_id);
        profile.name = Some("New name".to_string());
        profile.status = Some("ACTIVE".to_string());
        assert!(!self_lifecycle_change_requested(&context, &profile));
    }

    #[test]
    fn another_user_lifecycle_change_is_delegated() {
        let actor_id = Uuid::new_v4();
        let context = context(actor_id);
        let mut command = command(Uuid::new_v4());
        command.role = Some("manager".to_string());
        command.status = Some("inactive".to_string());

        assert!(!self_lifecycle_change_requested(&context, &command));
    }
}
