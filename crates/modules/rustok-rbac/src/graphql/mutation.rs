use async_graphql::{Context, FieldError, InputObject, MaybeUndefined, Object, Result};
use rustok_api::{
    AuthContext, AuthPrincipalContext, Permission, StoredLocale, TenantContext,
    graphql::GraphQLError, has_effective_permission,
};
use rustok_core::{UserRole, i18n::Locale};
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use uuid::Uuid;

use crate::RbacPresentationResourceKind;

use super::control_plane::{prepare_statement, require_direct_control_plane_user};
use super::types::{
    AssignUserRolePayload, CreateRoleInput, DeleteRolePayload, RbacGraphqlUserRole, RoleInfo,
    RoleMutationPayload, UpdateRoleInput,
};
use super::{
    RbacGraphqlRoleWriteError, RbacGraphqlRoleWriterHandle, RbacLockedMutationAuthority,
    RbacMutationAuthorityHandle,
};

#[derive(InputObject)]
pub struct AssignUserRoleInput {
    pub user_id: Uuid,
    /// Canonical built-in role. Kept for compatibility; prefer `role_slug`,
    /// which also accepts tenant-persisted custom roles. Exactly one of
    /// `role` / `role_slug` may be set, or both with matching values.
    pub role: Option<RbacGraphqlUserRole>,
    /// Slug of any role persisted for the tenant, built-in or custom.
    pub role_slug: Option<String>,
}

#[derive(Default)]
pub struct RbacMutation;

fn map_role_write_error(error: RbacGraphqlRoleWriteError) -> FieldError {
    match error {
        RbacGraphqlRoleWriteError::Forbidden(message) => {
            <FieldError as GraphQLError>::permission_denied(&message)
        }
        RbacGraphqlRoleWriteError::NotFound(message) => {
            <FieldError as GraphQLError>::not_found(&message)
        }
        RbacGraphqlRoleWriteError::Conflict(message) => {
            <FieldError as GraphQLError>::bad_user_input(&message)
        }
        RbacGraphqlRoleWriteError::Internal(message) => {
            <FieldError as GraphQLError>::internal_error(&message)
        }
    }
}

const ROLE_MANAGEMENT: &[Permission] = &[Permission::SETTINGS_MANAGE, Permission::USERS_MANAGE];

async fn prepare_role_authority(ctx: &Context<'_>) -> Result<RbacMutationAuthorityHandle> {
    let handle = ctx
        .data_opt::<RbacMutationAuthorityHandle>()
        .cloned()
        .ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error("RBAC mutation authority is unavailable")
        })?;
    handle.0.prepare().await.map_err(map_role_write_error)?;
    Ok(handle)
}

async fn lock_role_authority(
    handle: &RbacMutationAuthorityHandle,
    tx: &sea_orm::DatabaseTransaction,
    auth: &AuthContext,
    tenant_id: Uuid,
    target_role_id: Option<Uuid>,
) -> Result<RbacLockedMutationAuthority> {
    let mut authority = handle
        .0
        .lock(tx, tenant_id, auth.user_id, target_role_id, ROLE_MANAGEMENT)
        .await
        .map_err(map_role_write_error)?;
    // Enforce the owner contract even if an adapter returns raw live grants.
    authority.restrict_to(&auth.permissions);
    if !ROLE_MANAGEMENT
        .iter()
        .any(|p| has_effective_permission(&authority.permissions, p))
    {
        return Err(<FieldError as GraphQLError>::permission_denied(
            "current role management permission required",
        ));
    }
    Ok(authority)
}

/// Map a storage failure to a generic internal error. Raw database errors
/// (SQL text, constraint names) are logged server-side and never returned to
/// GraphQL clients.
fn internal_db_error<E: std::fmt::Display>(operation: &str, error: E) -> FieldError {
    tracing::error!(%error, operation, "RBAC role mutation storage operation failed");
    <FieldError as GraphQLError>::internal_error("RBAC role storage is temporarily unavailable")
}

fn is_unique_violation(error: &sea_orm::DbErr) -> bool {
    matches!(error.sql_err(), Some(sea_orm::SqlErr::UniqueConstraintViolation(_)))
}

// Private owner-only error boundary: do not erase the original SQLSTATE
// until the complete transaction has been rolled back and retry decided.
enum RoleAttemptError {
    Statement(sea_orm::DbErr),
    Rejected(FieldError),
}

impl From<FieldError> for RoleAttemptError {
    fn from(error: FieldError) -> Self {
        Self::Rejected(error)
    }
}

impl From<sea_orm::DbErr> for RoleAttemptError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Statement(error)
    }
}

impl From<crate::RbacInvalidationGenerationError> for RoleAttemptError {
    fn from(error: crate::RbacInvalidationGenerationError) -> Self {
        match error {
            crate::RbacInvalidationGenerationError::Database(error) => Self::Statement(error),
            other => Self::Rejected(internal_db_error("rbac role invalidation generation", other)),
        }
    }
}

/// Returns Ok only when a fresh attempt is permitted. Commit and host port
/// errors must never be passed to this SQL-statement-only retry boundary.
async fn retry_role_attempt(
    tx: sea_orm::DatabaseTransaction,
    error: RoleAttemptError,
    attempt: u32,
) -> Result<()> {
    match error {
        RoleAttemptError::Statement(error) => {
            if rustok_core::resilience::retry::rollback_for_transaction_retry(tx, &error, attempt)
                .await
                .map_err(|e| internal_db_error("rbac role rollback", e))?
            {
                Ok(())
            } else {
                Err(internal_db_error("rbac role mutation", error))
            }
        }
        RoleAttemptError::Rejected(error) => {
            tx.rollback()
                .await
                .map_err(|e| internal_db_error("rbac role rollback", e))?;
            Err(error)
        }
    }
}

async fn abort_tx(tx: sea_orm::DatabaseTransaction, err: FieldError) -> FieldError {
    if let Err(rollback_error) = tx.rollback().await {
        tracing::warn!(
            error = %rollback_error,
            "RBAC role mutation transaction rollback failed"
        );
    }
    err
}

/// Parse role permissions against the canonical permission vocabulary.
/// Malformed strings are a client error, never silently skipped.
fn parse_role_permissions(raw_permissions: &[String]) -> Result<Vec<Permission>> {
    let mut permissions: Vec<Permission> = Vec::with_capacity(raw_permissions.len());
    for raw in raw_permissions {
        let permission: Permission = raw.trim().parse().map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(&format!(
                "Invalid permission '{raw}': expected a canonical '<resource>:<action>' permission"
            ))
        })?;
        if !permissions.contains(&permission) {
            permissions.push(permission);
        }
    }
    permissions.sort_by_cached_key(ToString::to_string);
    Ok(permissions)
}

/// Enforce the actor's authority ceiling on role permissions.
/// Existing dormant grants (owned by a currently disabled tenant module and
/// stripped from `actor_permissions` at login) may be retained across saves,
/// whereas active grants and newly added grants require the actor to hold them.
fn authorize_role_permissions(
    actor_permissions: &[Permission],
    availability: Option<&crate::PermissionAvailability>,
    desired: &[Permission],
    already_granted: &[String],
) -> Result<()> {
    for permission in desired {
        if has_effective_permission(actor_permissions, permission) {
            continue;
        }
        let key = permission.to_string();
        let is_retained_dormant = already_granted.contains(&key)
            && matches!(
                availability.map(|snapshot| snapshot.status(&key)),
                Some(crate::ModulePermissionStatus::Disabled(_))
            );
        if !is_retained_dormant {
            return Err(<FieldError as GraphQLError>::permission_denied(&format!(
                "cannot grant permission '{permission}' because it exceeds the current request authority"
            )));
        }
    }
    Ok(())
}

/// Parse role permissions against the canonical permission vocabulary and
/// enforce the actor's authority ceiling with no prior dormant grant retention.
fn parse_and_authorize_role_permissions(
    actor_permissions: &[Permission],
    raw_permissions: &[String],
) -> Result<Vec<Permission>> {
    let permissions = parse_role_permissions(raw_permissions)?;
    authorize_role_permissions(actor_permissions, None, &permissions, &[])?;
    Ok(permissions)
}

/// Reject grants owned by modules disabled for this tenant. Dormant grants
/// already persisted on a role are preserved (they reactivate when the
/// module is enabled again); only *newly added* dormant grants are refused,
/// because granting a permission that cannot take effect is always a
/// configuration mistake.
fn reject_newly_dormant_grants(
    availability: Option<&crate::PermissionAvailability>,
    validated: &[Permission],
    already_granted: &[String],
) -> Result<()> {
    let Some(snapshot) = availability else {
        return Ok(());
    };
    let dormant: Vec<String> = validated
        .iter()
        .filter(|permission| !snapshot.is_active(permission))
        .map(ToString::to_string)
        .filter(|permission| !already_granted.contains(permission))
        .collect();
    if dormant.is_empty() {
        Ok(())
    } else {
        Err(<FieldError as GraphQLError>::bad_user_input(&format!(
            "permissions belong to modules disabled for this tenant and cannot be granted: {}",
            dormant.join(", ")
        )))
    }
}

fn dormant_grant_strings(
    availability: Option<&crate::PermissionAvailability>,
    grants: &[String],
) -> Vec<String> {
    match availability {
        Some(snapshot) => grants
            .iter()
            .filter(|permission| {
                matches!(
                    snapshot.status(permission),
                    crate::ModulePermissionStatus::Disabled(_)
                )
            })
            .cloned()
            .collect(),
        None => Vec::new(),
    }
}

fn permission_parts(permission: &Permission) -> (String, String) {
    (
        permission.resource.to_string(),
        permission.action.to_string(),
    )
}

fn normalize_description(value: &str) -> Result<Option<String>> {
    let trimmed = value.trim();
    if trimmed.chars().count() > 2000 {
        return Err(<FieldError as GraphQLError>::bad_user_input(
            "Role description must be at most 2000 characters",
        ));
    }
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

async fn grant_role_permissions_in_tx(
    tx: &sea_orm::DatabaseTransaction,
    backend: sea_orm::DbBackend,
    tenant_id: Uuid,
    role_id: Uuid,
    permissions: &[Permission],
) -> std::result::Result<Vec<String>, RoleAttemptError> {
    let mut granted = Vec::with_capacity(permissions.len());
    // All role writers materialize shared vocabulary keys in the same order.
    let mut ordered = permissions.iter().collect::<Vec<_>>();
    ordered.sort_by_cached_key(|permission| permission_parts(permission));
    for permission in ordered {
        let (resource, action) = permission_parts(permission);
        let (resource, action) = (resource.as_str(), action.as_str());

        let find_perm_sql = "SELECT id FROM permissions WHERE tenant_id = ? AND resource = ? AND action = ? LIMIT 1";
        let perm_row = tx
            .query_one_raw(prepare_statement(
                backend,
                find_perm_sql,
                vec![tenant_id.into(), resource.into(), action.into()],
            ))
            .await?;

        let perm_id = match perm_row {
            Some(row) => row
                .try_get::<Uuid>("", "id")?,
            None => {
                let new_perm_id = rustok_core::generate_id();
                let ins_perm_sql = "INSERT INTO permissions (id, tenant_id, resource, action, description) VALUES (?, ?, ?, ?, NULL) ON CONFLICT (tenant_id, resource, action) DO NOTHING";
                tx.execute_raw(prepare_statement(
                    backend,
                    ins_perm_sql,
                    vec![
                        new_perm_id.into(),
                        tenant_id.into(),
                        resource.into(),
                        action.into(),
                    ],
                ))
                .await?;

                let persisted_row = tx
                    .query_one_raw(prepare_statement(
                        backend,
                        find_perm_sql,
                        vec![tenant_id.into(), resource.into(), action.into()],
                    ))
                    .await?
                    .ok_or_else(|| {
                        <FieldError as GraphQLError>::internal_error(
                            "Permission could not be retrieved after insert",
                        )
                    })?;
                persisted_row
                    .try_get::<Uuid>("", "id")?
            }
        };

        let link_id = rustok_core::generate_id();
        let link_sql = "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?, ?, ?) ON CONFLICT (role_id, permission_id) DO NOTHING";
        tx.execute_raw(prepare_statement(
            backend,
            link_sql,
            vec![link_id.into(), role_id.into(), perm_id.into()],
        ))
        .await?;

        granted.push(format!("{resource}:{action}"));
    }
    Ok(granted)
}

#[Object]
impl RbacMutation {
    /// Create a new custom role with assigned permissions.
    /// Requires `settings:manage` or `users:manage`.
    async fn create_role(
        &self,
        ctx: &Context<'_>,
        input: CreateRoleInput,
    ) -> Result<RoleMutationPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let principal_context = ctx
            .data::<AuthPrincipalContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, *principal_context, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE)
            && !has_effective_permission(&auth.permissions, &Permission::USERS_MANAGE)
        {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:manage or users:manage required to create roles",
            ));
        }

        let name = input.name.trim();
        let slug = input.slug.trim().to_lowercase();
        if name.is_empty() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role name cannot be empty",
            ));
        }
        if name.chars().count() > 255 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role name must be at most 255 characters",
            ));
        }
        let normalized_description = match input.description.as_deref() {
            Some(value) => normalize_description(value)?,
            None => None,
        };
        if slug.len() < 2 || slug.len() > 64 {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role slug must be between 2 and 64 characters",
            ));
        }
        if !slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role slug must contain only lowercase letters, numbers, underscores or hyphens",
            ));
        }

        let reserved = ["super_admin", "admin", "manager", "customer"];
        if reserved.contains(&slug.as_str()) {
            return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                "Role slug '{slug}' is reserved for platform built-in roles"
            )));
        }

        let db = ctx.data::<DatabaseConnection>()?;
        let backend = db.get_database_backend();

        let validated_permissions =
            parse_and_authorize_role_permissions(&auth.permissions, &input.permissions)?;
        let mut attempt = 1;
        loop {
            // Repeat host admission and live credential/grant checks; no
            // authority or transaction-local facts survive rollback.
            let authority_handle = prepare_role_authority(ctx).await?;

            let role_id = rustok_core::generate_id();
            let tx = db
                .begin()
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;

            let authority =
                match lock_role_authority(&authority_handle, &tx, auth, tenant.id, None).await {
                    Ok(authority) => authority,
                    Err(err) => return Err(abort_tx(tx, err).await),
                };
            let availability = &authority.availability;

            // The uniqueness check runs inside the creating transaction; the
            // UNIQUE (tenant_id, slug) index remains the final arbiter under
            // concurrency and its violation is reported as a clean conflict.
            let tx_outcome: std::result::Result<Vec<String>, RoleAttemptError> = async {
                authorize_role_permissions(&authority.permissions, availability.as_ref(), &validated_permissions, &[])?;
                reject_newly_dormant_grants(availability.as_ref(), &validated_permissions, &[])?;
                let check_sql = "SELECT id FROM roles WHERE tenant_id = ? AND slug = ? LIMIT 1";
                let existing = tx
                    .query_one_raw(prepare_statement(
                        backend,
                        check_sql,
                        vec![tenant.id.into(), slug.clone().into()],
                    ))
                    .await?;
                if existing.is_some() {
                    return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                        "A role with slug '{slug}' already exists in this tenant"
                    )).into());
                }

                let insert_role_sql = "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES (?, ?, ?, ?, ?, FALSE)";
                if let Err(error) = tx
                    .execute_raw(prepare_statement(
                        backend,
                        insert_role_sql,
                        vec![
                            role_id.into(),
                            tenant.id.into(),
                            name.into(),
                            slug.clone().into(),
                            normalized_description.clone().into(),
                        ],
                    ))
                    .await
                {
                    if is_unique_violation(&error) {
                        return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                            "A role with slug '{slug}' already exists in this tenant"
                        )).into());
                    }
                    return Err(error.into());
                }

                grant_role_permissions_in_tx(&tx, backend, tenant.id, role_id, &validated_permissions)
                    .await
            }
            .await;

            let mut granted_permissions = match tx_outcome {
                Ok(perms) => perms,
                Err(err) => {
                    retry_role_attempt(tx, err, attempt).await?;
                    attempt += 1;
                    continue;
                }
            };

            if let Err(error) = authority_handle.0.ensure_deadline(&tx, &authority).await {
                return Err(abort_tx(tx, map_role_write_error(error)).await);
            }
            tx.commit()
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;
            tracing::info!(actor_id = %auth.user_id, tenant_id = %tenant.id, role_slug = %slug, "rbac role created");

            granted_permissions.sort();
            granted_permissions.dedup();

            return Ok(RoleMutationPayload {
                success: true,
                role: Some(RoleInfo {
                    id: Some(role_id.to_string()),
                    slug,
                    display_name: name.to_string(),
                    description: normalized_description,
                    is_system: false,
                    inactive_permissions: dormant_grant_strings(
                        availability.as_ref(),
                        &granted_permissions,
                    ),
                    permissions: granted_permissions,
                }),
            });
        }
    }

    /// Update an existing role (name, description, or assigned permissions).
    /// Built-in system roles preserve their slug. Super administrator permissions cannot be modified.
    /// Requires `settings:manage` or `users:manage`.
    async fn update_role(
        &self,
        ctx: &Context<'_>,
        input: UpdateRoleInput,
    ) -> Result<RoleMutationPayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let principal_context = ctx
            .data::<AuthPrincipalContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, *principal_context, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE)
            && !has_effective_permission(&auth.permissions, &Permission::USERS_MANAGE)
        {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:manage or users:manage required to update roles",
            ));
        }

        let slug = input.slug.trim().to_lowercase();
        if input
            .name
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role name cannot be empty",
            ));
        }
        if input
            .name
            .as_ref()
            .is_some_and(|value| value.trim().chars().count() > 255)
        {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role name must be at most 255 characters",
            ));
        }
        if let MaybeUndefined::Value(ref value) = input.description
            && value.trim().chars().count() > 2000
        {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role description must be at most 2000 characters",
            ));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        let backend = db.get_database_backend();
        let locale = ctx.data_opt::<Locale>().copied().unwrap_or_default();
        let requested_locale = StoredLocale::new(locale.as_str())
            .or_else(|_| StoredLocale::new("en"))
            .map_err(|e| internal_db_error("rbac role presentation locale", e))?;

        let find_role_sql = "SELECT id FROM roles WHERE tenant_id = ? AND slug = ? LIMIT 1";
        let existing = db
            .query_one_raw(prepare_statement(
                backend,
                find_role_sql,
                vec![tenant.id.into(), slug.clone().into()],
            ))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?
            .ok_or_else(|| {
                <FieldError as GraphQLError>::not_found(&format!("Role '{slug}' not found"))
            })?;

        let role_id: Uuid = existing
            .try_get("", "id")
            .map_err(|e| internal_db_error("rbac role mutation", e))?;

        // Default roles are persisted tenant records and are editable
        // (name, description, permissions) within the actor's authority
        // ceiling enforced below. The single exception is the break-glass
        // super administrator: its grant set is immutable through this
        // endpoint so a tenant can never lock itself out of recovery.
        if slug == "super_admin" && input.permissions.is_some() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Super administrator permissions are immutable and cannot be restricted",
            ));
        }

        let parsed_update_permissions = match &input.permissions {
            Some(raw) => Some(parse_role_permissions(raw)?),
            None => None,
        };

        // Dormant grants already on the role are preserved across saves;
        // adding a *new* grant of a disabled module is refused. The existing
        // grants are read under the role row lock below to avoid a concurrent
        // permission replacement racing this check.
        // Resolve the optional host notifier before the transaction. The durable
        // generation remains mandatory even when no fast-path notifier exists.
        let role_writer = if parsed_update_permissions.is_some() {
            ctx.data_opt::<RbacGraphqlRoleWriterHandle>()
                .map(|handle| handle.0.clone())
        } else {
            None
        };

        let mut attempt = 1;
        loop {
            // Repeat host admission and live credential/grant checks; no
            // authority or transaction-local facts survive rollback.
            let authority_handle = prepare_role_authority(ctx).await?;
            let tx = db
                .begin()
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;

            let authority =
                match lock_role_authority(&authority_handle, &tx, auth, tenant.id, Some(role_id)).await
                {
                    Ok(authority) => authority,
                    Err(err) => return Err(abort_tx(tx, err).await),
                };
            let availability = &authority.availability;

            // Lock and read current metadata inside the transaction so concurrent
            // renames/edits or deletes are serialized. Revalidate the requested slug
            // as well as the ID: a concurrent SQL rename can change break-glass status.
            let requested_desc_patch: Option<Option<String>> = match input.description {
                MaybeUndefined::Undefined => None,
                MaybeUndefined::Null => Some(None),
                MaybeUndefined::Value(ref value) => Some(normalize_description(value)?),
            };
            let requested_name = input.name.as_ref().map(|n| n.trim().to_string());
            let permissions_updated = parsed_update_permissions.is_some();

            let tx_outcome: std::result::Result<(
                String,
                Option<String>,
                bool,
                Vec<String>,
                Option<u64>,
            ), RoleAttemptError> = async {
                let lock_role_sql = match backend {
                    sea_orm::DbBackend::Sqlite => {
                        "UPDATE roles SET updated_at = updated_at WHERE tenant_id = ? AND id = ? AND slug = ? RETURNING name, description, is_system"
                    }
                    sea_orm::DbBackend::Postgres | sea_orm::DbBackend::MySql | _ => {
                        "SELECT name, description, is_system FROM roles WHERE tenant_id = ? AND id = ? AND slug = ? LIMIT 1 FOR UPDATE"
                    }
                };
                let tx_role = tx
                    .query_one_raw(prepare_statement(
                        backend,
                        lock_role_sql,
                        vec![tenant.id.into(), role_id.into(), slug.clone().into()],
                    ))
                    .await?
                    .ok_or_else(|| {
                        <FieldError as GraphQLError>::not_found(&format!("Role '{slug}' not found"))
                    })?;

                let current_name: String = tx_role
                    .try_get("", "name")?;
                let current_desc: Option<String> = tx_role
                    .try_get("", "description")?;
                let is_system: bool = tx_role
                    .try_get("", "is_system")?;

                if let Some(new_permissions) = parsed_update_permissions.as_ref() {
                    let current_rows = tx
                        .query_all_raw(prepare_statement(
                            backend,
                            "SELECT p.resource, p.action FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id WHERE rp.role_id = ? AND p.tenant_id = ?",
                            vec![role_id.into(), tenant.id.into()],
                        ))
                        .await?;
                    let mut current_grants = Vec::with_capacity(current_rows.len());
                    for row in current_rows {
                        let resource: String = row
                            .try_get("", "resource")?;
                        let action: String = row
                            .try_get("", "action")?;
                        current_grants.push(format!("{resource}:{action}"));
                    }
                    authorize_role_permissions(
                        &auth.permissions,
                        availability.as_ref(),
                        new_permissions,
                        &current_grants,
                    )?;
                    authorize_role_permissions(
                        &authority.permissions, availability.as_ref(), new_permissions, &current_grants,
                    )?;
                    reject_newly_dormant_grants(
                        availability.as_ref(),
                        new_permissions,
                        &current_grants,
                    )?;
                }

                let name_updated = requested_name.is_some();
                let desc_updated = requested_desc_patch.is_some();
                let new_name = requested_name.unwrap_or(current_name);
                let new_desc = match requested_desc_patch {
                    Some(patch) => patch,
                    None => current_desc,
                };

                let update_role_sql = "UPDATE roles SET name = ?, description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
                let update_result = tx
                    .execute_raw(prepare_statement(
                        backend,
                        update_role_sql,
                        vec![
                            new_name.clone().into(),
                            new_desc.clone().into(),
                            role_id.into(),
                        ],
                    ))
                    .await?;
                if update_result.rows_affected() != 1 {
                    return Err(<FieldError as GraphQLError>::not_found(&format!(
                        "Role '{slug}' not found"
                    )).into());
                }

                let response_display_name =
                    if is_system && (name_updated || desc_updated) && slug.parse::<UserRole>().is_ok()
                    {
                        let existing_loc = tx
                            .query_one_raw(prepare_statement(
                                backend,
                                "SELECT name, description FROM rbac_localized_presentations WHERE tenant_id = ? AND resource_kind = ? AND resource_key = ? AND locale = ? LIMIT 1",
                                vec![
                                    tenant.id.into(),
                                    RbacPresentationResourceKind::Role.as_str().into(),
                                    slug.clone().into(),
                                    requested_locale.as_str().into(),
                                ],
                            ))
                            .await?;
                        let (existing_loc_name, existing_loc_desc) = match existing_loc {
                            Some(row) => (
                                Some(
                                    row.try_get::<String>("", "name")?,
                                ),
                                row.try_get::<Option<String>>("", "description")
                                    ?,
                            ),
                            None => (None, None),
                        };

                        // Preserve the existing locale copy for fields omitted from
                        // this partial update so editing description alone in `ru`
                        // never overwrites the localized `ru` role title with `en`.
                        let loc_name = if name_updated {
                            new_name.clone()
                        } else {
                            existing_loc_name.unwrap_or_else(|| new_name.clone())
                        };
                        let loc_desc = if desc_updated {
                            new_desc.clone()
                        } else {
                            existing_loc_desc.or_else(|| new_desc.clone())
                        };

                        let sync_presentation_sql = "INSERT INTO rbac_localized_presentations (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) ON CONFLICT (tenant_id, resource_kind, resource_key, locale) DO UPDATE SET name = EXCLUDED.name, description = EXCLUDED.description, copy_revision = rbac_localized_presentations.copy_revision + 1, updated_at = CURRENT_TIMESTAMP";
                        tx.execute_raw(prepare_statement(
                            backend,
                            sync_presentation_sql,
                            vec![
                                tenant.id.into(),
                                RbacPresentationResourceKind::Role.as_str().into(),
                                slug.clone().into(),
                                requested_locale.as_str().into(),
                                loc_name.clone().into(),
                                loc_desc.into(),
                            ],
                        ))
                        .await?;
                        loc_name
                    } else {
                        new_name
                    };

                let final_permissions = if let Some(new_permissions) = parsed_update_permissions.as_ref()
                {
                    let del_links_sql = "DELETE FROM role_permissions WHERE role_id = ?";
                    tx.execute_raw(prepare_statement(
                        backend,
                        del_links_sql,
                        vec![role_id.into()],
                    ))
                    .await?;

                    grant_role_permissions_in_tx(&tx, backend, tenant.id, role_id, new_permissions)
                        .await?
                } else {
                    let cur_perms_sql = "SELECT p.resource, p.action FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id WHERE rp.role_id = ? AND p.tenant_id = ?";
                    let rows = tx
                        .query_all_raw(prepare_statement(
                            backend,
                            cur_perms_sql,
                            vec![role_id.into(), tenant.id.into()],
                        ))
                        .await?;
                    let mut existing_permissions = Vec::with_capacity(rows.len());
                    for row in rows {
                        let resource: String = row
                            .try_get("", "resource")?;
                        let action: String = row
                            .try_get("", "action")?;
                        existing_permissions.push(format!("{resource}:{action}"));
                    }
                    existing_permissions
                };

                // Reserve the recovery watermark in the grant transaction. Supplying
                // an identical replacement still conservatively advances the generation.
                let committed_generation = if permissions_updated {
                    Some(
                        crate::reserve_permission_invalidation_generation(&tx)
                            .await?,
                    )
                } else {
                    None
                };

                Ok((
                    response_display_name,
                    new_desc,
                    is_system,
                    final_permissions,
                    committed_generation,
                ))
            }
            .await;

            let (
                response_display_name,
                new_desc,
                is_system,
                mut final_permissions,
                committed_generation,
            ) = match tx_outcome {
                Ok(values) => values,
                Err(err) => {
                    retry_role_attempt(tx, err, attempt).await?;
                    attempt += 1;
                    continue;
                }
            };

            if let Err(error) = authority_handle.0.ensure_deadline(&tx, &authority).await {
                return Err(abort_tx(tx, map_role_write_error(error)).await);
            }
            tx.commit()
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;

            // The grant change is already committed, so a fast-path failure must
            // not make the client believe it rolled back. Durable-generation
            // watchers can recover if both notification attempts fail or are skipped.
            if let (Some(generation), Some(writer)) = (committed_generation, role_writer.as_ref())
                && writer
                    .role_permissions_committed(&tenant.id, &role_id, generation)
                    .await
                    .is_err()
                && let Err(error) = writer
                    .role_permissions_committed(&tenant.id, &role_id, generation)
                    .await
            {
                tracing::error!(
                    %error,
                    tenant_id = %tenant.id,
                    role_id = %role_id,
                    generation,
                    "RBAC role permission change committed but fast invalidation failed; durable generation reconciliation must recover"
                );
            }

            tracing::info!(actor_id = %auth.user_id, tenant_id = %tenant.id, role_slug = %slug, permissions_updated, "rbac role updated");
            final_permissions.sort();
            final_permissions.dedup();

            return Ok(RoleMutationPayload {
                success: true,
                role: Some(RoleInfo {
                    id: Some(role_id.to_string()),
                    slug,
                    display_name: response_display_name,
                    description: new_desc,
                    is_system,
                    inactive_permissions: dormant_grant_strings(
                        availability.as_ref(),
                        &final_permissions,
                    ),
                    permissions: final_permissions,
                }),
            });
        }
    }

    /// Delete a role.
    /// Only the super administrator role can never be deleted; every other
    /// role — including the seeded defaults `admin`/`manager`/`customer` —
    /// can be deleted while it is not assigned to any user.
    /// Requires `settings:manage` or `users:manage`.
    async fn delete_role(&self, ctx: &Context<'_>, slug: String) -> Result<DeleteRolePayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let principal_context = ctx
            .data::<AuthPrincipalContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, *principal_context, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE)
            && !has_effective_permission(&auth.permissions, &Permission::USERS_MANAGE)
        {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "settings:manage or users:manage required to delete roles",
            ));
        }

        let slug = slug.trim().to_lowercase();
        let db = ctx.data::<DatabaseConnection>()?;
        let backend = db.get_database_backend();

        let find_role_sql = "SELECT id FROM roles WHERE tenant_id = ? AND slug = ? LIMIT 1";
        let existing = db
            .query_one_raw(prepare_statement(
                backend,
                find_role_sql,
                vec![tenant.id.into(), slug.clone().into()],
            ))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?
            .ok_or_else(|| {
                <FieldError as GraphQLError>::not_found(&format!("Role '{slug}' not found"))
            })?;

        let role_id: Uuid = existing
            .try_get("", "id")
            .map_err(|e| internal_db_error("rbac role mutation", e))?;

        // Core platform invariant: the super administrator role must NEVER be
        // deleted (break-glass recovery account). All other roles, including
        // the seeded defaults, are ordinary tenant records and may be deleted
        // while unassigned. The slug is sufficient here: `create_role`
        // reserves this slug, so no custom role can carry it.
        if slug == "super_admin" {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Cannot delete the super administrator role",
            ));
        }

        let mut attempt = 1;
        loop {
            // Repeat host admission and live credential/grant checks; no
            // authority or transaction-local facts survive rollback.
            let authority_handle = prepare_role_authority(ctx).await?;
            let tx = db
                .begin()
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;
            let authority =
                match lock_role_authority(&authority_handle, &tx, auth, tenant.id, Some(role_id)).await
                {
                    Ok(authority) => authority,
                    Err(err) => return Err(abort_tx(tx, err).await),
                };

            // Re-verify the role inside the deleting transaction: the lookup
            // above ran outside it, so a concurrent delete or slug change must
            // surface as not-found. The old slug is not proof of break-glass status.
            let tx_outcome: std::result::Result<(), RoleAttemptError> = async {
                let lock_role_sql = match backend {
                    sea_orm::DbBackend::Sqlite => {
                        "UPDATE roles SET updated_at = updated_at WHERE tenant_id = ? AND id = ? AND slug = ? RETURNING id"
                    }
                    sea_orm::DbBackend::Postgres | sea_orm::DbBackend::MySql | _ => {
                        "SELECT id FROM roles WHERE tenant_id = ? AND id = ? AND slug = ? LIMIT 1 FOR UPDATE"
                    }
                };
                let tx_role = tx
                    .query_one_raw(prepare_statement(
                        backend,
                        lock_role_sql,
                        vec![tenant.id.into(), role_id.into(), slug.clone().into()],
                    ))
                    .await?;
                if tx_role.is_none() {
                    return Err(<FieldError as GraphQLError>::not_found(&format!(
                        "Role '{slug}' not found"
                    )).into());
                }

                // Integrity invariant, checked inside the deleting transaction so a
                // concurrent assignment cannot race past it (no TOCTOU window), and a
                // failed count read fails closed instead of reading as zero.
                let count_sql = "SELECT COUNT(*) as cnt FROM user_roles WHERE role_id = ?";
                let count_row = tx
                    .query_one_raw(prepare_statement(backend, count_sql, vec![role_id.into()]))
                    .await?
                    .ok_or_else(|| {
                        <FieldError as GraphQLError>::internal_error(
                            "Role assignment count query returned no row",
                        )
                    })?;
                let count: i64 = count_row
                    .try_get("", "cnt")?;
                if count > 0 {
                    return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                        "Cannot delete role '{slug}' because it is currently assigned to {count} user(s). Reassign them first."
                    )).into());
                }

                let del_links_sql = "DELETE FROM role_permissions WHERE role_id = ?";
                tx.execute_raw(prepare_statement(
                    backend,
                    del_links_sql,
                    vec![role_id.into()],
                ))
                .await?;

                // Artifact grant cleanup is part of the same transaction: if it
                // fails, the whole delete rolls back instead of leaving orphaned
                // artifact permissions pointing at a deleted role.
                let del_artifact_links_sql =
                    "DELETE FROM rbac_artifact_role_permissions WHERE role_id = ?";
                tx.execute_raw(prepare_statement(
                    backend,
                    del_artifact_links_sql,
                    vec![role_id.into()],
                ))
                .await?;

                let del_artifact_ops_sql =
                    "DELETE FROM rbac_artifact_role_permission_operations WHERE role_id = ?";
                tx.execute_raw(prepare_statement(
                    backend,
                    del_artifact_ops_sql,
                    vec![role_id.into()],
                ))
                .await?;

                let del_role_sql = "DELETE FROM roles WHERE id = ?";
                let delete_result = tx
                    .execute_raw(prepare_statement(
                        backend,
                        del_role_sql,
                        vec![role_id.into()],
                    ))
                    .await?;
                if delete_result.rows_affected() != 1 {
                    return Err(<FieldError as GraphQLError>::not_found(&format!(
                        "Role '{slug}' not found"
                    )).into());
                }
                Ok(())
            }
            .await;

            if let Err(err) = tx_outcome {
                retry_role_attempt(tx, err, attempt).await?;
                attempt += 1;
                continue;
            }

            if let Err(error) = authority_handle.0.ensure_deadline(&tx, &authority).await {
                return Err(abort_tx(tx, map_role_write_error(error)).await);
            }
            tx.commit()
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;
            tracing::info!(actor_id = %auth.user_id, tenant_id = %tenant.id, role_slug = %slug, "rbac role deleted");

            return Ok(DeleteRolePayload {
                success: true,
                slug,
            });
        }
    }

    /// Assign a role to a user (replaces the current role). The role may be
    /// referenced by built-in enum (`role`) or by any tenant-persisted role
    /// slug (`roleSlug`), including custom roles; custom assignments are
    /// bounded by the actor's effective-permission ceiling.
    /// Requires a direct, session-bound user principal with `users:manage`;
    /// hierarchy, target and continuity rules are enforced transactionally by
    /// the host role writer.
    async fn assign_user_role(
        &self,
        ctx: &Context<'_>,
        input: AssignUserRoleInput,
    ) -> Result<AssignUserRolePayload> {
        let auth = ctx
            .data::<AuthContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let principal_context = ctx
            .data::<AuthPrincipalContext>()
            .map_err(|_| <FieldError as GraphQLError>::unauthenticated())?;
        let tenant = ctx.data::<TenantContext>()?;

        require_direct_control_plane_user(auth, *principal_context, tenant.id)?;

        if input.user_id.is_nil() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "user_id must not be nil",
            ));
        }

        if !has_effective_permission(&auth.permissions, &Permission::USERS_MANAGE) {
            return Err(<FieldError as GraphQLError>::permission_denied(
                "users:manage required to assign roles",
            ));
        }

        let enum_slug = input.role.map(|role| {
            let user_role: UserRole = role.into();
            user_role.to_string()
        });
        let slug = match (enum_slug, input.role_slug) {
            (Some(enum_slug), Some(raw_slug)) => {
                let slug = raw_slug.trim().to_lowercase();
                if slug != enum_slug {
                    return Err(<FieldError as GraphQLError>::bad_user_input(
                        "role and roleSlug must reference the same role",
                    ));
                }
                slug
            }
            (Some(enum_slug), None) => enum_slug,
            (None, Some(raw_slug)) => {
                let slug = raw_slug.trim().to_lowercase();
                let valid = (2..=64).contains(&slug.len())
                    && slug
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
                if !valid {
                    return Err(<FieldError as GraphQLError>::bad_user_input(
                        "roleSlug must be 2-64 characters of lowercase letters, numbers, underscores or hyphens",
                    ));
                }
                slug
            }
            (None, None) => {
                return Err(<FieldError as GraphQLError>::bad_user_input(
                    "either role or roleSlug is required",
                ));
            }
        };
        let writer = ctx.data::<RbacGraphqlRoleWriterHandle>()?;

        writer
            .0
            .replace_user_role_by_slug(&tenant.id, &auth.user_id, &input.user_id, &slug)
            .await
            .map_err(map_role_write_error)?;

        Ok(AssignUserRolePayload {
            success: true,
            user_id: input.user_id.to_string(),
            role: slug,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::parse_and_authorize_role_permissions;
    use rustok_api::Permission;

    #[test]
    fn role_unique_conflict_does_not_classify_formatted_diagnostics() {
        assert!(!super::is_unique_violation(&sea_orm::DbErr::Custom(
            "duplicate unique constraint 23505".into(),
        )));
    }

    #[tokio::test]
    async fn rejected_role_attempt_rolls_back_without_retry() {
        use sea_orm::{ConnectionTrait, TransactionTrait};
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE retry_role_probe (id INTEGER PRIMARY KEY)")
            .await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("INSERT INTO retry_role_probe VALUES (1)").await.unwrap();
        let error = super::RoleAttemptError::Statement(sea_orm::DbErr::Custom("55P03".into()));
        assert!(super::retry_role_attempt(tx, error, 1).await.is_err());
        let row = db.query_one_raw(sea_orm::Statement::from_string(
            db.get_database_backend(), "SELECT COUNT(*) AS cnt FROM retry_role_probe",
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "cnt").unwrap(), 0);
    }

    #[test]
    fn rejects_malformed_and_unknown_permissions() {
        let actor = vec![Permission::USERS_MANAGE];
        assert!(parse_and_authorize_role_permissions(&actor, &["foo:bar".to_string()]).is_err());
        assert!(parse_and_authorize_role_permissions(&actor, &["users".to_string()]).is_err());
    }

    #[test]
    fn rejects_grants_above_actor_authority() {
        let actor = vec![Permission::USERS_READ];
        assert!(
            parse_and_authorize_role_permissions(&actor, &["tenants:manage".to_string()]).is_err()
        );
        assert!(parse_and_authorize_role_permissions(&actor, &["users:read".to_string()]).is_ok());
    }

    #[test]
    fn accepts_multisegment_ai_permissions_within_authority() {
        let actor = vec![Permission::AI_PROVIDERS_MANAGE];
        let parsed =
            parse_and_authorize_role_permissions(&actor, &["ai:providers:manage".to_string()])
                .expect("canonical AI permission");
        assert_eq!(parsed, vec![Permission::AI_PROVIDERS_MANAGE]);
    }
}
