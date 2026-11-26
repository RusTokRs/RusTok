use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::Permission;
use rustok_cache::CacheService;
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, IssueUserInviteCommand,
    IssuedUserInviteResult, UpdateUserCommand, UserAdminMutationPort, UserInviteRecord,
    UserMutationRecord,
};
use rustok_core::{UserRole, UserStatus};
use rustok_events::DomainEvent;
use rustok_rbac::{
    RbacCustomRoleAssignmentPlan, RbacCustomRoleAssignmentRequest, RbacRoleMutationOutcome,
    RbacRoleMutationPlan, RbacRoleMutationPolicyError, RbacUserRoleMutationRequest,
    plan_persisted_custom_role_assignment_on, plan_persisted_user_role_mutation_on,
    replace_persisted_user_role_by_slug_on,
    validate_persisted_role_grant_on,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use std::str::FromStr;
use uuid::Uuid;

use crate::auth::{AuthConfig, hash_password_async};
use crate::models::{auth_email_change_requests, auth_invite_consumptions, auth_invites, users};
use crate::services::auth_invite::{
    InviteLookup, invite_token_hash, issuer_may_issue_invite, lock_invite_for_mutation,
    validate_invite_role_grant_on,
};
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

fn invite_expiration_from_token(
    config: &AuthConfig,
    token: &str,
) -> Result<DateTime<Utc>, AuthAdminMutationError> {
    let claims =
        crate::auth::decode_invite_token(config, token).map_err(super::internal_admin_error)?;
    let timestamp = i64::try_from(claims.exp).map_err(|_| {
        AuthAdminMutationError::Internal("invite token expiration is out of range".to_string())
    })?;

    DateTime::<Utc>::from_timestamp(timestamp, 0).ok_or_else(|| {
        AuthAdminMutationError::Internal("invite token expiration is invalid".to_string())
    })
}

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

fn email_identity_changed(
    current_email: &str,
    requested_email: Option<&str>,
) -> bool {
    requested_email.is_some_and(|requested_email| requested_email != current_email)
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

fn user_row_update_requested(
    email_requested: bool,
    name_requested: bool,
    status_changed: bool,
    password_requested: bool,
    prepared_metadata: bool,
    prepared_localized_values: bool,
) -> bool {
    email_requested
        || name_requested
        || status_changed
        || password_requested
        || prepared_metadata
        || prepared_localized_values
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

fn validate_admin_password(
    config: &AuthConfig,
    password: &str,
) -> Result<(), AuthAdminMutationError> {
    rustok_auth::validate_password_policy(password, config.min_password_length)
        .map_err(map_auth_input_error)
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
        let event_bus = &self.event_bus;
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
        rustok_rbac::resolve_role_hierarchy_on(db, tenant_id, user_id)
            .await
            .map_err(super::super_admin_guard::persistence_error)
    }

    async fn user_role_slug<C>(
        &self,
        db: &C,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<String, AuthAdminMutationError>
    where
        C: ConnectionTrait,
    {
        rustok_rbac::resolve_persisted_role_slug_on(db, tenant_id, user_id)
            .await
            .map_err(super::super_admin_guard::persistence_error)
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
    AuthLifecycleService::lock_user_for_mutation(db, tenant_id, user_id)
        .await
        .map_err(super::internal_admin_error)?
        .ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))
}


async fn revoke_active_sessions<C>(
    db: &C,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<(), AuthAdminMutationError>
where
    C: ConnectionTrait,
{
    let now = Utc::now();
    AuthLifecycleService::revoke_user_sessions_db_with_connection(
        db, tenant_id, user_id, None,
    )
    .await
    .map_err(map_lifecycle_error)?;
    let _revoked_oauth_tokens =
        crate::services::oauth_token_service::revoke_user_delegated_tokens_in_tx(
            db, tenant_id, user_id, now,
        )
        .await
        .map_err(super::internal_admin_error)?;
    Ok(())
}

async fn publish_committed_user_invalidation(
    cache_service: &CacheService,
    tenant_id: Uuid,
    user_id: Uuid,
    durable_generation: u64,
) {
    RbacService::invalidate_user_rbac_caches(&tenant_id, &user_id).await;
    if let Err(error) =
        publish_user_rbac_invalidation(cache_service, &tenant_id, &user_id, durable_generation).await
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

        validate_admin_password(&self.auth_config, &command.password)?;
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
            self.auth_config.min_password_length,
        )
        .await
        .map_err(map_lifecycle_error)?;
        let tx = self.db.begin().await.map_err(super::internal_admin_error)?;
        if custom_slug.is_none() {
            let actor_permissions = permissions_for(&context.tenant_id, &context.actor_id)
                .ok_or_else(|| {
                    AuthAdminMutationError::Forbidden(
                        "user administration requires a request-bound permission snapshot"
                            .to_string(),
                    )
                })?;
            validate_persisted_role_grant_on(
                &tx,
                context.tenant_id,
                &actor_permissions,
                &role.to_string(),
            )
            .await
            .map_err(super::super_admin_guard::persistence_error)?;
        }
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
            let actor_permissions = permissions_for(&context.tenant_id, &context.actor_id)
                .ok_or_else(|| {
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
                replace_persisted_user_role_by_slug_on(&tx, context.tenant_id, user.id, slug)
                    .await
                    .map_err(super::super_admin_guard::persistence_error)?;
            }
            self.user_role_slug(&tx, context.tenant_id, user.id).await?
        } else {
            role.to_string()
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
            Some(email) => Some(rustok_auth::normalize_email(email).map_err(map_auth_input_error)?),
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
        let password_changed = command.password.is_some();
        // Validate and hash a replacement password before locking the user
        // row: Argon2 is deliberately expensive and must not run while the
        // transaction holds the row lock.
        let new_password_hash = match command.password.as_deref() {
            Some(password) => {
                validate_admin_password(&self.auth_config, password)?;
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
        let email_changed = email_identity_changed(
            &locked_user.email,
            normalized_email.as_deref(),
        );
        let current_role = self
            .user_role(&tx, context.tenant_id, locked_user.id)
            .await?;
        ensure_custom_field_snapshot_is_current(
            &initial_user.metadata,
            &locked_user.metadata,
            custom_fields_updated,
        )?;
        let prepared = FlexAttachedValuesService::prepare_user_update(
            &tx,
            context.tenant_id,
            locked_user.id,
            locale,
            &locked_user.metadata,
            command.custom_fields,
        )
        .await
        .map_err(map_custom_field_error)?;
        let user_id = locked_user.id;
        let target_status = locked_user.status.clone();
        let status_changed = status_change_requested(&target_status, requested_status.as_ref());

        let role_mutation_plan: Option<RolePlan> =
            if let Some(requested_role) = requested_role.as_ref() {
                let actor_role = self.actor_role(context).await?;
                let actor_permissions = permissions_for(&context.tenant_id, &context.actor_id)
                    .ok_or_else(|| {
                        AuthAdminMutationError::Forbidden(
                            "user administration requires a request-bound permission snapshot"
                                .to_string(),
                        )
                    })?;
                validate_persisted_role_grant_on(
                    &tx,
                    context.tenant_id,
                    &actor_permissions,
                    &requested_role.to_string(),
                )
                .await
                .map_err(super::super_admin_guard::persistence_error)?;
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
                let actor_permissions = permissions_for(&context.tenant_id, &context.actor_id)
                    .ok_or_else(|| {
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

        let user_row_update_requested = user_row_update_requested(
            command.email.is_some(),
            command.name.is_some(),
            status_changed,
            password_changed,
            prepared.metadata.is_some(),
            prepared.localized_values.is_some(),
        );
        let mut active: users::ActiveModel = locked_user.clone().into();
        if user_row_update_requested {
            active.updated_at = Set(Utc::now().into());
        }
        if email_changed {
            // A different address has not proven mailbox control; never carry
            // the previous address's verification or existing sessions over.
            active.email_verified_at = Set(None);
        }
        if let Some(email) = normalized_email {
            active.email = Set(email);
        }
        if let Some(name) = normalized_name {
            active.name = Set(name);
        }
        if status_changed && let Some(status) = requested_status.as_ref() {
            active.status = Set(status.clone());
        }
        if let Some(password_hash) = new_password_hash {
            active.set_replacement_password(password_hash, Utc::now());
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
        // A stale refresh token must not become usable again after a new
        // mailbox is verified. Revoke browser sessions and delegated OAuth
        // credentials in the same transaction as the identity change.
        if password_changed || status_disables_user || email_changed {
            revoke_active_sessions(&tx, context.tenant_id, user.id).await?;
            auth_email_change_requests::Entity::delete_many()
                .filter(auth_email_change_requests::Column::TenantId.eq(context.tenant_id))
                .filter(auth_email_change_requests::Column::UserId.eq(user.id))
                .exec(&tx)
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

        let event_bus = &self.event_bus;

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
            Some(RolePlan::Builtin(plan)) => plan.new_role().to_string(),
            Some(RolePlan::Custom(_)) | None => {
                self.user_role_slug(&tx, context.tenant_id, user.id).await?
            }
        };
        tx.commit().await.map_err(super::internal_admin_error)?;
        if let Some(durable_generation) = durable_generation {
            publish_committed_user_invalidation(&self.cache_service, context.tenant_id, user.id, durable_generation)
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

        let event_bus = &self.event_bus;
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
        auth_email_change_requests::Entity::delete_many()
            .filter(auth_email_change_requests::Column::TenantId.eq(context.tenant_id))
            .filter(auth_email_change_requests::Column::UserId.eq(user.id))
            .exec(&tx)
            .await
            .map_err(super::internal_admin_error)?;
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
        publish_committed_user_invalidation(&self.cache_service, context.tenant_id, user.id, durable_generation).await;
        Ok(())
    }

    async fn issue_user_invite(
        &self,
        context: &AuthAdminMutationContext,
        command: IssueUserInviteCommand,
    ) -> Result<IssuedUserInviteResult, AuthAdminMutationError> {
        // Preserve the authenticated request's ceiling even if persisted
        // authority has expanded since this request was admitted.
        let request_permissions = self.request_permissions(context)?;
        let request_role = self.actor_role(context).await?;
        let (role, custom_slug) =
            parse_requested_role(command.role.as_deref().unwrap_or("customer"))?;
        if !issuer_may_issue_invite(&request_permissions, &role, custom_slug.is_some()) {
            return Err(AuthAdminMutationError::Forbidden(
                "current invitation permission is required".to_string(),
            ));
        }

        let email = rustok_auth::normalize_email(&command.email)
            .map_err(|error| AuthAdminMutationError::Validation(error.to_string()))?;

        rustok_auth::application_clock::admit_now(&self.db).await
            .map_err(super::internal_admin_error)?;
        let tx = self.db.begin().await.map_err(super::internal_admin_error)?;
        // Membership and account-status writers serialize on this user
        // fence. Do not issue a durable invitation for a removed, disabled
        // or demoted actor based solely on a stale request snapshot.
        let issuer = AuthLifecycleService::lock_user_for_mutation(
            &tx, context.tenant_id, context.actor_id,
        )
        .await
        .map_err(super::internal_admin_error)?;
        if !issuer.is_some_and(|user| user.is_active()) {
            return Err(AuthAdminMutationError::Forbidden(
                "active invitation issuer is required".to_string(),
            ));
        }
        let persisted_permissions = RbacService::get_user_permissions_authoritative(
            &tx, &context.tenant_id, &context.actor_id,
        )
        .await
        .map_err(super::internal_admin_error)?;
        let persisted_role = RbacService::get_user_role_authoritative(
            &tx, &context.tenant_id, &context.actor_id,
        )
        .await
        .map_err(super::internal_admin_error)?;
        if !issuer_may_issue_invite(&persisted_permissions, &role, custom_slug.is_some()) {
            return Err(AuthAdminMutationError::Forbidden(
                "current invitation permission is required".to_string(),
            ));
        }

        // Both views must independently authorize the exact target grants.
        // A newly granted permission must not expand an older request and
        // an older request must not retain a permission already removed.
        validate_invite_role_grant_on(
            &tx, context.tenant_id, &request_role, &request_permissions,
            &role, custom_slug.as_deref(),
        )
        .await
        .map_err(super::super_admin_guard::persistence_error)?;
        let ledger_role = validate_invite_role_grant_on(
            &tx, context.tenant_id, &persisted_role, &persisted_permissions,
            &role, custom_slug.as_deref(),
        )
        .await
        .map_err(super::super_admin_guard::persistence_error)?;

        // Start the signed lifetime after account and delegation checks.
        let now = rustok_auth::application_clock::verified_now(&tx).await
            .map_err(super::internal_admin_error)?;
        let token = crate::auth::encode_invite_token(
            &self.auth_config,
            context.tenant_id,
            &email,
            role.clone(),
            custom_slug.clone(),
            INVITE_TOKEN_TTL_SECS,
        )
        .map_err(super::internal_admin_error)?;
        let expires_at = invite_expiration_from_token(&self.auth_config, &token)?;

        let invite = auth_invites::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(context.tenant_id),
            token_hash: Set(invite_token_hash(&token)),
            email: Set(email),
            role: Set(ledger_role),
            issued_by: Set(Some(context.actor_id)),
            expires_at: Set(expires_at.into()),
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
        let final_now = rustok_auth::application_clock::verified_now(&tx).await
            .map_err(super::internal_admin_error)?;
        if final_now >= expires_at {
            tx.rollback().await.map_err(super::internal_admin_error)?;
            return Err(AuthAdminMutationError::Validation("Invite expired before issuance".to_string()));
        }
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

        // Acceptance uses the same row fence. Whichever operation acquires
        // it first determines the outcome: a committed revocation prevents
        // acceptance, while an already accepted invite cannot be "revoked"
        // retroactively or emit a misleading second lifecycle event.
        let tx = self.db.begin().await.map_err(super::internal_admin_error)?;
        let invite = lock_invite_for_mutation(
            &tx,
            context.tenant_id,
            InviteLookup::Id(invite_id),
        )
        .await
        .map_err(super::internal_admin_error)?
        .ok_or_else(|| AuthAdminMutationError::NotFound("user invite".to_string()))?;

        if invite.revoked_at.is_some() {
            tx.commit().await.map_err(super::internal_admin_error)?;
            return Ok(invite_record(invite));
        }

        let consumed = auth_invite_consumptions::Entity::find()
            .filter(auth_invite_consumptions::Column::TenantId.eq(context.tenant_id))
            .filter(auth_invite_consumptions::Column::TokenHash.eq(&invite.token_hash))
            .one(&tx)
            .await
            .map_err(super::internal_admin_error)?;
        if consumed.is_some() {
            tx.rollback().await.map_err(super::internal_admin_error)?;
            return Err(AuthAdminMutationError::Conflict(
                "user invite has already been accepted".to_string(),
            ));
        }

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
        ensure_custom_field_snapshot_is_current, invite_expiration_from_token,
        map_role_mutation_policy_error, parse_user_role, parse_user_status,
        email_identity_changed, status_change_requested, INVITE_TOKEN_TTL_SECS,
    };
    use rustok_auth::AuthAdminMutationError;
    use rustok_core::{UserRole, UserStatus};
    use rustok_rbac::RbacRoleMutationPolicyError;

    #[tokio::test]
    async fn stale_invite_request_authority_cannot_issue_for_missing_inactive_or_ungranted_actor() {
        use crate::models::{auth_invites, tenants, users};
        use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use rustok_api::Permission;
        use rustok_auth::{AuthAdminMutationContext, IssueUserInviteCommand, UserAdminMutationPort};
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};
        use uuid::Uuid;

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "invite-stale-authority")
            .insert(&db).await.expect("tenant");
        let mut inactive = users::ActiveModel::new(tenant.id, "inactive@example.test", "hash");
        inactive.status = Set(UserStatus::Inactive);
        let inactive = inactive.insert(&db).await.expect("inactive actor");
        let active = users::ActiveModel::new(tenant.id, "active@example.test", "hash")
            .insert(&db).await.expect("actor without grants");
        let runtime = ServerRuntimeContext::new(db.clone(), Default::default());
        let provider = super::ServerAuthAdminMutationProvider::new(
            &runtime,
            crate::auth::test_auth_config("aB3!zY7@qW8#eR2$".repeat(4)),
        );

        for actor_id in [Uuid::new_v4(), inactive.id, active.id] {
            let context = AuthAdminMutationContext {
                actor_id,
                tenant_id: tenant.id,
                request_id: None,
                locale: Some("en".to_string()),
                principal_kind: Some(rustok_api::AuthPrincipalKind::DirectUser),
            };
            let scope = RbacRequestScope::new(
                tenant.id, actor_id, vec![Permission::USERS_MANAGE], UserRole::Admin,
            );
            let result = with_rbac_request_scope(Some(scope), async {
                provider.issue_user_invite(&context, IssueUserInviteCommand {
                    email: "invitee@example.test".to_string(),
                    role: Some("customer".to_string()),
                }).await
            }).await;
            assert!(matches!(result, Err(AuthAdminMutationError::Forbidden(_))));
        }
        assert!(auth_invites::Entity::find().all(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn invite_revocation_rolls_back_when_runtime_event_transport_fails() {
        use crate::models::{auth_invites, tenants};
        use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use rustok_api::Permission;
        use rustok_auth::{AuthAdminMutationContext, UserAdminMutationPort};
        use rustok_core::events::{EventTransport, ReliabilityLevel};
        use rustok_events::EventEnvelope;
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        use uuid::Uuid;

        #[derive(Default)]
        struct FailingTransport(AtomicUsize);

        #[async_trait::async_trait]
        impl EventTransport for FailingTransport {
            async fn publish(&self, _event: EventEnvelope) -> rustok_core::Result<()> {
                panic!("admin events must remain transactional");
            }

            fn supports_transactional_writes(&self) -> bool {
                true
            }

            async fn write_root_in_transaction(
                &self,
                _transaction: &sea_orm::DatabaseTransaction,
                _event: EventEnvelope,
            ) -> rustok_core::Result<()> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err(rustok_core::Error::Validation("injected outbox failure".to_string()))
            }

            fn reliability_level(&self) -> ReliabilityLevel {
                ReliabilityLevel::Outbox
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Admin event tenant", "admin-event-tenant")
            .insert(&db)
            .await
            .expect("tenant");
        let actor_id = Uuid::new_v4();
        let invite_id = Uuid::new_v4();
        let now = chrono::Utc::now();
        auth_invites::ActiveModel {
            id: Set(invite_id),
            tenant_id: Set(tenant.id),
            token_hash: Set("a".repeat(64)),
            email: Set("invitee@example.test".to_string()),
            role: Set("customer".to_string()),
            issued_by: Set(None),
            expires_at: Set((now + chrono::Duration::hours(1)).into()),
            revoked_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("invite");
        let runtime = ServerRuntimeContext::new(db.clone(), Default::default());
        let failing = Arc::new(FailingTransport::default());
        runtime.shared_insert(failing.clone() as Arc<dyn EventTransport>);
        let provider = super::ServerAuthAdminMutationProvider::new(
            &runtime,
            crate::auth::test_auth_config("aB3!zY7@qW8#eR2$".repeat(4)),
        );
        let context = AuthAdminMutationContext {
            actor_id,
            tenant_id: tenant.id,
            request_id: None,
            locale: Some("en".to_string()),
            principal_kind: Some(rustok_api::AuthPrincipalKind::DirectUser),
        };
        let scope = RbacRequestScope::new(
            tenant.id, actor_id, vec![Permission::USERS_MANAGE], UserRole::Admin,
        );

        let result = with_rbac_request_scope(Some(scope), async {
            provider.revoke_user_invite(&context, invite_id).await
        })
        .await;
        assert!(matches!(result, Err(AuthAdminMutationError::Internal(_))));
        assert_eq!(failing.0.load(Ordering::SeqCst), 1);
        let persisted = auth_invites::Entity::find_by_id(invite_id)
            .one(&db)
            .await
            .expect("read invite")
            .expect("invite retained");
        assert!(persisted.revoked_at.is_none());
    }

    #[test]
    fn invite_ledger_expiration_matches_signed_jwt_second_precision() {
        let config = crate::auth::test_auth_config("aB3!zY7@qW8#eR2$".repeat(4));
        let token = crate::auth::encode_invite_token(
            &config,
            uuid::Uuid::new_v4(),
            "invitee@example.test",
            UserRole::Customer,
            None,
            INVITE_TOKEN_TTL_SECS,
        )
        .expect("encode invite token");
        let claims =
            crate::auth::decode_invite_token(&config, &token).expect("decode issued invite token");

        let ledger_expiry =
            invite_expiration_from_token(&config, &token).expect("resolve signed expiry");

        assert_eq!(ledger_expiry.timestamp(), claims.exp as i64);
        assert_eq!(ledger_expiry.timestamp_subsec_nanos(), 0);
    }

    #[test]
    fn administrative_password_policy_honors_configured_minimum() {
        let config = crate::auth::test_auth_config("aB3!zY7@qW8#eR2$".repeat(4))
            .with_min_password_length(20);
        assert!(matches!(
            super::validate_admin_password(&config, "ValidPass123!"),
            Err(AuthAdminMutationError::Validation(_))
        ));
        assert!(super::validate_admin_password(&config, "ValidLongPassword123!").is_ok());
    }

    #[test]
    fn email_identity_change_is_detected() {
        assert!(email_identity_changed(
            "old@example.test",
            Some("new@example.test"),
        ));
        assert!(!email_identity_changed(
            "new@example.test",
            Some("new@example.test"),
        ));
        assert!(!email_identity_changed(
            "new@example.test",
            None,
        ));
    }

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
    fn localized_custom_field_only_updates_emit_user_mutation() {
        assert!(super::user_row_update_requested(
            false, false, false, false, false, true
        ));
        assert!(super::user_row_update_requested(
            false, false, false, false, true, false
        ));
        assert!(!super::user_row_update_requested(
            false, false, false, false, false, false
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
