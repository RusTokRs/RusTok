use async_graphql::{Context, FieldError, InputObject, Object, Result};
use rustok_api::{
    AuthContext, AuthPrincipalContext, Permission, TenantContext, graphql::GraphQLError,
    has_effective_permission,
};
use rustok_core::UserRole;
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use uuid::Uuid;

use super::control_plane::{
    availability_snapshot, prepare_statement, require_direct_control_plane_user,
};
use super::types::{
    AssignUserRolePayload, CreateRoleInput, DeleteRolePayload, RbacGraphqlUserRole, RoleInfo,
    RoleMutationPayload, UpdateRoleInput,
};
use super::{RbacGraphqlRoleWriteError, RbacGraphqlRoleWriterHandle};

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

/// Map a storage failure to a generic internal error. Raw database errors
/// (SQL text, constraint names) are logged server-side and never returned to
/// GraphQL clients.
fn internal_db_error<E: std::fmt::Display>(operation: &str, error: E) -> FieldError {
    tracing::error!(%error, operation, "RBAC role mutation storage operation failed");
    <FieldError as GraphQLError>::internal_error("RBAC role storage is temporarily unavailable")
}

fn is_unique_violation<E: std::fmt::Display>(error: &E) -> bool {
    let message = error.to_string().to_lowercase();
    message.contains("unique") || message.contains("duplicate")
}

/// Parse role permissions against the canonical permission vocabulary and
/// enforce the actor's authority ceiling: a role may never be granted a
/// permission the actor does not effectively hold. Malformed strings are a
/// client error, never silently skipped.
fn parse_and_authorize_role_permissions(
    actor_permissions: &[Permission],
    raw_permissions: &[String],
) -> Result<Vec<Permission>> {
    let mut permissions: Vec<Permission> = Vec::with_capacity(raw_permissions.len());
    for raw in raw_permissions {
        let permission: Permission = raw.trim().parse().map_err(|_| {
            <FieldError as GraphQLError>::bad_user_input(&format!(
                "Invalid permission '{raw}': expected a canonical '<resource>:<action>' permission"
            ))
        })?;
        if !has_effective_permission(actor_permissions, &permission) {
            return Err(<FieldError as GraphQLError>::permission_denied(&format!(
                "cannot grant permission '{permission}' because it exceeds the current request authority"
            )));
        }
        if !permissions.contains(&permission) {
            permissions.push(permission);
        }
    }
    permissions.sort_by_cached_key(ToString::to_string);
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
    (permission.resource.to_string(), permission.action.to_string())
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
        if input
            .description
            .as_ref()
            .is_some_and(|value| value.chars().count() > 2000)
        {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role description must be at most 2000 characters",
            ));
        }
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
        let availability = availability_snapshot(ctx, tenant.id).await;
        reject_newly_dormant_grants(availability.as_ref(), &validated_permissions, &[])?;

        let role_id = rustok_core::generate_id();
        let tx = db
            .begin()
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;

        // The uniqueness check runs inside the creating transaction; the
        // UNIQUE (tenant_id, slug) index remains the final arbiter under
        // concurrency and its violation is reported as a clean conflict.
        let check_sql = "SELECT id FROM roles WHERE tenant_id = ? AND slug = ? LIMIT 1";
        let existing = tx
            .query_one_raw(prepare_statement(
                backend,
                check_sql,
                vec![tenant.id.into(), slug.clone().into()],
            ))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        if existing.is_some() {
            return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                "A role with slug '{slug}' already exists in this tenant"
            )));
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
                    input.description.clone().into(),
                ],
            ))
            .await
        {
            if is_unique_violation(&error) {
                return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                    "A role with slug '{slug}' already exists in this tenant"
                )));
            }
            return Err(internal_db_error("rbac role mutation", error));
        }

        let mut granted_permissions = Vec::new();
        for permission in &validated_permissions {
            let (resource, action) = permission_parts(permission);
            let (resource, action) = (resource.as_str(), action.as_str());

            let find_perm_sql = "SELECT id FROM permissions WHERE tenant_id = ? AND resource = ? AND action = ? LIMIT 1";
            let perm_row = tx
                .query_one_raw(prepare_statement(
                    backend,
                    find_perm_sql,
                    vec![tenant.id.into(), resource.into(), action.into()],
                ))
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;

            let perm_id = match perm_row {
                Some(row) => row
                    .try_get::<Uuid>("", "id")
                    .map_err(|e| internal_db_error("rbac role mutation", e))?,
                None => {
                    let new_perm_id = rustok_core::generate_id();
                    let ins_perm_sql = "INSERT INTO permissions (id, tenant_id, resource, action, description) VALUES (?, ?, ?, ?, NULL) ON CONFLICT (tenant_id, resource, action) DO NOTHING";
                    tx.execute_raw(prepare_statement(
                        backend,
                        ins_perm_sql,
                        vec![
                            new_perm_id.into(),
                            tenant.id.into(),
                            resource.into(),
                            action.into(),
                        ],
                    ))
                    .await
                    .map_err(|e| internal_db_error("rbac role mutation", e))?;

                    let persisted_row = tx
                        .query_one_raw(prepare_statement(
                            backend,
                            find_perm_sql,
                            vec![tenant.id.into(), resource.into(), action.into()],
                        ))
                        .await
                        .map_err(|e| internal_db_error("rbac role mutation", e))?
                        .ok_or_else(|| {
                            FieldError::new("Permission could not be retrieved after insert")
                        })?;
                    persisted_row
                        .try_get::<Uuid>("", "id")
                        .map_err(|e| internal_db_error("rbac role mutation", e))?
                }
            };

            let link_id = rustok_core::generate_id();
            let link_sql = "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?, ?, ?) ON CONFLICT (role_id, permission_id) DO NOTHING";
            tx.execute_raw(prepare_statement(
                backend,
                link_sql,
                vec![link_id.into(), role_id.into(), perm_id.into()],
            ))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;

            granted_permissions.push(format!("{resource}:{action}"));
        }

        tx.commit()
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        tracing::info!(actor_id = %auth.user_id, tenant_id = %tenant.id, role_slug = %slug, "rbac role created");

        granted_permissions.sort();
        granted_permissions.dedup();

        Ok(RoleMutationPayload {
            success: true,
            role: Some(RoleInfo {
                id: Some(role_id.to_string()),
                slug,
                display_name: name.to_string(),
                description: input.description,
                is_system: false,
                inactive_permissions: dormant_grant_strings(
                    availability.as_ref(),
                    &granted_permissions,
                ),
                permissions: granted_permissions,
            }),
        })
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
            .is_some_and(|value| value.trim().chars().count() > 255)
        {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role name must be at most 255 characters",
            ));
        }
        if input
            .description
            .as_ref()
            .is_some_and(|value| value.chars().count() > 2000)
        {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Role description must be at most 2000 characters",
            ));
        }
        let db = ctx.data::<DatabaseConnection>()?;
        let backend = db.get_database_backend();

        let find_role_sql = "SELECT id, name, description, is_system FROM roles WHERE tenant_id = ? AND slug = ? LIMIT 1";
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
        let current_name: String = existing
            .try_get("", "name")
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        let current_desc: Option<String> = existing.try_get("", "description").ok();
        let is_system: bool = existing.try_get("", "is_system").unwrap_or(false);

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

        let validated_update_permissions = match &input.permissions {
            Some(raw) => Some(parse_and_authorize_role_permissions(&auth.permissions, raw)?),
            None => None,
        };

        // Dormant grants already on the role are preserved across saves;
        // adding a *new* grant of a disabled module is refused.
        let availability = availability_snapshot(ctx, tenant.id).await;
        if let Some(new_permissions) = validated_update_permissions.as_ref() {
            let current_rows = db
                .query_all_raw(prepare_statement(
                    backend,
                    "SELECT p.resource, p.action FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id WHERE rp.role_id = ?",
                    vec![role_id.into()],
                ))
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;
            let mut current_grants = Vec::with_capacity(current_rows.len());
            for row in current_rows {
                if let (Ok(resource), Ok(action)) = (
                    row.try_get::<String>("", "resource"),
                    row.try_get::<String>("", "action"),
                ) {
                    current_grants.push(format!("{resource}:{action}"));
                }
            }
            reject_newly_dormant_grants(availability.as_ref(), new_permissions, &current_grants)?;
        }

        // Resolve the invalidation writer up front (when the host registered
        // one) so post-commit invalidation never has to discover the runtime.
        // Hosts that register no writer run without a permission cache, so
        // there is nothing to invalidate.
        let role_writer = if validated_update_permissions.is_some() {
            ctx.data_opt::<RbacGraphqlRoleWriterHandle>()
                .map(|handle| handle.0.clone())
        } else {
            None
        };

        let tx = db
            .begin()
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;

        // Re-read the role inside the transaction: the pre-transaction lookup
        // above only shapes error reporting. If the role was deleted (or
        // deleted and recreated) concurrently, fail instead of writing grant
        // rows for a role that no longer exists.
        let tx_role = tx
            .query_one_raw(prepare_statement(
                backend,
                "SELECT id FROM roles WHERE tenant_id = ? AND id = ? LIMIT 1",
                vec![tenant.id.into(), role_id.into()],
            ))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        if tx_role.is_none() {
            return Err(<FieldError as GraphQLError>::not_found(&format!(
                "Role '{slug}' not found"
            )));
        }

        let new_name = input
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or(current_name);
        let new_desc = input.description.or(current_desc);

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
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        if update_result.rows_affected() != 1 {
            return Err(<FieldError as GraphQLError>::not_found(&format!(
                "Role '{slug}' not found"
            )));
        }

        let permissions_updated = input.permissions.is_some();
        let mut final_permissions = Vec::new();
        if let Some(new_permissions) = validated_update_permissions {
            let del_links_sql = "DELETE FROM role_permissions WHERE role_id = ?";
            tx.execute_raw(prepare_statement(
                backend,
                del_links_sql,
                vec![role_id.into()],
            ))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;

            for permission in &new_permissions {
                let (resource, action) = permission_parts(permission);
                let (resource, action) = (resource.as_str(), action.as_str());

                let find_perm_sql = "SELECT id FROM permissions WHERE tenant_id = ? AND resource = ? AND action = ? LIMIT 1";
                let perm_row = tx
                    .query_one_raw(prepare_statement(
                        backend,
                        find_perm_sql,
                        vec![tenant.id.into(), resource.into(), action.into()],
                    ))
                    .await
                    .map_err(|e| internal_db_error("rbac role mutation", e))?;

                let perm_id = match perm_row {
                    Some(row) => row
                        .try_get::<Uuid>("", "id")
                        .map_err(|e| internal_db_error("rbac role mutation", e))?,
                    None => {
                        let new_perm_id = rustok_core::generate_id();
                        let ins_perm_sql = "INSERT INTO permissions (id, tenant_id, resource, action, description) VALUES (?, ?, ?, ?, NULL) ON CONFLICT (tenant_id, resource, action) DO NOTHING";
                        tx.execute_raw(prepare_statement(
                            backend,
                            ins_perm_sql,
                            vec![
                                new_perm_id.into(),
                                tenant.id.into(),
                                resource.into(),
                                action.into(),
                            ],
                        ))
                        .await
                        .map_err(|e| internal_db_error("rbac role mutation", e))?;

                        let persisted_row = tx
                            .query_one_raw(prepare_statement(
                                backend,
                                find_perm_sql,
                                vec![tenant.id.into(), resource.into(), action.into()],
                            ))
                            .await
                            .map_err(|e| internal_db_error("rbac role mutation", e))?
                            .ok_or_else(|| {
                                FieldError::new("Permission could not be retrieved after insert")
                            })?;
                        persisted_row
                            .try_get::<Uuid>("", "id")
                            .map_err(|e| internal_db_error("rbac role mutation", e))?
                    }
                };

                let link_id = rustok_core::generate_id();
                let link_sql = "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?, ?, ?) ON CONFLICT (role_id, permission_id) DO NOTHING";
                tx.execute_raw(prepare_statement(
                    backend,
                    link_sql,
                    vec![link_id.into(), role_id.into(), perm_id.into()],
                ))
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;

                final_permissions.push(format!("{resource}:{action}"));
            }
        } else {
            let cur_perms_sql = "SELECT p.resource, p.action FROM role_permissions rp JOIN permissions p ON p.id = rp.permission_id WHERE rp.role_id = ?";
            let rows = tx
                .query_all_raw(prepare_statement(
                    backend,
                    cur_perms_sql,
                    vec![role_id.into()],
                ))
                .await
                .map_err(|e| internal_db_error("rbac role mutation", e))?;
            for row in rows {
                if let (Ok(res), Ok(act)) = (
                    row.try_get::<String>("", "resource"),
                    row.try_get::<String>("", "action"),
                ) {
                    final_permissions.push(format!("{res}:{act}"));
                }
            }
        }

        tx.commit()
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;

        // The grant change is already committed at this point, so an
        // invalidation failure must not rewrite the mutation outcome into
        // an error the client would retry as if nothing happened. Retry
        // once, then log loudly: any remaining staleness is bounded by
        // the permission-cache TTL.
        if permissions_updated
            && let Some(writer) = role_writer.as_ref()
            && writer.invalidate_role_users(&tenant.id, &role_id).await.is_err()
            && let Err(error) = writer.invalidate_role_users(&tenant.id, &role_id).await
        {
            tracing::error!(
                %error,
                tenant_id = %tenant.id,
                role_id = %role_id,
                "RBAC role permission change committed but cache invalidation failed; stale snapshots expire with the permission-cache TTL"
            );
        }

        tracing::info!(actor_id = %auth.user_id, tenant_id = %tenant.id, role_slug = %slug, permissions_updated, "rbac role updated");
        final_permissions.sort();
        final_permissions.dedup();

        Ok(RoleMutationPayload {
            success: true,
            role: Some(RoleInfo {
                id: Some(role_id.to_string()),
                slug,
                display_name: new_name,
                description: new_desc,
                is_system,
                inactive_permissions: dormant_grant_strings(
                    availability.as_ref(),
                    &final_permissions,
                ),
                permissions: final_permissions,
            }),
        })
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

        let tx = db
            .begin()
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;

        // Re-verify the role inside the deleting transaction: the lookup
        // above ran outside it, so a concurrent delete must surface as
        // not-found here instead of a phantom success below.
        let tx_role = tx
            .query_one_raw(prepare_statement(
                backend,
                "SELECT id FROM roles WHERE tenant_id = ? AND id = ? LIMIT 1",
                vec![tenant.id.into(), role_id.into()],
            ))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        if tx_role.is_none() {
            return Err(<FieldError as GraphQLError>::not_found(&format!(
                "Role '{slug}' not found"
            )));
        }

        // Integrity invariant, checked inside the deleting transaction so a
        // concurrent assignment cannot race past it (no TOCTOU window), and a
        // failed count read fails closed instead of reading as zero.
        let count_sql = "SELECT COUNT(*) as cnt FROM user_roles WHERE role_id = ?";
        let count_row = tx
            .query_one_raw(prepare_statement(backend, count_sql, vec![role_id.into()]))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?
            .ok_or_else(|| FieldError::new("Role assignment count query returned no row"))?;
        let count: i64 = count_row
            .try_get("", "cnt")
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        if count > 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                "Cannot delete role '{slug}' because it is currently assigned to {count} user(s). Reassign them first."
            )));
        }

        let del_links_sql = "DELETE FROM role_permissions WHERE role_id = ?";
        tx.execute_raw(prepare_statement(
            backend,
            del_links_sql,
            vec![role_id.into()],
        ))
        .await
        .map_err(|e| internal_db_error("rbac role mutation", e))?;

        // Artifact grant cleanup is part of the same transaction: if it
        // fails, the whole delete rolls back instead of leaving orphaned
        // artifact permissions pointing at a deleted role.
        let del_artifact_links_sql = "DELETE FROM rbac_artifact_role_permissions WHERE role_id = ?";
        tx.execute_raw(prepare_statement(
            backend,
            del_artifact_links_sql,
            vec![role_id.into()],
        ))
        .await
        .map_err(|e| internal_db_error("rbac role mutation", e))?;

        let del_artifact_ops_sql =
            "DELETE FROM rbac_artifact_role_permission_operations WHERE role_id = ?";
        tx.execute_raw(prepare_statement(
            backend,
            del_artifact_ops_sql,
            vec![role_id.into()],
        ))
        .await
        .map_err(|e| internal_db_error("rbac role mutation", e))?;

        let del_role_sql = "DELETE FROM roles WHERE id = ?";
        let delete_result = tx
            .execute_raw(prepare_statement(
                backend,
                del_role_sql,
                vec![role_id.into()],
            ))
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        if delete_result.rows_affected() != 1 {
            return Err(<FieldError as GraphQLError>::not_found(&format!(
                "Role '{slug}' not found"
            )));
        }

        tx.commit()
            .await
            .map_err(|e| internal_db_error("rbac role mutation", e))?;
        tracing::info!(actor_id = %auth.user_id, tenant_id = %tenant.id, role_slug = %slug, "rbac role deleted");

        Ok(DeleteRolePayload {
            success: true,
            slug,
        })
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
    fn rejects_malformed_and_unknown_permissions() {
        let actor = vec![Permission::USERS_MANAGE];
        assert!(parse_and_authorize_role_permissions(&actor, &["foo:bar".to_string()]).is_err());
        assert!(parse_and_authorize_role_permissions(&actor, &["users".to_string()]).is_err());
    }

    #[test]
    fn rejects_grants_above_actor_authority() {
        let actor = vec![Permission::USERS_READ];
        assert!(parse_and_authorize_role_permissions(&actor, &["tenants:manage".to_string()]).is_err());
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
