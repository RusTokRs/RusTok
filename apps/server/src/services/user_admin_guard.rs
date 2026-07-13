use std::sync::Arc;

use async_trait::async_trait;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, UpdateUserCommand,
    UserAdminMutationPort, UserMutationRecord,
};
use uuid::Uuid;

/// Cross-transport guard for mutations that would invalidate the immutable
/// authorization snapshot of the request currently being executed.
pub struct GuardedUserAdminMutationProvider {
    inner: Arc<dyn UserAdminMutationPort>,
}

impl GuardedUserAdminMutationProvider {
    pub fn new(inner: Arc<dyn UserAdminMutationPort>) -> Self {
        Self { inner }
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
