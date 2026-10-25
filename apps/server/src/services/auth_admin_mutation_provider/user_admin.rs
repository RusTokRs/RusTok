use async_trait::async_trait;
use chrono::Utc;
use rustok_api::Permission;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, IssueUserInviteCommand,
    IssuedUserInviteResult, UpdateUserCommand, UserAdminMutationPort, UserInviteRecord,
    UserMutationRecord,
};
use rustok_core::events::EventTransport;
use rustok_core::{UserRole, UserStatus, infer_user_role_from_permissions};
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_rbac::{
    RbacCustomRoleAssignmentPlan, RbacCustomRoleAssignmentRequest, RbacRoleMutationOutcome,
    RbacRoleMutationPlan, RbacRoleMutationPolicyError, RbacUserRoleMutationRequest,
    plan_persisted_custom_role_assignment_on, plan_persisted_user_role_mutation_on,
    replace_persisted_user_role_by_slug_on, validate_custom_role_grant_on,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use std::{str::FromStr, sync::Arc};
use uuid::Uuid;

use crate::auth::hash_password_async;
use sha2::{Digest, Sha256};

use crate::models::{auth_invites, sessions, users};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::flex_attached_values::FlexAttachedValuesService;
use crate::services::rbac_cache_invalidation::publish_user_rbac_invalidation;
use crate::services::rbac_invalidation_generation::reserve_rbac_invalidation_generation;
use crate::services::rbac_request_scope::{permissions_for, role_for};
use crate::services::rbac_service::RbacService;

use super::{
    ServerAuthAdminMutationProvider, super_admin_guard::ensure_active_super_admin_continuity,
};

/// Invite tokens stay valid for a week: long enough for out-of-band
/// delivery by an administrator, short enough to bound exposure.
const INVITE_TOKEN_TTL_SECS: u64 = 7 * 24 * 60 * 60;

fn invite_record(invite: auth_invites::Model) -> UserInviteRecord {
    UserInviteRecord {
        id: invite.id,
        email: invite.email,
        role: invite.role,
        issued_by: invite.issued_by,
        expires_at: invite.expires_at.with_timezone(&Utc),
        revoked_at: invite.revoked_at.map(|value| value.with_timezone(&Utc)),
        created_at: invite.created_at.with_timezone(&Utc),
    }
}

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

/// A validated role change inside `update_user`: canonical built-in roles go
/// through the rank-based owner plan, tenant-persisted custom roles through
/// the permission-ceiling owner plan.
enum RolePlan {
    Builtin(RbacRoleMutationPlan),
    Custom(RbacCustomRoleAssignmentPlan),
}

impl RolePlan {
    fn tenant_id(&self) -> Uuid {
        match self {
            RolePlan::Builtin(plan) => plan.tenant_id(),
            RolePlan::Custom(plan) => plan.tenant_id(),
        }
    }

    fn actor_id(&self) -> Uuid {
        match self {
            RolePlan::Builtin(plan) => plan.actor_id(),
            RolePlan::Custom(plan) => plan.actor_id(),
        }
    }

    fn integration_event(
        &self,
        durable_generation: u64,
    ) -> Result<rustok_events::RbacRoleMutationEvent, AuthAdminMutationError> {
        match self {
            RolePlan::Builtin(plan) => plan
                .integration_event(durable_generation)
                .map_err(map_role_mutation_policy_error),
            RolePlan::Custom(plan) => plan
                .integration_event(durable_generation)
                .map_err(map_role_mutation_policy_error),
        }
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

fn ensure_custom_field_snapshot_is_current(
    initial_metadata: &serde_json::Value,
    locked_metadata: &serde_json::Value,
    custom_fields_requested: bool,
) -> Result<(), AuthAdminMutationError> {
    if custom_fields_requested && initial_metadata != locked_metadata {
        return Err(AuthAdminMutationError::Conflict(
            "user custom fields changed concurrently; retry the update".to_string(),
        ));
    }
    Ok(())
}

fn map_lifecycle_error(error: AuthLifecycleError) -> AuthAdminMutationError {
    match error {
        AuthLifecycleError::EmailAlreadyExists => {
            AuthAdminMutationError::Conflict("user email already exists".to_string())
        }
        AuthLifecycleError::Validation(message) => AuthAdminMutationError::Validation(message),
        other => super::internal_admin_error(crate::error::Error::from(other)),
    }
}

fn map_auth_input_error(error: rustok_auth::AuthError) -> AuthAdminMutationError {
    match error {
        rustok_auth::AuthError::Validation(message) => AuthAdminMutationError::Validation(message),
        other => super::internal_admin_error(other),
    }
}

fn map_custom_field_error(error: rustok_core::field_schema::FlexError) -> AuthAdminMutationError {
    match error {
        rustok_core::field_schema::FlexError::ValidationFailed(errors) => {
            AuthAdminMutationError::CustomFieldsValidation(
                serde_json::to_value(errors).unwrap_or_else(|_| serde_json::json!([])),
            )
        }
        other => super::internal_admin_error(other),
    }
}

/// Resolve an admin-supplied role string to a built-in role or a
/// tenant-defined custom role slug layered over the baseline customer
/// membership. A malformed slug is a validation error rather than a
/// silent downgrade to customer.
fn parse_requested_role(
    raw_role: &str,
) -> Result<(UserRole, Option<String>), AuthAdminMutationError> {
    match parse_user_role(raw_role) {
        Ok(role) => Ok((role, None)),
        Err(_) => {
            let slug = raw_role.trim().to_lowercase();
            let valid = (2..=64).contains(&slug.len())
                && slug
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            if !valid {
                return Err(AuthAdminMutationError::Validation(format!(
                    "unsupported user role: {raw_role}"
                )));
            }
            Ok((UserRole::Customer, Some(slug)))
        }
    }
}

pub(super) fn map_role_mutation_policy_error(
    error: RbacRoleMutationPolicyError,
) -> AuthAdminMutationError {
    match error {
        RbacRoleMutationPolicyError::CannotAssignPeerOrHigherRole
        | RbacRoleMutationPolicyError::RequestAuthorityCeiling { .. }
        | RbacRoleMutationPolicyError::CustomRequestAuthorityCeiling { .. }
        | RbacRoleMutationPolicyError::BuiltInRoleRequiresCanonicalPath(_)
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
            super::internal_admin_error(error)
        }
    }
}

impl ServerAuthAdminMutationProvider {
    /// Publish an admin audit event inside the caller's transaction
    /// (transactional outbox: the event commits exactly with the
    /// mutation it describes; a publish failure drops the transaction
    /// and rolls the mutation back).
    async fn publish_admin_event_in_tx(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        context: &AuthAdminMutationContext,
        event: DomainEvent,
    ) -> Result<(), AuthAdminMutationError> {
        let event_bus = TransactionalEventBus::new(
            Arc::new(OutboxTransport::new(self.db.clone())) as Arc<dyn EventTransport>,
        );
        event_bus
            .publish_in_tx(tx, context.tenant_id, Some(context.actor_id), event)
            .await
            .map_err(super::internal_admin_error)
    }

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
            .map_err(super::internal_admin_error)?;
        Ok(infer_user_role_from_permissions(&permissions))
    }

    async fn ensure_role_assignment_allowed(
        &self,
        context: &AuthAdminMutationContext,
        requested_role: &UserRole,
    ) -> Result<(), AuthAdminMutationError> {
        let actor_role = self.actor_role(context).await?;
        rustok_rbac::require_role_assignment(&actor_role, requested_role)
            .map_err(map_role_mutation_policy_error)
    }

    async fn ensure_target_management_allowed(
        &self,
        context: &AuthAdminMutationContext,
        target_user_id: Uuid,
        target_role: &UserRole,
    ) -> Result<(), AuthAdminMutationError> {
        let actor_role = self.actor_role(context).await?;
        rustok_rbac::require_user_management(
            context.actor_id,
            target_user_id,
            &actor_role,
            target_role,
        )
        .map_err(map_role_mutation_policy_error)
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
            .map_err(super::internal_admin_error)?,
        DbBackend::Sqlite => {
            let user = query().one(db).await.map_err(super::internal_admin_error)?;
            if let Some(user) = user.as_ref() {
                let result = users::Entity::update_many()
                    .col_expr(
                        users::Column::UpdatedAt,
                        Expr::col(users::Column::UpdatedAt),
                    )
                    .filter(users::Column::Id.eq(user.id))
                    .filter(users::Column::TenantId.eq(tenant_id))
                    .exec(db)
                    .await
                    .map_err(super::internal_admin_error)?;
                if result.rows_affected != 1 {
                    return Err(AuthAdminMutationError::Internal(
                        "user mutation lock fence could not be acquired".to_string(),
                    ));
                }

                return query()
                    .one(db)
                    .await
                    .map_err(super::internal_admin_error)?
                    .ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()));
            }
            None
        }
        _ => {
            return Err(AuthAdminMutationError::Internal(
                "unsupported SeaORM database backend".to_string(),
            ));
        }
    };

    user.ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))
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
        .map_err(super::internal_admin_error)?;
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

        // Custom roles are layered onto the baseline customer membership
        // inside the creation transaction below.
        let (role, custom_slug) =
            parse_requested_role(command.role.as_deref().unwrap_or("customer"))?;
        let status = command
            .status
            .as_deref()
            .map(parse_user_status)
            .transpose()?;

        if custom_slug.is_some()
            || role != UserRole::Customer
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
        // Authorization is fully decided above; only now spend Argon2
        // work, still before opening the transaction.
        let prepared_user = AuthLifecycleService::prepare_new_user(
            &command.email,
            &command.password,
            command.name,
            rustok_auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
        )
        .await
        .map_err(map_lifecycle_error)?;
        let tx = self.db.begin().await.map_err(super::internal_admin_error)?;
        let mut user = AuthLifecycleService::create_user_in_tx(
            &tx,
            context.tenant_id,
            prepared_user,
            role.clone(),
            status.clone(),
        )
        .await
        .map_err(map_lifecycle_error)?;

        if let Some(metadata) = prepared.metadata {
            let mut active: users::ActiveModel = user.into();
            active.metadata = Set(metadata);
            user = active
                .update(&tx)
                .await
                .map_err(super::internal_admin_error)?;
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
        let final_role = if let Some(slug) = custom_slug.as_ref() {
            let actor_role = self.actor_role(context).await?;
            let actor_permissions =
                permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
                    AuthAdminMutationError::Forbidden(
                        "user administration requires a request-bound permission snapshot"
                            .to_string(),
                    )
                })?;
            let plan = plan_persisted_custom_role_assignment_on(
                &tx,
                RbacCustomRoleAssignmentRequest {
                    tenant_id: context.tenant_id,
                    actor_id: context.actor_id,
                    actor_tenant_id: context.tenant_id,
                    actor_role,
                    actor_permissions,
                    target_user_id: user.id,
                    requested_role_slug: slug.clone(),
                    resulting_status: status.clone(),
                },
            )
            .await
            .map_err(super::super_admin_guard::persistence_error)?;
            if plan.is_some() {
                replace_persisted_user_role_by_slug_on(
                    &tx,
                    context.tenant_id,
                    user.id,
                    slug,
                )
                .await
                .map_err(super::super_admin_guard::persistence_error)?;
            }
            self.user_role(&tx, context.tenant_id, user.id).await?
        } else {
            role
        };
        let tenant_name = self.tenant_name(&tx, context.tenant_id).await?;
        tx.commit().await.map_err(super::internal_admin_error)?;
        Ok(ServerAuthAdminMutationProvider::user_record(
            user,
            final_role,
            tenant_name,
        ))
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
            .map_err(super::internal_admin_error)?
            .ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?;

        if command.role.is_some() || command.status.is_some() {
            self.authorize_user(
                context,
                &[Permission::USERS_MANAGE],
                "users:manage required to change user role or status",
            )
            .await?;
        }

        let normalized_email = match command.email.as_deref() {
            Some(email) => {
                Some(rustok_auth::normalize_email(email).map_err(map_auth_input_error)?)
            }
            None => None,
        };
        let normalized_name = match command.name.as_deref() {
            Some(name) => Some(
                rustok_auth::normalize_display_name(Some(name)).map_err(map_auth_input_error)?,
            ),
            None => None,
        };

        if let Some(email) = normalized_email.as_deref() {
            let existing = users::Entity::find_by_email(&self.db, context.tenant_id, email)
                .await
                .map_err(super::internal_admin_error)?;
            if existing
                .as_ref()
                .is_some_and(|existing| existing.id != initial_user.id)
            {
                return Err(AuthAdminMutationError::Conflict(
                    "user email already exists".to_string(),
                ));
            }
        }

        let requested_role = command
            .role
            .as_deref()
            .map(parse_user_role)
            .transpose()
            .ok()
            .flatten();
        let requested_custom_slug = match (command.role.as_deref(), requested_role.as_ref()) {
            (Some(raw), None) => {
                let slug = raw.trim().to_lowercase();
                let valid = (2..=64).contains(&slug.len())
                    && slug
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
                if !valid {
                    return Err(AuthAdminMutationError::Validation(format!(
                        "unsupported user role: {raw}"
                    )));
                }
                Some(slug)
            }
            _ => None,
        };
        let requested_status = command
            .status
            .as_deref()
            .map(parse_user_status)
            .transpose()?;

        let locale = context
            .locale
            .as_deref()
            .unwrap_or(rustok_api::PLATFORM_FALLBACK_LOCALE);
        let custom_fields_updated = command.custom_fields.is_some();
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
        // Validate and hash a replacement password before locking the user
        // row: Argon2 is deliberately expensive and must not run while the
        // transaction holds the row lock.
        let new_password_hash = match command.password.as_deref() {
            Some(password) => {
                rustok_auth::validate_password_policy(
                    password,
                    rustok_auth::DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
                )
                .map_err(map_auth_input_error)?;
                Some(
                    hash_password_async(password.to_string())
                        .await
                        .map_err(super::internal_admin_error)?,
                )
            }
            None => None,
        };

        let tx = self.db.begin().await.map_err(super::internal_admin_error)?;
        let locked_user = lock_user_for_mutation(&tx, context.tenant_id, command.id).await?;
        let current_role = self
            .user_role(&tx, context.tenant_id, locked_user.id)
            .await?;
        ensure_custom_field_snapshot_is_current(
            &initial_user.metadata,
            &locked_user.metadata,
            custom_fields_updated,
        )?;
        let user_id = locked_user.id;
        let target_status = locked_user.status.clone();
        let status_changed = status_change_requested(&target_status, requested_status.as_ref());

        let role_mutation_plan: Option<RolePlan> =
            if let Some(requested_role) = requested_role.as_ref() {
                let actor_role = self.actor_role(context).await?;
                match plan_persisted_user_role_mutation_on(
                    &tx,
                    RbacUserRoleMutationRequest {
                        tenant_id: context.tenant_id,
                        actor_id: context.actor_id,
                        actor_tenant_id: context.tenant_id,
                        actor_role,
                        target_user_id: user_id,
                        requested_role: requested_role.clone(),
                        resulting_status: requested_status.clone(),
                    },
                )
                .await
                .map_err(super::super_admin_guard::persistence_error)?
                {
                    RbacRoleMutationOutcome::Noop => None,
                    RbacRoleMutationOutcome::Apply(plan) => Some(RolePlan::Builtin(plan)),
                }
            } else if let Some(slug) = requested_custom_slug.as_ref() {
                let actor_role = self.actor_role(context).await?;
                let actor_permissions =
                    permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
                        AuthAdminMutationError::Forbidden(
                            "user administration requires a request-bound permission snapshot"
                                .to_string(),
                        )
                    })?;
                plan_persisted_custom_role_assignment_on(
                    &tx,
                    RbacCustomRoleAssignmentRequest {
                        tenant_id: context.tenant_id,
                        actor_id: context.actor_id,
                        actor_tenant_id: context.tenant_id,
                        actor_role,
                        actor_permissions,
                        target_user_id: user_id,
                        requested_role_slug: slug.clone(),
                        resulting_status: requested_status.clone(),
                    },
                )
                .await
                .map_err(super::super_admin_guard::persistence_error)?
                .map(RolePlan::Custom)
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
        if let Some(email) = normalized_email {
            if email != locked_user.email {
                // A different address has not proven mailbox control; never
                // carry the previous address's verification over.
                active.email_verified_at = Set(None);
            }
            active.email = Set(email);
        }
        if let Some(name) = normalized_name {
            active.name = Set(name);
        }
        if status_changed && let Some(status) = requested_status.as_ref() {
            active.status = Set(status.clone());
        }
        if let Some(password_hash) = new_password_hash {
            active.password_hash = Set(password_hash);
        }
        if let Some(metadata) = prepared.metadata {
            active.metadata = Set(metadata);
        }

        if requested_role.is_none() && requested_custom_slug.is_none() {
            let effective_requested_status = if status_changed {
                requested_status.as_ref()
            } else {
                None
            };
            ensure_active_super_admin_continuity(
                &tx,
                context.tenant_id,
                user_id,
                None,
                effective_requested_status,
                false,
            )
            .await?;
        }
        let user = if user_row_update_requested {
            active.update(&tx).await.map_err(|error| {
                // The uniqueness pre-check above runs outside this
                // transaction; a concurrent claim of the same email loses
                // here and must surface as Conflict, not Internal.
                if matches!(
                    error.sql_err(),
                    Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
                ) {
                    AuthAdminMutationError::Conflict("user email already exists".to_string())
                } else {
                    super::internal_admin_error(error)
                }
            })?
        } else {
            locked_user
        };
        match role_mutation_plan.as_ref() {
            Some(RolePlan::Builtin(plan)) => {
                RbacService::replace_user_role_in_transaction(
                    &tx,
                    &user.id,
                    &context.tenant_id,
                    plan.new_role().clone(),
                )
                .await
                .map_err(super::internal_admin_error)?;
            }
            Some(RolePlan::Custom(plan)) => {
                replace_persisted_user_role_by_slug_on(
                    &tx,
                    context.tenant_id,
                    user.id,
                    plan.role_slug(),
                )
                .await
                .map_err(super::super_admin_guard::persistence_error)?;
            }
            None => {}
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
                    .map_err(super::internal_admin_error)?,
            )
        } else {
            None
        };

        let event_bus = TransactionalEventBus::new(
            Arc::new(OutboxTransport::new(self.db.clone())) as Arc<dyn EventTransport>
        );

        if let Some(plan) = role_mutation_plan.as_ref() {
            let generation = durable_generation.ok_or_else(|| {
                AuthAdminMutationError::Internal(
                    "RBAC role mutation did not reserve a durable generation".to_string(),
                )
            })?;
            let event = plan.integration_event(generation)?;
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

        if (user_row_update_requested || role_mutation_plan.is_some())
            && let Err(error) = event_bus
                .publish_in_tx(
                    &tx,
                    context.tenant_id,
                    Some(context.actor_id),
                    DomainEvent::UserUpdated { user_id: user.id },
                )
                .await
        {
            let rollback_error = tx.rollback().await.err();
            tracing::error!(
                %error,
                ?rollback_error,
                tenant_id = %context.tenant_id,
                actor_id = %context.actor_id,
                user_id = %user.id,
                "Durable UserUpdated publication failed; user update rolled back"
            );
            return Err(AuthAdminMutationError::Internal(
                "durable UserUpdated event is unavailable".to_string(),
            ));
        }

        let tenant_name = self.tenant_name(&tx, context.tenant_id).await?;
        let final_role = match role_mutation_plan.as_ref() {
            Some(RolePlan::Builtin(plan)) => plan.new_role().clone(),
            Some(RolePlan::Custom(_)) => {
                self.user_role(&tx, context.tenant_id, user.id).await?
            }
            None => current_role.clone(),
        };
        tx.commit().await.map_err(super::internal_admin_error)?;
        if let Some(durable_generation) = durable_generation {
            publish_committed_user_invalidation(context.tenant_id, user.id, durable_generation)
                .await;
        }
        Ok(ServerAuthAdminMutationProvider::user_record(
            user,
            final_role,
            tenant_name,
        ))
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
        let tx = self.db.begin().await.map_err(super::internal_admin_error)?;
        let user = lock_user_for_mutation(&tx, context.tenant_id, user_id).await?;
        let current_role = self.user_role(&tx, context.tenant_id, user.id).await?;
        self.ensure_target_management_allowed(context, user.id, &current_role)
            .await?;
        ensure_active_super_admin_continuity(&tx, context.tenant_id, user.id, None, None, true)
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
        .map_err(super::internal_admin_error)?;
        revoke_active_sessions(&tx, context.tenant_id, user.id).await?;
        let durable_generation = reserve_rbac_invalidation_generation(&tx)
            .await
            .map_err(super::internal_admin_error)?;
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
        tx.commit().await.map_err(super::internal_admin_error)?;
        publish_committed_user_invalidation(context.tenant_id, user.id, durable_generation).await;
        Ok(())
    }

    async fn issue_user_invite(
        &self,
        context: &AuthAdminMutationContext,
        command: IssueUserInviteCommand,
    ) -> Result<IssuedUserInviteResult, AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_CREATE, Permission::USERS_MANAGE],
            "users:create or users:manage required",
        )
        .await?;

        // Built-in roles ride in the token directly; a tenant-defined
        // custom role is carried as a slug layered over the baseline
        // customer membership at acceptance (same model as create).
        let (role, custom_slug) =
            parse_requested_role(command.role.as_deref().unwrap_or("customer"))?;
        if custom_slug.is_some() || role != UserRole::Customer {
            self.authorize_user(
                context,
                &[Permission::USERS_MANAGE],
                "users:manage required to invite privileged users",
            )
            .await?;
        }
        if role != UserRole::Customer {
            self.ensure_role_assignment_allowed(context, &role).await?;
        }

        let email = rustok_auth::normalize_email(&command.email)
            .map_err(|error| AuthAdminMutationError::Validation(error.to_string()))?;

        let token = crate::auth::encode_invite_token(
            &self.auth_config,
            context.tenant_id,
            &email,
            role.clone(),
            custom_slug.clone(),
            INVITE_TOKEN_TTL_SECS,
        )
        .map_err(super::internal_admin_error)?;

        let now = Utc::now();
        let tx = self.db.begin().await.map_err(super::internal_admin_error)?;

        // The custom-role ceiling is validated at issuance under the
        // issuer's current authority: the role must exist in this
        // tenant and the issuer must effectively hold every permission
        // it would hand out when the invite is accepted.
        let ledger_role = if let Some(slug) = custom_slug.as_deref() {
            let actor_role = self.actor_role(context).await?;
            let actor_permissions =
                permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
                    AuthAdminMutationError::Forbidden(
                        "user administration requires a request-bound permission snapshot"
                            .to_string(),
                    )
                })?;
            validate_custom_role_grant_on(
                &tx,
                context.tenant_id,
                &actor_role,
                &actor_permissions,
                slug,
            )
            .await
            .map_err(super::super_admin_guard::persistence_error)?
        } else {
            role.to_string()
        };

        let invite = auth_invites::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(context.tenant_id),
            token_hash: Set(hex::encode(Sha256::digest(token.as_bytes()))),
            email: Set(email),
            role: Set(ledger_role),
            issued_by: Set(Some(context.actor_id)),
            expires_at: Set(
                (now + chrono::Duration::seconds(INVITE_TOKEN_TTL_SECS as i64)).into(),
            ),
            revoked_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&tx)
        .await
        .map_err(super::internal_admin_error)?;

        self.publish_admin_event_in_tx(
            &tx,
            context,
            DomainEvent::UserInviteIssued {
                invite_id: invite.id,
                issued_by: Some(context.actor_id),
            },
        )
        .await?;
        tx.commit().await.map_err(super::internal_admin_error)?;

        Ok(IssuedUserInviteResult {
            invite: invite_record(invite),
            token,
        })
    }

    async fn list_user_invites(
        &self,
        context: &AuthAdminMutationContext,
        limit: u64,
    ) -> Result<Vec<UserInviteRecord>, AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_LIST, Permission::USERS_MANAGE],
            "users:list or users:manage required",
        )
        .await?;

        let invites = auth_invites::Entity::find()
            .filter(auth_invites::Column::TenantId.eq(context.tenant_id))
            .order_by_desc(auth_invites::Column::CreatedAt)
            .limit(super::clamp_auth_admin_list_limit(limit))
            .all(&self.db)
            .await
            .map_err(super::internal_admin_error)?;

        Ok(invites.into_iter().map(invite_record).collect())
    }

    async fn revoke_user_invite(
        &self,
        context: &AuthAdminMutationContext,
        invite_id: Uuid,
    ) -> Result<UserInviteRecord, AuthAdminMutationError> {
        self.authorize_user(
            context,
            &[Permission::USERS_MANAGE],
            "users:manage required",
        )
        .await?;

        let invite = auth_invites::Entity::find_by_id(invite_id)
            .filter(auth_invites::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(super::internal_admin_error)?
            .ok_or_else(|| AuthAdminMutationError::NotFound("user invite".to_string()))?;

        if invite.revoked_at.is_some() {
            return Ok(invite_record(invite));
        }

        let tx = self.db.begin().await.map_err(super::internal_admin_error)?;
        let mut active: auth_invites::ActiveModel = invite.into();
        active.revoked_at = Set(Some(Utc::now().into()));
        let updated = active
            .update(&tx)
            .await
            .map_err(super::internal_admin_error)?;

        self.publish_admin_event_in_tx(
            &tx,
            context,
            DomainEvent::UserInviteRevoked {
                invite_id: updated.id,
                revoked_by: Some(context.actor_id),
            },
        )
        .await?;
        tx.commit().await.map_err(super::internal_admin_error)?;

        Ok(invite_record(updated))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_custom_field_snapshot_is_current, map_role_mutation_policy_error, parse_user_role,
        parse_user_status, status_change_requested,
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
    fn custom_field_updates_reject_stale_metadata_snapshots() {
        let initial = serde_json::json!({"tier": "gold"});
        let changed = serde_json::json!({"tier": "silver"});

        assert!(ensure_custom_field_snapshot_is_current(&initial, &initial, true).is_ok());
        assert!(ensure_custom_field_snapshot_is_current(&initial, &changed, false).is_ok());
        assert!(matches!(
            ensure_custom_field_snapshot_is_current(&initial, &changed, true),
            Err(AuthAdminMutationError::Conflict(message))
                if message == "user custom fields changed concurrently; retry the update"
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
