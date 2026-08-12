use async_trait::async_trait;
use chrono::Utc;
use rustok_api::Permission;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, UpdateUserCommand,
    UserAdminMutationPort, UserMutationRecord,
};
use rustok_core::events::EventTransport;
use rustok_core::{UserRole, UserStatus, infer_user_role_from_permissions};
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_rbac::{
    RbacRoleMutationFacts, RbacRoleMutationOutcome, RbacRoleMutationPlan,
    RbacRoleMutationPolicyError, plan_user_role_mutation,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
    QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use std::{str::FromStr, sync::Arc};
use uuid::Uuid;

use crate::auth::hash_password;
use crate::models::{
    _entities::{roles, user_roles},
    sessions, users,
};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::flex_attached_values::FlexAttachedValuesService;
use crate::services::rbac_cache_invalidation::publish_user_rbac_invalidation;
use crate::services::rbac_invalidation_generation::reserve_rbac_invalidation_generation;
use crate::services::rbac_request_scope::role_for;
use crate::services::rbac_service::RbacService;

use super::{
    ServerAuthAdminMutationProvider,
    super_admin_guard::{
        count_remaining_active_super_admins, ensure_active_super_admin_continuity,
    },
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

fn status_change_requested(
    current_status: &UserStatus,
    requested_status: Option<&UserStatus>,
) -> bool {
    requested_status.is_some_and(|status| status != current_status)
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

fn map_role_mutation_policy_error(error: RbacRoleMutationPolicyError) -> AuthAdminMutationError {
    match error {
        RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole
        | RbacRoleMutationPolicyError::CannotManagePeerOrHigherUser
        | RbacRoleMutationPolicyError::ActorTenantMismatch
        | RbacRoleMutationPolicyError::TargetTenantMismatch => {
            AuthAdminMutationError::Forbidden(error.to_string())
        }
        RbacRoleMutationPolicyError::LastActiveSuperAdmin => {
            AuthAdminMutationError::Conflict(error.to_string())
        }
        RbacRoleMutationPolicyError::NilIdentity(_)
        | RbacRoleMutationPolicyError::InvalidDurableGeneration => {
            AuthAdminMutationError::Internal(error.to_string())
        }
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
        role_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            AuthAdminMutationError::Forbidden(
                "user administration requires a request-bound role snapshot".to_string(),
            )
        })
    }

    async fn user_role<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<UserRole, AuthAdminMutationError>
    where
        C: ConnectionTrait,
    {
        let permissions = RbacService::get_user_permissions_authoritative(db, &tenant_id, &user_id)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        Ok(infer_user_role_from_permissions(&permissions))
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
            Err(forbidden("cannot modify a peer or higher-privileged user"))
        }
    }
}

async fn lock_user_for_mutation<C>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<users::Model, AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    let query = || users::Entity::find_by_id(user_id).filter(users::Column::TenantId.eq(tenant_id));

    let user = match db.get_database_backend() {
        DbBackend::Postgres | DbBackend::MySql => query()
            .lock_exclusive()
            .one(db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?,
        DbBackend::Sqlite => {
            let user = query()
                .one(db)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
            if let Some(user) = user.as_ref() {
                users::Entity::update_many()
                    .col_expr(
                        users::Column::UpdatedAt,
                        Expr::col(users::Column::UpdatedAt).into(),
                    )
                    .filter(users::Column::Id.eq(user.id))
                    .filter(users::Column::TenantId.eq(tenant_id))
                    .exec(db)
                    .await
                    .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
            }
            user
        }
    };

    user.ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))
}

async fn has_exact_tenant_role_assignment<C>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
    requested_role: &UserRole,
) -> Result<bool, AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    let tenant_roles = roles::Entity::find()
        .filter(roles::Column::TenantId.eq(tenant_id))
        .all(db)
        .await
        .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
    let requested_slug = requested_role.to_string();
    let requested_role_id = tenant_roles
        .iter()
        .find(|role| role.slug == requested_slug && role.is_system)
        .map(|role| role.id);
    let tenant_role_ids = tenant_roles
        .into_iter()
        .map(|role| role.id)
        .collect::<Vec<_>>();
    if requested_role_id.is_none() || tenant_role_ids.is_empty() {
        return Ok(false);
    }

    let assignments = user_roles::Entity::find()
        .filter(user_roles::Column::UserId.eq(user_id))
        .filter(user_roles::Column::RoleId.is_in(tenant_role_ids))
        .all(db)
        .await
        .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
    Ok(assignments.len() == 1 && assignments[0].role_id == requested_role_id.unwrap())
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

async fn publish_committed_user_invalidation(
    tenant_id: Uuid,
    user_id: Uuid,
    durable_generation: u64,
) {
    RbacService::invalidate_user_rbac_caches(&tenant_id, &user_id).await;
    if let Err(error) =
        publish_user_rbac_invalidation(&tenant_id, &user_id, durable_generation).await
    {
        tracing::warn!(
            %error,
            durable_generation,
            %tenant_id,
            %user_id,
            "User administration fast RBAC invalidation fan-out failed; durable generation reconciliation will recover"
        );
        rustok_telemetry::metrics::record_event_error(
            "rbac.permissions.durable_generation.v1",
            "post_commit_fanout",
        );
    }
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
        let initial_user = users::Entity::find_by_id(command.id)
            .filter(users::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
            .ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?;

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
                .is_some_and(|existing| existing.id != initial_user.id)
            {
                return Err(AuthAdminMutationError::Conflict(
                    "user email already exists".to_string(),
                ));
            }
        }

        let requested_role = command.role.as_deref().map(parse_user_role).transpose()?;
        let requested_status = command
            .status
            .as_deref()
            .map(parse_user_status)
            .transpose()?;

        let locale = context
            .locale
            .as_deref()
            .unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE);
        let prepared = FlexAttachedValuesService::prepare_update(
            &self.db,
            context.tenant_id,
            "user",
            initial_user.id,
            locale,
            &initial_user.metadata,
            command.custom_fields,
        )
        .await
        .map_err(map_custom_field_error)?;
        let password_changed = command.password.is_some();

        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let locked_user = lock_user_for_mutation(&tx, context.tenant_id, command.id).await?;
        let current_role = self
            .user_role(&tx, context.tenant_id, locked_user.id)
            .await?;
        let user_id = locked_user.id;
        let target_tenant_id = locked_user.tenant_id;
        let target_status = locked_user.status.clone();
        let status_changed = status_change_requested(&target_status, requested_status.as_ref());

        let role_mutation_plan: Option<RbacRoleMutationPlan> = if let Some(requested_role) =
            requested_role.as_ref()
        {
            let actor_role = self.actor_role(context).await?;
            let resulting_status = requested_status
                .clone()
                .unwrap_or_else(|| target_status.clone());
            let removes_active_super_admin = current_role == UserRole::SuperAdmin
                && target_status == UserStatus::Active
                && (requested_role != &UserRole::SuperAdmin
                    || resulting_status != UserStatus::Active);
            let remaining_active_super_admins = if removes_active_super_admin {
                count_remaining_active_super_admins(&tx, context.tenant_id, user_id).await?
            } else {
                0
            };
            let assignment_is_exact =
                has_exact_tenant_role_assignment(&tx, context.tenant_id, user_id, requested_role)
                    .await?;
            match plan_user_role_mutation(RbacRoleMutationFacts {
                tenant_id: context.tenant_id,
                actor_id: context.actor_id,
                actor_tenant_id: context.tenant_id,
                actor_role,
                target_user_id: user_id,
                target_tenant_id,
                target_role: current_role.clone(),
                target_status: target_status.clone(),
                requested_role: requested_role.clone(),
                resulting_status,
                assignment_is_exact,
                remaining_active_super_admins,
            })
            .map_err(map_role_mutation_policy_error)?
            {
                RbacRoleMutationOutcome::Noop => None,
                RbacRoleMutationOutcome::Apply(plan) => Some(plan),
            }
        } else {
            self.ensure_target_management_allowed(context, user_id, &current_role)
                .await?;
            None
        };

        let user_row_update_requested = command.email.is_some()
            || command.name.is_some()
            || status_changed
            || command.password.is_some()
            || prepared.metadata.is_some();
        let mut active: users::ActiveModel = locked_user.clone().into();
        if let Some(email) = command.email {
            active.email = Set(email.to_lowercase());
        }
        if let Some(name) = command.name {
            active.name = Set(Some(name));
        }
        if status_changed {
            if let Some(status) = requested_status.as_ref() {
                active.status = Set(status.clone());
            }
        }
        if let Some(password) = command.password {
            active.password_hash = Set(hash_password(&password)
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?);
        }
        if let Some(metadata) = prepared.metadata {
            active.metadata = Set(metadata);
        }

        if requested_role.is_none() {
            let effective_requested_status = if status_changed {
                requested_status.as_ref()
            } else {
                None
            };
            ensure_active_super_admin_continuity(
                &tx,
                context.tenant_id,
                user_id,
                &current_role,
                None,
                effective_requested_status,
                false,
            )
            .await?;
        }
        let user = if user_row_update_requested {
            active
                .update(&tx)
                .await
                .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?
        } else {
            locked_user
        };
        if let Some(plan) = role_mutation_plan.as_ref() {
            RbacService::replace_user_role_in_transaction(
                &tx,
                &user.id,
                &context.tenant_id,
                plan.new_role().clone(),
            )
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        }
        let status_disables_user = status_changed
            && requested_status
                .as_ref()
                .is_some_and(|status| status != &UserStatus::Active);
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
        let invalidates_authorization = role_mutation_plan.is_some() || status_changed;
        let durable_generation = if invalidates_authorization {
            Some(
                reserve_rbac_invalidation_generation(&tx)
                    .await
                    .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?,
            )
        } else {
            None
        };

        if let Some(plan) = role_mutation_plan.as_ref() {
            let generation = durable_generation.ok_or_else(|| {
                AuthAdminMutationError::Internal(
                    "RBAC role mutation did not reserve a durable generation".to_string(),
                )
            })?;
            let event = plan
                .integration_event(generation)
                .map_err(map_role_mutation_policy_error)?;
            let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(
                self.db.clone(),
            )) as Arc<dyn EventTransport>);
            if let Err(error) = event_bus
                .publish_contract_in_tx(&tx, plan.tenant_id(), Some(plan.actor_id()), event)
                .await
            {
                let rollback_error = tx.rollback().await.err();
                tracing::error!(
                    %error,
                    ?rollback_error,
                    tenant_id = %context.tenant_id,
                    actor_id = %context.actor_id,
                    user_id = %user.id,
                    "Durable RBAC role mutation event publication failed; user update rolled back"
                );
                return Err(AuthAdminMutationError::Internal(
                    "durable RBAC role mutation event is unavailable".to_string(),
                ));
            }
        }

        tx.commit()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        if let Some(durable_generation) = durable_generation {
            publish_committed_user_invalidation(context.tenant_id, user.id, durable_generation)
                .await;
        }
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

        let event_bus = TransactionalEventBus::new(
            Arc::new(OutboxTransport::new(self.db.clone())) as Arc<dyn EventTransport>
        );
        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        let user = lock_user_for_mutation(&tx, context.tenant_id, user_id).await?;
        let current_role = self.user_role(&tx, context.tenant_id, user.id).await?;
        self.ensure_target_management_allowed(context, user.id, &current_role)
            .await?;
        ensure_active_super_admin_continuity(
            &tx,
            context.tenant_id,
            user.id,
            &current_role,
            None,
            None,
            true,
        )
        .await?;
        AuthLifecycleService::deactivate_user_in_tx(&tx, context.tenant_id, user.id)
            .await
            .map_err(map_lifecycle_error)?;
        #[cfg(feature = "mod-profiles")]
        rustok_profiles::redact_profile_for_account_deactivation_in_tx(
            &tx,
            context.tenant_id,
            user.id,
        )
        .await
        .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        revoke_active_sessions(&tx, context.tenant_id, user.id).await?;
        let durable_generation = reserve_rbac_invalidation_generation(&tx)
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        if let Err(error) = event_bus
            .publish_in_tx(
                &tx,
                context.tenant_id,
                Some(context.actor_id),
                DomainEvent::UserDeleted { user_id: user.id },
            )
            .await
        {
            let rollback_error = tx.rollback().await.err();
            tracing::error!(
                %error,
                ?rollback_error,
                tenant_id = %context.tenant_id,
                user_id = %user.id,
                "Durable UserDeleted publication failed; account deactivation rolled back"
            );
            return Err(AuthAdminMutationError::Internal(
                "durable user deletion invalidation is unavailable".to_string(),
            ));
        }
        tx.commit()
            .await
            .map_err(|error| AuthAdminMutationError::Internal(error.to_string()))?;
        publish_committed_user_invalidation(context.tenant_id, user.id, durable_generation).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        map_role_mutation_policy_error, parse_user_role, parse_user_status, status_change_requested,
    };
    use rustok_auth::AuthAdminMutationError;
    use rustok_core::{UserRole, UserStatus};
    use rustok_rbac::RbacRoleMutationPolicyError;

    #[test]
    fn parses_admin_user_enums_case_insensitively() {
        assert_eq!(parse_user_role("  ADMIN ").unwrap(), UserRole::Admin);
        assert_eq!(parse_user_status("  BANNED ").unwrap(), UserStatus::Banned);
    }

    #[test]
    fn maps_owner_policy_failures_to_stable_admin_categories() {
        assert!(matches!(
            map_role_mutation_policy_error(
                RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole
            ),
            AuthAdminMutationError::Forbidden(_)
        ));
        assert!(matches!(
            map_role_mutation_policy_error(RbacRoleMutationPolicyError::LastActiveSuperAdmin),
            AuthAdminMutationError::Conflict(_)
        ));
    }

    #[test]
    fn status_effective_change_ignores_exact_replay() {
        assert!(!status_change_requested(
            &UserStatus::Active,
            Some(&UserStatus::Active),
        ));
        assert!(!status_change_requested(&UserStatus::Active, None));
        assert!(status_change_requested(
            &UserStatus::Active,
            Some(&UserStatus::Banned),
        ));
    }
}
