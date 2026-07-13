use async_trait::async_trait;
use chrono::Utc;
use rustok_api::Permission;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, UpdateUserCommand,
    UserAdminMutationPort, UserMutationRecord,
};
use rustok_core::{UserRole, UserStatus};
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use std::str::FromStr;
use uuid::Uuid;

use crate::auth::hash_password;
use crate::models::{sessions, users};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::flex_attached_values::FlexAttachedValuesService;
use crate::services::rbac_service::RbacService;

use super::{
    super_admin_guard::ensure_active_super_admin_continuity, ServerAuthAdminMutationProvider,
};

fn parse_user_status(value: &str) -> Result<UserStatus, AuthAdminMutationError> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "active" => Ok(UserStatus::Active),
        "inactive" => Ok(UserStatus::Inactive),
        "banned" => Ok(UserStatus::Banned),
        _ => Err(AuthAdminMutationError::Validation(format!(
            "unsupported user status: {value}"
        ))),
    }
}

fn parse_user_role(value: &str) -> Result<UserRole, AuthAdminMutationError> {
    UserRole::from_str(&value.trim().to_ascii_lowercase())
        .map_err(|error| AuthAdminMutationError::Validation(error.to_string()))
}

fn map_lifecycle_error(error: AuthLifecycleError) -> AuthAdminMutationError {
    match error {
        AuthLifecycleError::EmailAlreadyExists => {
            AuthAdminMutationError::Conflict("user email already exists".to_string())
        }
        other => AuthAdminMutationError::Internal(crate::error::Error::from(other).to_string()),
    }
}

fn map_custom_field_error(error: rustok_core::field_schema::FlexError) -> AuthAdminMutationError {
    match error {
        rustok_core::field_schema::FlexError::ValidationFailed(errors) => {
            AuthAdminMutationError::CustomFieldsValidation(
                serde_json::to_value(errors).unwrap_or_else(|_| serde_json::json!([])),
            )
        }
        other => AuthAdminMutationError::Internal(other.to_string()),
    }
}

fn forbidden(message: impl Into<String>) -> AuthAdminMutationError {
    AuthAdminMutationError::Forbidden(message.into())
}

impl ServerAuthAdminMutationProvider {
    async fn actor_role(
        &self,
        context: &AuthAdminMutationContext,
    ) -> Result<UserRole, AuthAdminMutationError> {
        RbacService::get_user_role(&self.db, &context.tenant_id, &context.actor_id)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))
    }

    async fn user_role(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<UserRole, AuthAdminMutationError> {
        RbacService::get_user_role(&self.db, &tenant_id, &user_id)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))
    }

    async fn ensure_role_assignment_allowed(
        &self,
        context: &AuthAdminMutationContext,
        requested_role: &UserRole,
    ) -> Result<(), AuthAdminMutationError> {
        let actor_role = self.actor_role(context).await?;
        if actor_role.can_assign_role(requested_role) {
            Ok(())
        } else {
            Err(forbidden("cannot assign a peer or higher-privileged role"))
        }
    }

    async fn ensure_target_management_allowed(
        &self,
        context: &AuthAdminMutationContext,
        target_user_id: Uuid,
        target_role: &UserRole,
    ) -> Result<(), AuthAdminMutationError> {
        if context.actor_id == target_user_id {
            return Ok(());
        }

        let actor_role = self.actor_role(context).await?;
        if actor_role.can_manage_role(target_role) {
            Ok(())
        } else {
            Err(forbidden(
                "cannot modify a peer or higher-privileged user",
            ))
        }
    }
}

async fn revoke_active_sessions<C>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    sessions::Entity::update_many()
        .col_expr(sessions::Column::RevokedAt, Expr::value(Utc::now()))
        .filter(sessions::Column::TenantId.eq(tenant_id))
        .filter(sessions::Column::UserId.eq(user_id))
        .filter(sessions::Column::RevokedAt.is_null())
        .exec(db)
        .await
        .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
    Ok(())
}

#[async_trait]
impl UserAdminMutationPort for ServerAuthAdminMutationProvider {
    async fn create_user(
        &self,
        context: &AuthAdminMutationContext,
        command: CreateUserCommand,
    ) -> Result<UserMutationRecord, AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_CREATE, Permission::USERS_MANAGE],
            "users:create or users:manage required",
        )
        .await?;

        let role = parse_user_role(command.role.as_deref().unwrap_or("customer"))?;
        let status = command
            .status
            .as_deref()
            .map(parse_user_status)
            .transpose()?;

        if role != UserRole::Customer
            || status
                .as_ref()
                .is_some_and(|value| value != &UserStatus::Active)
        {
            self.authorize_user(
                context,
                &[Permission::USERS_MANAGE],
                "users:manage required to create privileged or disabled users",
            )
            .await?;
        }
        if role != UserRole::Customer {
            self.ensure_role_assignment_allowed(context, &role).await?;
        }

        let locale = context
            .locale
            .as_deref()
            .unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE);
        let prepared = FlexAttachedValuesService::prepare_create(
            &self.db,
            context.tenant_id,
            "user",
            locale,
            command.custom_fields,
        )
        .await
        .map_err(map_custom_field_error)?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let mut user = AuthLifecycleService::create_user_in_tx(
            &tx,
            context.tenant_id,
            &command.email,
            &command.password,
            command.name,
            role,
            status,
        )
        .await
        .map_err(map_lifecycle_error)?;

        if let Some(metadata) = prepared.metadata {
            let mut active: users::ActiveModel = user.into();
            active.metadata = Set(metadata);
            user = active
                .update(&tx)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        }
        if let (Some(locale), Some(values)) = (
            prepared.locale.as_deref(),
            prepared.localized_values.as_ref(),
        ) {
            FlexAttachedValuesService::persist_localized_values(
                &tx,
                context.tenant_id,
                "user",
                user.id,
                locale,
                values,
            )
            .await
            .map_err(map_custom_field_error)?;
        }
        tx.commit()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        RbacService::invalidate_user_rbac_caches(&context.tenant_id, &user.id).await;
        self.user_record(user).await
    }

    async fn update_user(
        &self,
        context: &AuthAdminMutationContext,
        command: UpdateUserCommand,
    ) -> Result<UserMutationRecord, AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_UPDATE, Permission::USERS_MANAGE],
            "users:update or users:manage required",
        )
        .await?;
        let user = users::Entity::find_by_id(command.id)
            .filter(users::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?;
        let current_role = self.user_role(context.tenant_id, user.id).await?;
        self.ensure_target_management_allowed(context, user.id, &current_role)
            .await?;

        if command.role.is_some() || command.status.is_some() {
            self.authorize_user(
                context,
                &[Permission::USERS_MANAGE],
                "users:manage required to change user role or status",
            )
            .await?;
        }

        if let Some(email) = command.email.as_deref() {
            let existing = users::Entity::find_by_email(&self.db, context.tenant_id, email)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
            if existing
                .as_ref()
                .is_some_and(|existing| existing.id != user.id)
            {
                return Err(AuthAdminMutationError::Conflict(
                    "user email already exists".to_string(),
                ));
            }
        }

        let requested_role = command.role.as_deref().map(parse_user_role).transpose()?;
        if let Some(role) = requested_role.as_ref() {
            self.ensure_role_assignment_allowed(context, role).await?;
        }
        let requested_status = command.status.as_deref().map(parse_user_status).transpose()?;

        let locale = context
            .locale
            .as_deref()
            .unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE);
        let prepared = FlexAttachedValuesService::prepare_update(
            &self.db,
            context.tenant_id,
            "user",
            user.id,
            locale,
            &user.metadata,
            command.custom_fields,
        )
        .await
        .map_err(map_custom_field_error)?;
        let user_id = user.id;
        let password_changed = command.password.is_some();
        let status_disables_user = requested_status
            .as_ref()
            .is_some_and(|status| status != &UserStatus::Active);
        let mut active: users::ActiveModel = user.into();
        if let Some(email) = command.email {
            active.email = Set(email.to_lowercase());
        }
        if let Some(name) = command.name {
            active.name = Set(Some(name));
        }
        if let Some(status) = requested_status.as_ref() {
            active.status = Set(status.clone());
        }
        if let Some(password) = command.password {
            active.password_hash = Set(
                hash_password(&password)
                    .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?,
            );
        }
        if let Some(metadata) = prepared.metadata {
            active.metadata = Set(metadata);
        }

        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        ensure_active_super_admin_continuity(
            &tx,
            context.tenant_id,
            user_id,
            &current_role,
            requested_role.as_ref(),
            requested_status.as_ref(),
            false,
        )
        .await?;
        let user = active
            .update(&tx)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        if let Some(role) = requested_role {
            RbacService::replace_user_role(&tx, &user.id, &context.tenant_id, role)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        }
        if password_changed || status_disables_user {
            revoke_active_sessions(&tx, context.tenant_id, user.id).await?;
        }
        if let (Some(locale), Some(values)) = (
            prepared.locale.as_deref(),
            prepared.localized_values.as_ref(),
        ) {
            FlexAttachedValuesService::persist_localized_values(
                &tx,
                context.tenant_id,
                "user",
                user_id,
                locale,
                values,
            )
            .await
            .map_err(map_custom_field_error)?;
        }
        tx.commit()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        RbacService::invalidate_user_rbac_caches(&context.tenant_id, &user.id).await;
        self.user_record(user).await
    }

    async fn delete_user(
        &self,
        context: &AuthAdminMutationContext,
        user_id: Uuid,
    ) -> Result<(), AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_MANAGE],
            "users:manage required",
        )
        .await?;
        let user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?;
        let current_role = self.user_role(context.tenant_id, user.id).await?;
        self.ensure_target_management_allowed(context, user.id, &current_role)
            .await?;

        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        ensure_active_super_admin_continuity(
            &tx,
            context.tenant_id,
            user_id,
            &current_role,
            None,
            None,
            true,
        )
        .await?;
        revoke_active_sessions(&tx, context.tenant_id, user_id).await?;
        FlexAttachedValuesService::delete_localized_values(&tx, context.tenant_id, "user", user_id)
            .await
            .map_err(|error| AuthAdminMutationError::Validation(error.to_string()))?;
        let active: users::ActiveModel = user.into();
        active
            .delete(&tx)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        tx.commit()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        RbacService::invalidate_user_rbac_caches(&context.tenant_id, &user_id).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_user_role, parse_user_status};

    #[test]
    fn parses_admin_user_enums_case_insensitively() {
        assert_eq!(
            parse_user_role("MANAGER").expect("uppercase role"),
            rustok_core::UserRole::Manager
        );
        assert_eq!(
            parse_user_status("ACTIVE").expect("uppercase status"),
            rustok_core::UserStatus::Active
        );
    }

    #[test]
    fn trims_admin_user_enums_at_server_boundary() {
        assert_eq!(
            parse_user_role(" admin ").expect("trimmed role"),
            rustok_core::UserRole::Admin
        );
        assert_eq!(
            parse_user_status(" banned ").expect("trimmed status"),
            rustok_core::UserStatus::Banned
        );
    }
}