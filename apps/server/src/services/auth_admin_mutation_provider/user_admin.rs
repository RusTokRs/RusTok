use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{AuthPrincipalKind, Permission};
use rustok_auth::{
    AuthAdminMutationContext, AuthAdminMutationError, CreateUserCommand, IssueUserInviteCommand,
    IssuedUserInviteResult, UpdateUserCommand, UserAdminMutationPort, UserInviteRecord,
    UserMutationRecord,
};
use rustok_cache::CacheService;
use rustok_core::{UserRole, UserStatus};
use rustok_events::DomainEvent;
use rustok_rbac::{
    RbacCustomRoleAssignmentPlan, RbacCustomRoleAssignmentRequest, RbacRoleMutationOutcome,
    RbacRoleMutationPlan, RbacRoleMutationPolicyError, RbacUserRoleMutationRequest,
    plan_persisted_custom_role_assignment_on, plan_persisted_user_role_mutation_on,
    replace_persisted_user_role_by_slug_on, validate_custom_role_grant_on,
    validate_persisted_role_grant_on,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, Statement, TransactionTrait,
};
use std::str::FromStr;
use uuid::Uuid;

use crate::auth::{AuthConfig, hash_password_async};
use crate::models::{
    auth_email_change_requests, auth_invite_consumptions, auth_invites, oauth_apps, oauth_tokens,
    tenants, users,
};
use crate::services::auth_invite::{
    InviteLookup, invite_token_hash, issuer_may_issue_invite, lock_invite_for_mutation,
    validate_invite_role_grant_on,
};
use crate::services::auth_lifecycle::{AuthLifecycleError, AuthLifecycleService};
use crate::services::flex_attached_values::FlexAttachedValuesService;
use crate::services::rbac_cache_invalidation::publish_user_rbac_invalidation;
use crate::services::rbac_invalidation_generation::reserve_rbac_invalidation_generation;
use crate::services::rbac_request_scope::{
    OAuthCredentialEvidence, credential_for_admin, permissions_for, role_for,
};
use crate::services::rbac_service::RbacService;

use super::{
    ServerAuthAdminMutationProvider, super_admin_guard::ensure_active_super_admin_continuity,
};

pub(super) fn map_oauth_credential_error(
    (status, message): (axum::http::StatusCode, &'static str),
) -> AuthAdminMutationError {
    if status.is_server_error() {
        super::internal_admin_error(message)
    } else {
        AuthAdminMutationError::Unauthorized
    }
}

/// Restrict newly resolved grants to the exact admitted OAuth scopes as well
/// as requiring the independent original request permission ceiling.
pub(super) fn scope_live_admin_permissions(
    context: &AuthAdminMutationContext,
    permissions: Vec<Permission>,
) -> Result<Vec<Permission>, AuthAdminMutationError> {
    match credential_for_admin(context)? {
        Some(evidence) if evidence.principal_kind != AuthPrincipalKind::DirectUser => {
            let delegated = evidence.oauth.ok_or(AuthAdminMutationError::Unauthorized)?;
            Ok(rustok_api::context::restrict_permissions_to_scopes(
                &permissions,
                &delegated.scopes,
            ))
        }
        _ => Ok(permissions),
    }
}

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

fn email_identity_changed(current_email: &str, requested_email: Option<&str>) -> bool {
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
        credential_for_admin(context)?;
        role_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            AuthAdminMutationError::Forbidden(
                "user administration requires a request-bound role snapshot".to_string(),
            )
        })
    }

    /// Shared OAuth mutation fence. Tenant first (manifest reconciliation),
    /// then all participating app rows in UUID order, before any user row.
    /// SQLite reserves its writer before reading any protected state.
    pub(super) async fn lock_admin_oauth_apps(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        context: &AuthAdminMutationContext,
        target_id: Option<Uuid>,
    ) -> Result<(Option<oauth_apps::Model>, Option<oauth_apps::Model>), AuthAdminMutationError>
    {
        let evidence = credential_for_admin(context)?;
        if evidence.as_ref().is_some_and(|e| {
            context
                .principal_kind
                .is_some_and(|kind| kind != e.principal_kind)
        }) {
            return Err(AuthAdminMutationError::Unauthorized);
        }
        let oauth = match evidence.as_ref() {
            Some(e) if e.principal_kind != AuthPrincipalKind::DirectUser => Some(
                e.oauth
                    .as_ref()
                    .ok_or(AuthAdminMutationError::Unauthorized)?,
            ),
            _ => None,
        };
        match tx.get_database_backend() {
            DbBackend::Postgres => {
                let tenant = tx
                    .query_one_raw(Statement::from_sql_and_values(
                        DbBackend::Postgres,
                        // KEY SHARE protects only identity/FKs, not a change to is_active.
                        "SELECT id FROM tenants WHERE id = $1 AND is_active = TRUE FOR SHARE",
                        [context.tenant_id.into()],
                    ))
                    .await
                    .map_err(super::internal_admin_error)?;
                if tenant.is_none() {
                    return Err(AuthAdminMutationError::Unauthorized);
                }
            }
            DbBackend::Sqlite => {
                let locked = tenants::Entity::update_many()
                    .col_expr(
                        tenants::Column::UpdatedAt,
                        sea_orm::sea_query::Expr::col(tenants::Column::UpdatedAt),
                    )
                    .filter(tenants::Column::Id.eq(context.tenant_id))
                    .filter(tenants::Column::IsActive.eq(true))
                    .exec(tx)
                    .await
                    .map_err(super::internal_admin_error)?;
                if locked.rows_affected != 1 {
                    return Err(AuthAdminMutationError::Unauthorized);
                }
            }
            _ => {
                return Err(AuthAdminMutationError::Internal(
                    "Unsupported credential transaction backend".to_string(),
                ));
            }
        }
        let mut ids = Vec::new();
        if let Some(id) = target_id {
            ids.push(id);
        }
        if let Some(oauth) = oauth {
            ids.push(oauth.app_id);
        }
        ids.sort_unstable();
        ids.dedup();
        let mut target = None;
        let mut issuer = None;
        for id in ids {
            let app =
                crate::services::oauth_app::lock_oauth_app_for_mutation(tx, id, context.tenant_id)
                    .await
                    .map_err(super::internal_admin_error)?;
            if oauth.is_some_and(|e| e.app_id == id) {
                let e = oauth.ok_or(AuthAdminMutationError::Unauthorized)?;
                let grant = if evidence
                    .as_ref()
                    .is_some_and(|e| e.principal_kind == AuthPrincipalKind::Service)
                {
                    "client_credentials"
                } else {
                    "authorization_code"
                };
                let app = app
                    .as_ref()
                    .filter(|app| {
                        app.is_active()
                            && app.client_id == e.client_id
                            && app.supports_grant_type(grant)
                    })
                    .ok_or(AuthAdminMutationError::Unauthorized)?;
                crate::extractors::auth::validate_oauth_token_scopes(app, &e.scopes)
                    .map_err(map_oauth_credential_error)?;
                issuer = Some(app.clone());
            }
            if target_id == Some(id) {
                target =
                    Some(app.ok_or_else(|| {
                        AuthAdminMutationError::NotFound("oauth app".to_string())
                    })?);
            }
        }
        Ok((target, issuer))
    }

    /// The app and actor fences are already held. Check the exact access
    /// registration under a row lock; SQLite already holds its writer fence.
    /// All canonical app/consent changes revoke old registrations in their
    /// own transaction. Keep both fences until the administrative commit.
    pub(super) async fn lock_admin_oauth_registration(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        context: &AuthAdminMutationContext,
        actor: Option<&users::Model>,
        app: &oauth_apps::Model,
        delegated: &OAuthCredentialEvidence,
    ) -> Result<DateTime<Utc>, AuthAdminMutationError> {
        let query = oauth_tokens::Entity::find_by_id(delegated.registration_id)
            .filter(oauth_tokens::Column::TenantId.eq(context.tenant_id))
            .filter(oauth_tokens::Column::AppId.eq(app.id));
        let row = match tx.get_database_backend() {
            DbBackend::Postgres => query.lock_exclusive().one(tx).await,
            DbBackend::Sqlite => query.one(tx).await,
            _ => {
                return Err(AuthAdminMutationError::Internal(
                    "Unsupported credential transaction backend".to_string(),
                ));
            }
        }
        .map_err(super::internal_admin_error)?
        .ok_or(AuthAdminMutationError::Unauthorized)?;
        let scopes = row
            .decoded_scopes()
            .map_err(|_| AuthAdminMutationError::Unauthorized)?;
        let (expected_subject, expected_grant) = match actor {
            Some(actor) => (Some(actor.id), "authorization_code"),
            None if app.id == context.actor_id => (None, "client_credentials"),
            None => return Err(AuthAdminMutationError::Unauthorized),
        };
        if app.id != delegated.app_id
            || app.client_id != delegated.client_id
            || row.user_id != expected_subject
            || row.token_hash != delegated.token_hash
            || row.grant_type != expected_grant
            || row.token_kind != oauth_tokens::TOKEN_KIND_ACCESS
            || row.revoked_at.is_some()
            || row.created_at.with_timezone(&Utc) != delegated.issued_at
            || row.consent_granted_at != delegated.consent_granted_at
            || scopes != delegated.scopes
            || actor
                .is_some_and(|actor| actor.rejects_delegated_token_issued_at(delegated.issued_at))
            || (actor.is_none() && row.consent_granted_at.is_some())
        {
            return Err(AuthAdminMutationError::Unauthorized);
        }
        if let Some(actor) = actor {
            crate::extractors::auth::validate_active_user_consent(
                tx,
                app,
                context.tenant_id,
                actor.id,
                &delegated.scopes,
                row.consent_granted_at,
            )
            .await
            .map_err(map_oauth_credential_error)?;
        }
        let now = self.user_admin_credential_now(tx).await?;
        let expires = row.expires_at.with_timezone(&Utc);
        if now < delegated.issued_at || now >= expires {
            return Err(AuthAdminMutationError::Unauthorized);
        }
        Ok(expires)
    }

    /// The actor user row is already locked. Acquire the direct session fence
    /// in the same user -> session order as refresh and recovery, or validate
    /// the delegated registration under the earlier app fence.
    pub(super) async fn lock_user_admin_credential(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        context: &AuthAdminMutationContext,
        actor: &users::Model,
        delegated_app: Option<&oauth_apps::Model>,
    ) -> Result<Option<DateTime<Utc>>, AuthAdminMutationError> {
        let evidence = credential_for_admin(context)?;
        if context.principal_kind == Some(AuthPrincipalKind::Service) {
            return Err(AuthAdminMutationError::Forbidden(
                "user administration requires a user actor".to_string(),
            ));
        }
        // Only an explicit owner SQL test fixture may lack bearer evidence.
        // The production credential_for_admin boundary rejects that variant.
        let Some(evidence) = evidence else {
            return Ok(None);
        };
        if evidence.principal_kind == AuthPrincipalKind::Service
            || context
                .principal_kind
                .is_some_and(|kind| kind != evidence.principal_kind)
            || actor.id != context.actor_id
            || actor.tenant_id != context.tenant_id
            || actor.recovery_revision < 0
            || actor.recovery_revision != evidence.recovery_revision
        {
            return Err(AuthAdminMutationError::Unauthorized);
        }
        let mut deadline = evidence.expires_at;
        if evidence.principal_kind == AuthPrincipalKind::DirectUser {
            let session = AuthLifecycleService::find_session_by_id_for_update_in_tx(
                tx,
                context.tenant_id,
                evidence.session_id,
            )
            .await
            .map_err(super::internal_admin_error)?
            .filter(|session| session.user_id == context.actor_id)
            .ok_or(AuthAdminMutationError::Unauthorized)?;
            deadline = deadline
                .min(session.expires_at.with_timezone(&Utc))
                .min(session.absolute_deadline());
            let now = self.user_admin_credential_now(tx).await?;
            if !session.is_active_at(now) || now >= deadline {
                return Err(AuthAdminMutationError::Unauthorized);
            }
        } else {
            let app = delegated_app.ok_or(AuthAdminMutationError::Unauthorized)?;
            let delegated = evidence
                .oauth
                .as_ref()
                .ok_or(AuthAdminMutationError::Unauthorized)?;
            deadline = deadline.min(
                self.lock_admin_oauth_registration(tx, context, Some(actor), app, delegated)
                    .await?,
            );
            self.ensure_user_admin_credential_deadline(tx, Some(deadline))
                .await?;
        }
        Ok(Some(deadline))
    }

    async fn user_admin_credential_now(
        &self,
        tx: &sea_orm::DatabaseTransaction,
    ) -> Result<DateTime<Utc>, AuthAdminMutationError> {
        let verified = rustok_auth::application_clock::verified_now(tx)
            .await
            .map_err(super::internal_admin_error)?;
        let live = self
            .auth_config
            .credential_now()
            .map_err(super::internal_admin_error)?;
        let decision = rustok_auth::application_clock::decision_time(verified, live)
            .map_err(super::internal_admin_error)?;
        // A later live sample may deny admission, never extend the deadline.
        Ok(decision.max(live))
    }

    pub(super) async fn ensure_user_admin_credential_deadline(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        deadline: Option<DateTime<Utc>>,
    ) -> Result<(), AuthAdminMutationError> {
        if let Some(deadline) = deadline {
            if self.user_admin_credential_now(tx).await? >= deadline {
                return Err(AuthAdminMutationError::Unauthorized);
            }
        }
        Ok(())
    }

    /// Lock the active tenant and any delegated app first, then all existing users
    /// before module and role state. Request evidence
    /// remains a ceiling; the independently resolved live actor must also
    /// authorize the operation. The target is returned from its locked read.
    async fn lock_user_admin_authority(
        &self,
        tx: &sea_orm::DatabaseTransaction,
        context: &AuthAdminMutationContext,
        target_id: Option<Uuid>,
        requested_builtin: Option<&UserRole>,
        requested_slug: Option<&str>,
        required: &[Permission],
    ) -> Result<
        (
            Option<users::Model>,
            UserRole,
            Vec<Permission>,
            Option<DateTime<Utc>>,
        ),
        AuthAdminMutationError,
    > {
        // A service principal's subject is an app ID, never a persisted user
        // administrator, even if an unrelated user happens to share its UUID.
        if context.principal_kind == Some(rustok_api::AuthPrincipalKind::Service) {
            return Err(AuthAdminMutationError::Forbidden(
                "user administration requires a user actor".to_string(),
            ));
        }
        let delegated_app = self.lock_admin_oauth_apps(tx, context, None).await?.1;
        let mut ids = vec![context.actor_id];
        if let Some(target) = target_id {
            ids.push(target);
        }
        ids.sort_unstable();
        ids.dedup();
        let mut target = None;
        let mut actor = None;
        for id in ids {
            let locked = AuthLifecycleService::lock_user_for_mutation(tx, context.tenant_id, id)
                .await
                .map_err(super::internal_admin_error)?;
            if id == context.actor_id {
                actor = Some(
                    locked
                        .as_ref()
                        .filter(|actor| actor.is_active())
                        .cloned()
                        .ok_or_else(|| {
                            AuthAdminMutationError::Forbidden(
                                "active user administrator is required".to_string(),
                            )
                        })?,
                );
            }
            if target_id == Some(id) {
                target = Some(
                    locked.ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?,
                );
            }
        }

        let actor = actor.ok_or(AuthAdminMutationError::Unauthorized)?;
        let credential_deadline = self
            .lock_user_admin_credential(tx, context, &actor, delegated_app.as_ref())
            .await?;

        let (_, availability) =
            crate::services::rbac_module_availability::lock_invite_module_policy(
                &self.db,
                tx,
                self.invite_registry.as_ref(),
                context.tenant_id,
            )
            .await
            .map_err(super::internal_admin_error)?;
        // A missing built-in destination must be materialized before checking
        // its exact grants; never authorize against a seed that loses an
        // insert race to a differently configured persisted role.
        if let Some(role) = requested_builtin {
            rustok_rbac::RbacRoleAssignmentDbWriter::ensure_builtin_role_definition_on(
                tx,
                context.tenant_id,
                role,
            )
            .await
            .map_err(super::internal_admin_error)?;
        }
        if !rustok_rbac::lock_user_admin_role_grants_on(
            tx,
            context.tenant_id,
            context.actor_id,
            target_id,
            requested_slug,
        )
        .await
        .map_err(super::internal_admin_error)?
        {
            return Err(AuthAdminMutationError::NotFound(
                "requested role".to_string(),
            ));
        }
        let mut permissions = RbacService::get_user_permissions_authoritative(
            tx,
            &context.tenant_id,
            &context.actor_id,
        )
        .await
        .map_err(super::internal_admin_error)?;
        if let Some(availability) = availability {
            permissions = availability.active_permissions(&permissions);
        }
        let permissions = scope_live_admin_permissions(context, permissions)?;
        if !required.iter().any(|permission| {
            rustok_rbac::has_effective_permission_in_set(&permissions, permission)
        }) {
            return Err(AuthAdminMutationError::Forbidden(
                "current user administration permission is required".to_string(),
            ));
        }
        let role =
            RbacService::get_user_role_authoritative(tx, &context.tenant_id, &context.actor_id)
                .await
                .map_err(super::internal_admin_error)?;
        self.ensure_user_admin_credential_deadline(tx, credential_deadline)
            .await?;
        Ok((target, role, permissions, credential_deadline))
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
    AuthLifecycleService::revoke_user_sessions_db_with_connection(db, tenant_id, user_id, None)
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
        publish_user_rbac_invalidation(cache_service, &tenant_id, &user_id, durable_generation)
            .await
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

        // A custom role is a direct creation target, not a replacement
        // of a temporary Customer membership.
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
        // Request admission precedes Argon2; persisted grant checks run
        // inside the creation transaction, after hashing.
        let prepared_user = AuthLifecycleService::prepare_new_user(
            &command.email,
            &command.password,
            command.name,
            self.auth_config.min_password_length,
        )
        .await
        .map_err(map_lifecycle_error)?;
        let actor_permissions = self.request_permissions(context)?;
        let request_role = self.actor_role(context).await?;
        let privileged = custom_slug.is_some()
            || role != UserRole::Customer
            || status
                .as_ref()
                .is_some_and(|value| value != &UserStatus::Active);
        let required: &[Permission] = if privileged {
            &[Permission::USERS_MANAGE]
        } else {
            &[Permission::USERS_CREATE, Permission::USERS_MANAGE]
        };
        let tx = self.prepare_admin_transaction(true).await?;
        let target_slug = custom_slug.clone().unwrap_or_else(|| role.to_string());
        let (_, current_role, current_permissions, credential_deadline) = self
            .lock_user_admin_authority(
                &tx,
                context,
                None,
                if custom_slug.is_none() {
                    Some(&role)
                } else {
                    None
                },
                Some(&target_slug),
                required,
            )
            .await?;
        let mut user = if let Some(slug) = custom_slug.as_deref() {
            validate_custom_role_grant_on(
                &tx,
                context.tenant_id,
                &request_role,
                &actor_permissions,
                slug,
            )
            .await
            .map_err(super::super_admin_guard::persistence_error)?;
            let target_slug = validate_custom_role_grant_on(
                &tx,
                context.tenant_id,
                &current_role,
                &current_permissions,
                slug,
            )
            .await
            .map_err(super::super_admin_guard::persistence_error)?;
            AuthLifecycleService::create_user_with_custom_role_in_tx(
                &tx,
                context.tenant_id,
                prepared_user,
                &target_slug,
                status.clone(),
            )
            .await
            .map_err(map_lifecycle_error)?
        } else {
            if role != UserRole::Customer {
                rustok_rbac::require_role_assignment(&current_role, &role)
                    .map_err(map_role_mutation_policy_error)?;
            }
            for ceiling in [&actor_permissions, &current_permissions] {
                validate_persisted_role_grant_on(&tx, context.tenant_id, ceiling, &target_slug)
                    .await
                    .map_err(super::super_admin_guard::persistence_error)?;
            }
            AuthLifecycleService::create_user_in_tx(
                &tx,
                context.tenant_id,
                prepared_user,
                role.clone(),
                status.clone(),
            )
            .await
            .map_err(map_lifecycle_error)?
        };

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
        let final_role = if custom_slug.is_some() {
            self.user_role_slug(&tx, context.tenant_id, user.id).await?
        } else {
            role.to_string()
        };
        let tenant_name = self.tenant_name(&tx, context.tenant_id).await?;
        self.publish_admin_event_in_tx(
            &tx,
            context,
            DomainEvent::UserAccountRegistered { user_id: user.id },
        )
        .await?;
        self.ensure_user_admin_credential_deadline(&tx, credential_deadline)
            .await?;
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

        let request_role = self.actor_role(context).await?;
        let request_permissions = self.request_permissions(context)?;
        let required: &[Permission] = if command.role.is_some() || command.status.is_some() {
            &[Permission::USERS_MANAGE]
        } else {
            &[Permission::USERS_UPDATE, Permission::USERS_MANAGE]
        };
        let requested_slug = requested_custom_slug
            .clone()
            .or_else(|| requested_role.as_ref().map(ToString::to_string));
        let tx = self.prepare_admin_transaction(true).await?;
        let (locked_user, live_actor_role, live_permissions, credential_deadline) = self
            .lock_user_admin_authority(
                &tx,
                context,
                Some(command.id),
                requested_role.as_ref(),
                requested_slug.as_deref(),
                required,
            )
            .await?;
        let locked_user =
            locked_user.ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?;
        let email_changed = email_identity_changed(&locked_user.email, normalized_email.as_deref());
        let current_role = self
            .user_role(&tx, context.tenant_id, locked_user.id)
            .await?;
        // Both the original request ceiling and the live hierarchy must
        // authorize the exact target, including profile-only updates.
        for actor_role in [&request_role, &live_actor_role] {
            rustok_rbac::require_user_management(
                context.actor_id,
                locked_user.id,
                actor_role,
                &current_role,
            )
            .map_err(map_role_mutation_policy_error)?;
        }
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
                rustok_rbac::require_role_assignment(&request_role, requested_role)
                    .map_err(map_role_mutation_policy_error)?;
                for ceiling in [&request_permissions, &live_permissions] {
                    validate_persisted_role_grant_on(
                        &tx,
                        context.tenant_id,
                        ceiling,
                        &requested_role.to_string(),
                    )
                    .await
                    .map_err(super::super_admin_guard::persistence_error)?;
                }
                match plan_persisted_user_role_mutation_on(
                    &tx,
                    RbacUserRoleMutationRequest {
                        tenant_id: context.tenant_id,
                        actor_id: context.actor_id,
                        actor_tenant_id: context.tenant_id,
                        actor_role: live_actor_role.clone(),
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
                validate_custom_role_grant_on(
                    &tx,
                    context.tenant_id,
                    &request_role,
                    &request_permissions,
                    slug,
                )
                .await
                .map_err(super::super_admin_guard::persistence_error)?;
                plan_persisted_custom_role_assignment_on(
                    &tx,
                    RbacCustomRoleAssignmentRequest {
                        tenant_id: context.tenant_id,
                        actor_id: context.actor_id,
                        actor_tenant_id: context.tenant_id,
                        actor_role: live_actor_role,
                        actor_permissions: live_permissions,
                        target_user_id: user_id,
                        requested_role_slug: slug.clone(),
                        resulting_status: requested_status.clone(),
                    },
                )
                .await
                .map_err(super::super_admin_guard::persistence_error)?
                .map(RolePlan::Custom)
            } else {
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
        self.ensure_user_admin_credential_deadline(&tx, credential_deadline)
            .await?;
        tx.commit().await.map_err(super::internal_admin_error)?;
        if let Some(durable_generation) = durable_generation {
            publish_committed_user_invalidation(
                &self.cache_service,
                context.tenant_id,
                user.id,
                durable_generation,
            )
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

        let request_role = self.actor_role(context).await?;
        let event_bus = &self.event_bus;
        let tx = self.prepare_admin_transaction(true).await?;
        let (user, live_actor_role, _, credential_deadline) = self
            .lock_user_admin_authority(
                &tx,
                context,
                Some(user_id),
                None,
                None,
                &[Permission::USERS_MANAGE],
            )
            .await?;
        let user = user.ok_or_else(|| AuthAdminMutationError::NotFound("user".to_string()))?;
        let current_role = self.user_role(&tx, context.tenant_id, user.id).await?;
        for actor_role in [&request_role, &live_actor_role] {
            rustok_rbac::require_user_management(
                context.actor_id,
                user.id,
                actor_role,
                &current_role,
            )
            .map_err(map_role_mutation_policy_error)?;
        }
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
        self.ensure_user_admin_credential_deadline(&tx, credential_deadline)
            .await?;
        tx.commit().await.map_err(super::internal_admin_error)?;
        publish_committed_user_invalidation(
            &self.cache_service,
            context.tenant_id,
            user.id,
            durable_generation,
        )
        .await;
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

        let tx = self.prepare_admin_transaction(true).await?;
        let delegated_app = self.lock_admin_oauth_apps(&tx, context, None).await?.1;
        // Membership and account-status writers serialize on this user
        // fence. Do not issue a durable invitation for a removed, disabled
        // or demoted actor based solely on a stale request snapshot.
        let issuer =
            AuthLifecycleService::lock_user_for_mutation(&tx, context.tenant_id, context.actor_id)
                .await
                .map_err(super::internal_admin_error)?
                .filter(|user| user.is_active() && user.recovery_revision >= 0)
                .ok_or_else(|| {
                    AuthAdminMutationError::Forbidden(
                        "active invitation issuer is required".to_string(),
                    )
                })?;
        let credential_deadline = self
            .lock_user_admin_credential(&tx, context, &issuer, delegated_app.as_ref())
            .await?;
        let (module_policy_binding, availability) =
            crate::services::rbac_module_availability::lock_invite_module_policy(
                &self.db,
                &tx,
                self.invite_registry.as_ref(),
                context.tenant_id,
            )
            .await
            .map_err(super::internal_admin_error)?;
        let membership_revision =
            rustok_rbac::read_user_membership_revision_on(&tx, context.tenant_id, context.actor_id)
                .await
                .map_err(super::internal_admin_error)?;
        if custom_slug.is_none() {
            rustok_rbac::RbacRoleAssignmentDbWriter::ensure_builtin_role_definition_on(
                &tx,
                context.tenant_id,
                &role,
            )
            .await
            .map_err(super::internal_admin_error)?;
        }
        let target_slug = custom_slug.clone().unwrap_or_else(|| role.to_string());
        let role_binding = rustok_rbac::lock_invite_role_grant_binding_on(
            &tx,
            context.tenant_id,
            context.actor_id,
            &target_slug,
        )
        .await
        .map_err(super::internal_admin_error)?
        .ok_or_else(|| AuthAdminMutationError::NotFound("invitation role".to_string()))?;
        let mut persisted_permissions = RbacService::get_user_permissions_authoritative(
            &tx,
            &context.tenant_id,
            &context.actor_id,
        )
        .await
        .map_err(super::internal_admin_error)?;
        if let Some(availability) = availability {
            persisted_permissions = availability.active_permissions(&persisted_permissions);
        }
        let persisted_permissions = scope_live_admin_permissions(context, persisted_permissions)?;
        let persisted_role =
            RbacService::get_user_role_authoritative(&tx, &context.tenant_id, &context.actor_id)
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
            &tx,
            context.tenant_id,
            &request_role,
            &request_permissions,
            &role,
            custom_slug.as_deref(),
        )
        .await
        .map_err(super::super_admin_guard::persistence_error)?;
        let ledger_role = validate_invite_role_grant_on(
            &tx,
            context.tenant_id,
            &persisted_role,
            &persisted_permissions,
            &role,
            custom_slug.as_deref(),
        )
        .await
        .map_err(super::super_admin_guard::persistence_error)?;

        self.ensure_user_admin_credential_deadline(&tx, credential_deadline)
            .await?;
        // Start the signed lifetime after account and delegation checks.
        let now = rustok_auth::application_clock::verified_now(&tx)
            .await
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
            issuer_recovery_revision: Set(Some(issuer.recovery_revision)),
            issuer_membership_revision: Set(Some(membership_revision)),
            role_grant_binding: Set(Some(role_binding)),
            module_policy_binding: Set(Some(module_policy_binding)),
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
        let final_now = rustok_auth::application_clock::verified_now(&tx)
            .await
            .map_err(super::internal_admin_error)?;
        if final_now >= expires_at {
            tx.rollback().await.map_err(super::internal_admin_error)?;
            return Err(AuthAdminMutationError::Validation(
                "Invite expired before issuance".to_string(),
            ));
        }
        // Do not publish an invitation signed by a key that became unusable
        // while the ledger and outbox writes were waiting.
        invite_expiration_from_token(&self.auth_config, &token)?;
        self.ensure_user_admin_credential_deadline(&tx, credential_deadline)
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

        // Acquire the active tenant and any delegated OAuth app fence before
        // the acceptance-compatible actor -> invite -> policy -> roles order.
        // Revoking an invite does not require its original issuer or
        // destination role to remain valid.
        let tx = self.prepare_admin_transaction(true).await?;
        let delegated_app = self.lock_admin_oauth_apps(&tx, context, None).await?.1;
        let actor =
            AuthLifecycleService::lock_user_for_mutation(&tx, context.tenant_id, context.actor_id)
                .await
                .map_err(super::internal_admin_error)?
                .filter(|actor| actor.is_active())
                .ok_or_else(|| {
                    AuthAdminMutationError::Forbidden(
                        "active invitation administrator is required".to_string(),
                    )
                })?;
        let credential_deadline = self
            .lock_user_admin_credential(&tx, context, &actor, delegated_app.as_ref())
            .await?;
        let invite = lock_invite_for_mutation(&tx, context.tenant_id, InviteLookup::Id(invite_id))
            .await
            .map_err(super::internal_admin_error)?;

        // The request ceiling was checked above. Independently re-read
        // live persisted authority under the mutation fences, including
        // for idempotent retries and accepted-invite conflicts.
        let (_, availability) =
            crate::services::rbac_module_availability::lock_invite_module_policy(
                &self.db,
                &tx,
                self.invite_registry.as_ref(),
                context.tenant_id,
            )
            .await
            .map_err(super::internal_admin_error)?;
        rustok_rbac::lock_actor_role_grants_on(&tx, context.tenant_id, context.actor_id)
            .await
            .map_err(super::internal_admin_error)?;
        let mut permissions = RbacService::get_user_permissions_authoritative(
            &tx,
            &context.tenant_id,
            &context.actor_id,
        )
        .await
        .map_err(super::internal_admin_error)?;
        if let Some(availability) = availability {
            permissions = availability.active_permissions(&permissions);
        }
        let permissions = scope_live_admin_permissions(context, permissions)?;
        if !rustok_rbac::has_effective_permission_in_set(&permissions, &Permission::USERS_MANAGE) {
            return Err(AuthAdminMutationError::Forbidden(
                "current users:manage required".to_string(),
            ));
        }

        self.ensure_user_admin_credential_deadline(&tx, credential_deadline)
            .await?;

        // Do not disclose whether the invite exists to an actor whose
        // current permission was removed after request admission.
        let invite =
            invite.ok_or_else(|| AuthAdminMutationError::NotFound("user invite".to_string()))?;

        // The same ledger lock serializes with acceptance: a committed
        // revocation prevents acceptance, and an accepted invite cannot
        // emit a subsequent revocation event.
        if invite.revoked_at.is_some() {
            self.ensure_user_admin_credential_deadline(&tx, credential_deadline)
                .await?;
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
        self.ensure_user_admin_credential_deadline(&tx, credential_deadline)
            .await?;
        tx.commit().await.map_err(super::internal_admin_error)?;

        Ok(invite_record(updated))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        INVITE_TOKEN_TTL_SECS, email_identity_changed, ensure_custom_field_snapshot_is_current,
        invite_expiration_from_token, map_role_mutation_policy_error, parse_user_role,
        parse_user_status, status_change_requested,
    };
    use rustok_auth::AuthAdminMutationError;
    use rustok_core::{UserRole, UserStatus};
    use rustok_rbac::RbacRoleMutationPolicyError;

    #[tokio::test]
    async fn custom_role_provisioning_requires_live_actor_and_assigns_only_target() {
        use crate::models::_entities::{roles, user_roles};
        use crate::models::{tenants, users};
        use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use rustok_api::Permission;
        use rustok_auth::{AuthAdminMutationContext, CreateUserCommand, UserAdminMutationPort};
        use rustok_migrations::SqliteTestMigrator;
        use rustok_outbox::entity as sys_events;
        use rustok_test_utils::db::setup_test_db_with_migrations;
        use sea_orm::{
            ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait,
        };
        use uuid::Uuid;

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "custom-admin-provisioning")
            .insert(&db)
            .await
            .unwrap();
        let actor = users::ActiveModel::new(tenant.id, "admin@example.test", "hash")
            .insert(&db)
            .await
            .unwrap();
        let target_role = roles::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            name: Set("Content editor".to_string()),
            slug: Set("content_editor".to_string()),
            description: Set(None),
            is_system: Set(false),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        }
        .insert(&db)
        .await
        .unwrap();
        let runtime = ServerRuntimeContext::new(db.clone(), Default::default());
        let provider = super::ServerAuthAdminMutationProvider::new(
            &runtime,
            crate::auth::test_auth_config("aB3!zY7@qW8#eR2$".repeat(4)),
        );
        let context = AuthAdminMutationContext {
            actor_id: actor.id,
            tenant_id: tenant.id,
            request_id: None,
            locale: Some("en".to_string()),
            principal_kind: Some(rustok_api::AuthPrincipalKind::DirectUser),
            credential_authority: rustok_auth::CredentialAuthority::TrustedInternal,
        };
        let scope = || {
            RbacRequestScope::new(
                tenant.id,
                actor.id,
                vec![Permission::USERS_MANAGE],
                UserRole::Admin,
            )
        };
        let command = || CreateUserCommand {
            email: "editor@example.test".to_string(),
            password: "Password123!Strong".to_string(),
            name: None,
            role: Some(target_role.slug.clone()),
            status: None,
            custom_fields: None,
        };
        // A stale request with users:manage cannot replace persisted grants.
        let denied = with_rbac_request_scope(Some(scope()), async {
            provider.create_user(&context, command()).await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
        assert!(
            users::Entity::find_by_email(&db, tenant.id, "editor@example.test")
                .await
                .unwrap()
                .is_none()
        );

        let tx = db.begin().await.unwrap();
        rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
            &tx,
            tenant.id,
            actor.id,
            UserRole::Admin,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let created = with_rbac_request_scope(Some(scope()), async {
            provider.create_user(&context, command()).await
        })
        .await
        .unwrap();
        assert_eq!(created.role, target_role.slug);
        let memberships = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(created.id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(memberships.len(), 1);
        assert_eq!(memberships[0].role_id, target_role.id);
        let registration_events = sys_events::Entity::find()
            .filter(sys_events::Column::TenantId.eq(tenant.id))
            .filter(sys_events::Column::EventType.eq("user.account_registered"))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            registration_events.len(),
            1,
            "admin-created accounts must publish exactly one registration event"
        );
        assert!(
            roles::Entity::find()
                .filter(roles::Column::TenantId.eq(tenant.id))
                .filter(roles::Column::Slug.eq("customer"))
                .one(&db)
                .await
                .unwrap()
                .is_none()
        );
        let tx = db.begin().await.unwrap();
        assert_eq!(
            rustok_rbac::read_user_membership_revision_on(&tx, tenant.id, created.id)
                .await
                .unwrap(),
            1
        );
        tx.rollback().await.unwrap();

        users::ActiveModel {
            id: Set(actor.id),
            status: Set(UserStatus::Inactive),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        let denied = with_rbac_request_scope(Some(scope()), async {
            let mut command = command();
            command.email = "inactive-actor@example.test".to_string();
            provider.create_user(&context, command).await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
        assert!(
            users::Entity::find_by_email(&db, tenant.id, "inactive-actor@example.test")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn stale_user_admin_snapshot_cannot_create_update_or_delete() {
        use crate::models::{tenants, users};
        use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use rustok_api::{AuthPrincipalKind, Permission};
        use rustok_auth::{
            AuthAdminMutationContext, CreateUserCommand, UpdateUserCommand, UserAdminMutationPort,
        };
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "stale-user-admin")
            .insert(&db)
            .await
            .unwrap();
        let actor = users::ActiveModel::new(tenant.id, "actor@example.test", "hash")
            .insert(&db)
            .await
            .unwrap();
        let target = users::ActiveModel::new(tenant.id, "target@example.test", "hash")
            .insert(&db)
            .await
            .unwrap();
        let tx = db.begin().await.unwrap();
        rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
            &tx,
            tenant.id,
            actor.id,
            UserRole::Admin,
        )
        .await
        .unwrap();
        rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
            &tx,
            tenant.id,
            target.id,
            UserRole::Customer,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let runtime = ServerRuntimeContext::new(db.clone(), Default::default());
        let provider = super::ServerAuthAdminMutationProvider::new(
            &runtime,
            crate::auth::test_auth_config("aB3!zY7@qW8#eR2$".repeat(4)),
        );
        let context = AuthAdminMutationContext {
            actor_id: actor.id,
            tenant_id: tenant.id,
            request_id: None,
            locale: Some("en".to_string()),
            principal_kind: Some(AuthPrincipalKind::DirectUser),
            credential_authority: rustok_auth::CredentialAuthority::TrustedInternal,
        };
        let scope = || {
            RbacRequestScope::new(
                tenant.id,
                actor.id,
                vec![Permission::USERS_MANAGE],
                UserRole::Admin,
            )
        };
        let create = |email: &str| CreateUserCommand {
            email: email.to_string(),
            password: "Password123!Strong".to_string(),
            name: None,
            role: Some("customer".to_string()),
            status: None,
            custom_fields: None,
        };
        let created = with_rbac_request_scope(Some(scope()), async {
            provider
                .create_user(&context, create("allowed@example.test"))
                .await
        })
        .await
        .unwrap();
        assert_eq!(created.role, "customer");

        // The authenticated path must not accept a stale or substituted session.
        let session = crate::models::sessions::ActiveModel::new(
            tenant.id,
            actor.id,
            "admin-session-test-digest".to_string(),
            chrono::Utc::now() + chrono::Duration::hours(1),
            None,
            None,
        )
        .insert(&db)
        .await
        .unwrap();
        let current = crate::extractors::auth::CurrentUser {
            user: actor.clone(),
            session_id: session.id,
            permissions: vec![Permission::USERS_MANAGE],
            inferred_role: UserRole::Admin,
            actor_kind: rustok_core::SecurityActorKind::User,
            principal_kind: AuthPrincipalKind::DirectUser,
            client_id: None,
            scopes: Vec::new(),
            grant_type: "direct".to_string(),
            access_token_expires_at: chrono::Utc::now() + chrono::Duration::minutes(10),
            oauth_credential: None,
        };
        let authenticated = |deadline| RbacRequestScope::authenticated(&current, deadline);
        let authenticated_context = |deadline| AuthAdminMutationContext {
            credential_authority: authenticated(deadline).admin_credential().unwrap(),
            ..context.clone()
        };
        let allowed = with_rbac_request_scope(
            Some(authenticated(current.access_token_expires_at)),
            async {
                provider
                    .create_user(
                        &authenticated_context(current.access_token_expires_at),
                        create("session-allowed@example.test"),
                    )
                    .await
            },
        )
        .await;
        assert!(allowed.is_ok(), "{allowed:?}");

        // A tenant disabled after request admission must not authorize any
        // user-admin mutation, even with a still-live direct session.
        tenants::ActiveModel {
            id: Set(tenant.id),
            is_active: Set(false),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        let inactive_tenant = with_rbac_request_scope(
            Some(authenticated(current.access_token_expires_at)),
            async {
                provider
                    .create_user(
                        &authenticated_context(current.access_token_expires_at),
                        create("tenant-inactive@example.test"),
                    )
                    .await
            },
        )
        .await;
        assert!(matches!(
            inactive_tenant,
            Err(AuthAdminMutationError::Unauthorized)
        ));
        assert!(
            users::Entity::find_by_email(&db, tenant.id, "tenant-inactive@example.test")
                .await
                .unwrap()
                .is_none()
        );
        tenants::ActiveModel {
            id: Set(tenant.id),
            is_active: Set(true),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();

        let expired_at = chrono::Utc::now();
        let expired = with_rbac_request_scope(Some(authenticated(expired_at)), async {
            provider
                .create_user(
                    &authenticated_context(expired_at),
                    create("jwt-expired@example.test"),
                )
                .await
        })
        .await;
        assert!(matches!(expired, Err(AuthAdminMutationError::Unauthorized)));
        crate::models::sessions::ActiveModel {
            id: Set(session.id),
            revoked_at: Set(Some(chrono::Utc::now().into())),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        let revoked = with_rbac_request_scope(
            Some(authenticated(current.access_token_expires_at)),
            async {
                provider
                    .create_user(
                        &authenticated_context(current.access_token_expires_at),
                        create("session-revoked@example.test"),
                    )
                    .await
            },
        )
        .await;
        assert!(matches!(revoked, Err(AuthAdminMutationError::Unauthorized)));
        for email in ["jwt-expired@example.test", "session-revoked@example.test"] {
            assert!(
                users::Entity::find_by_email(&db, tenant.id, email)
                    .await
                    .unwrap()
                    .is_none()
            );
        }

        // A live user with a stale request is no longer an administrator.
        let tx = db.begin().await.unwrap();
        rustok_rbac::RbacRoleAssignmentDbWriter::replace_role_on(
            &tx,
            tenant.id,
            actor.id,
            UserRole::Customer,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let denied = with_rbac_request_scope(Some(scope()), async {
            provider
                .create_user(&context, create("denied@example.test"))
                .await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
        assert!(
            users::Entity::find_by_email(&db, tenant.id, "denied@example.test")
                .await
                .unwrap()
                .is_none()
        );

        let denied = with_rbac_request_scope(Some(scope()), async {
            provider
                .update_user(
                    &context,
                    UpdateUserCommand {
                        id: target.id,
                        email: None,
                        password: None,
                        name: Some("Changed".to_string()),
                        role: None,
                        status: None,
                        custom_fields: None,
                    },
                )
                .await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
        let denied = with_rbac_request_scope(Some(scope()), async {
            provider.delete_user(&context, target.id).await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
        let after = users::Entity::find_by_id(target.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.name, target.name);
        assert_eq!(after.status, target.status);

        // A UUID collision cannot convert an application into a user actor.
        let tx = db.begin().await.unwrap();
        rustok_rbac::RbacRoleAssignmentDbWriter::replace_role_on(
            &tx,
            tenant.id,
            actor.id,
            UserRole::Admin,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let mut service = context.clone();
        service.principal_kind = Some(AuthPrincipalKind::Service);
        let denied = with_rbac_request_scope(Some(scope()), async {
            provider
                .create_user(&service, create("service@example.test"))
                .await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
        users::ActiveModel {
            id: Set(actor.id),
            status: Set(UserStatus::Inactive),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        let denied = with_rbac_request_scope(Some(scope()), async {
            provider.delete_user(&context, target.id).await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
    }

    #[tokio::test]
    async fn stale_invite_request_authority_cannot_issue_for_missing_inactive_or_ungranted_actor() {
        use crate::models::{auth_invites, tenants, users};
        use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use rustok_api::Permission;
        use rustok_auth::{
            AuthAdminMutationContext, IssueUserInviteCommand, UserAdminMutationPort,
        };
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set};
        use uuid::Uuid;

        let db = setup_test_db_with_migrations::<SqliteTestMigrator>().await;
        let tenant = tenants::ActiveModel::new("Tenant", "invite-stale-authority")
            .insert(&db)
            .await
            .expect("tenant");
        let mut inactive = users::ActiveModel::new(tenant.id, "inactive@example.test", "hash");
        inactive.status = Set(UserStatus::Inactive);
        let inactive = inactive.insert(&db).await.expect("inactive actor");
        let active = users::ActiveModel::new(tenant.id, "active@example.test", "hash")
            .insert(&db)
            .await
            .expect("actor without grants");
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
                credential_authority: rustok_auth::CredentialAuthority::TrustedInternal,
            };
            let scope = RbacRequestScope::new(
                tenant.id,
                actor_id,
                vec![Permission::USERS_MANAGE],
                UserRole::Admin,
            );
            let result = with_rbac_request_scope(Some(scope), async {
                provider
                    .issue_user_invite(
                        &context,
                        IssueUserInviteCommand {
                            email: "invitee@example.test".to_string(),
                            role: Some("customer".to_string()),
                        },
                    )
                    .await
            })
            .await;
            assert!(matches!(result, Err(AuthAdminMutationError::Forbidden(_))));
        }
        assert!(
            auth_invites::Entity::find()
                .all(&db)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn invite_revocation_rolls_back_when_runtime_event_transport_fails() {
        use crate::models::{auth_invite_consumptions, auth_invites, tenants, users};
        use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};
        use crate::services::server_runtime_context::ServerRuntimeContext;
        use rustok_api::Permission;
        use rustok_auth::{AuthAdminMutationContext, UserAdminMutationPort};
        use rustok_core::events::{EventTransport, ReliabilityLevel};
        use rustok_events::EventEnvelope;
        use rustok_migrations::SqliteTestMigrator;
        use rustok_test_utils::db::setup_test_db_with_migrations;
        use sea_orm::{ActiveModelTrait, EntityTrait, Set, TransactionTrait};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        };
        use uuid::Uuid;

        struct FailingTransport(AtomicUsize, AtomicBool);

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
                if self.1.load(Ordering::SeqCst) {
                    Err(rustok_core::Error::Validation(
                        "injected outbox failure".to_string(),
                    ))
                } else {
                    Ok(())
                }
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
        let actor = users::ActiveModel::new(tenant.id, "revoker@example.test", "hash")
            .insert(&db)
            .await
            .unwrap();
        let actor_id = actor.id;
        let invite_id = Uuid::new_v4();
        let now = chrono::Utc::now();
        auth_invites::ActiveModel {
            id: Set(invite_id),
            tenant_id: Set(tenant.id),
            token_hash: Set("a".repeat(64)),
            email: Set("invitee@example.test".to_string()),
            role: Set("customer".to_string()),
            issued_by: Set(None),
            issuer_recovery_revision: Set(None),
            issuer_membership_revision: Set(None),
            role_grant_binding: Set(None),
            module_policy_binding: Set(None),
            expires_at: Set((now + chrono::Duration::hours(1)).into()),
            revoked_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .expect("invite");
        let runtime = ServerRuntimeContext::new(db.clone(), Default::default());
        let failing = Arc::new(FailingTransport(AtomicUsize::new(0), AtomicBool::new(true)));
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
            credential_authority: rustok_auth::CredentialAuthority::TrustedInternal,
        };
        let scope = || {
            RbacRequestScope::new(
                tenant.id,
                actor_id,
                vec![Permission::USERS_MANAGE],
                UserRole::Admin,
            )
        };

        let denied = with_rbac_request_scope(Some(scope()), async {
            provider.revoke_user_invite(&context, invite_id).await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
        assert_eq!(failing.0.load(Ordering::SeqCst), 0);
        let missing = with_rbac_request_scope(Some(scope()), async {
            provider.revoke_user_invite(&context, Uuid::new_v4()).await
        })
        .await;
        assert!(matches!(missing, Err(AuthAdminMutationError::Forbidden(_))));
        let tx = db.begin().await.unwrap();
        rustok_rbac::RbacRoleAssignmentDbWriter::assign_role_on(
            &tx,
            tenant.id,
            actor_id,
            UserRole::Admin,
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();

        let result = with_rbac_request_scope(Some(scope()), async {
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

        // A successful retry emits one event; idempotent retries do not.
        // The ledger has no issuer or target-role binding and can still
        // be revoked by a current administrator.
        failing.1.store(false, Ordering::SeqCst);
        let revoked = with_rbac_request_scope(Some(scope()), async {
            provider.revoke_user_invite(&context, invite_id).await
        })
        .await
        .unwrap();
        assert!(revoked.revoked_at.is_some());
        let retry = with_rbac_request_scope(Some(scope()), async {
            provider.revoke_user_invite(&context, invite_id).await
        })
        .await
        .unwrap();
        assert_eq!(retry.revoked_at, revoked.revoked_at);
        assert_eq!(failing.0.load(Ordering::SeqCst), 2);

        // A committed acceptance receipt is terminal under the same fence.
        let consumed_invite = auth_invites::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            token_hash: Set("b".repeat(64)),
            email: Set("accepted@example.test".to_string()),
            role: Set("customer".to_string()),
            issued_by: Set(None),
            issuer_recovery_revision: Set(None),
            issuer_membership_revision: Set(None),
            role_grant_binding: Set(None),
            module_policy_binding: Set(None),
            expires_at: Set((now + chrono::Duration::hours(1)).into()),
            revoked_at: Set(None),
            created_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .unwrap();
        auth_invite_consumptions::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant.id),
            token_hash: Set(consumed_invite.token_hash.clone()),
            email: Set(consumed_invite.email.clone()),
            role: Set(consumed_invite.role.clone()),
            user_id: Set(None),
            expires_at: Set(consumed_invite.expires_at),
            consumed_at: Set(now.into()),
        }
        .insert(&db)
        .await
        .unwrap();
        let result = with_rbac_request_scope(Some(scope()), async {
            provider
                .revoke_user_invite(&context, consumed_invite.id)
                .await
        })
        .await;
        assert!(matches!(result, Err(AuthAdminMutationError::Conflict(_))));
        assert_eq!(failing.0.load(Ordering::SeqCst), 2);

        // Losing the account fence also denies a previously idempotent retry.
        users::ActiveModel {
            id: Set(actor_id),
            status: Set(UserStatus::Inactive),
            ..Default::default()
        }
        .update(&db)
        .await
        .unwrap();
        let denied = with_rbac_request_scope(Some(scope()), async {
            provider.revoke_user_invite(&context, invite_id).await
        })
        .await;
        assert!(matches!(denied, Err(AuthAdminMutationError::Forbidden(_))));
        assert_eq!(failing.0.load(Ordering::SeqCst), 2);
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
        assert!(!email_identity_changed("new@example.test", None,));
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
