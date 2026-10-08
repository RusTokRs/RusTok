use async_graphql::{Context, FieldError, InputObject, Object, Result};
use rustok_api::{
    AuthContext, AuthPrincipalContext, Permission, TenantContext, graphql::GraphQLError,
    has_effective_permission,
};
use rustok_core::UserRole;
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionTrait};
use uuid::Uuid;

use super::control_plane::{prepare_statement, require_direct_control_plane_user};
use super::types::{
    AssignUserRolePayload, CreateRoleInput, DeleteRolePayload, RbacGraphqlUserRole, RoleInfo,
    RoleMutationPayload, UpdateRoleInput,
};
use super::{RbacGraphqlRoleWriteError, RbacGraphqlRoleWriterHandle};

#[derive(InputObject)]
pub struct AssignUserRoleInput {
    pub user_id: Uuid,
    pub role: RbacGraphqlUserRole,
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

        let check_sql = "SELECT id FROM roles WHERE tenant_id = ? AND slug = ? LIMIT 1";
        let existing = db
            .query_one_raw(prepare_statement(
                backend,
                check_sql,
                vec![tenant.id.into(), slug.clone().into()],
            ))
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;
        if existing.is_some() {
            return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                "A role with slug '{slug}' already exists in this tenant"
            )));
        }

        let role_id = rustok_core::generate_id();
        let tx = db
            .begin()
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        let insert_role_sql = "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES (?, ?, ?, ?, ?, FALSE)";
        tx.execute_raw(prepare_statement(
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
        .map_err(|e| FieldError::new(e.to_string()))?;

        let mut granted_permissions = Vec::new();
        for perm_str in &input.permissions {
            let parts: Vec<&str> = perm_str.split(':').collect();
            if parts.len() != 2 {
                continue;
            }
            let (resource, action) = (parts[0].trim(), parts[1].trim());
            if resource.is_empty() || action.is_empty() {
                continue;
            }

            let find_perm_sql = "SELECT id FROM permissions WHERE tenant_id = ? AND resource = ? AND action = ? LIMIT 1";
            let perm_row = tx
                .query_one_raw(prepare_statement(
                    backend,
                    find_perm_sql,
                    vec![tenant.id.into(), resource.into(), action.into()],
                ))
                .await
                .map_err(|e| FieldError::new(e.to_string()))?;

            let perm_id = match perm_row {
                Some(row) => row
                    .try_get::<Uuid>("", "id")
                    .map_err(|e| FieldError::new(e.to_string()))?,
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
                    .map_err(|e| FieldError::new(e.to_string()))?;
                    new_perm_id
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
            .map_err(|e| FieldError::new(e.to_string()))?;

            granted_permissions.push(format!("{resource}:{action}"));
        }

        tx.commit()
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

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
            .map_err(|e| FieldError::new(e.to_string()))?
            .ok_or_else(|| {
                <FieldError as GraphQLError>::not_found(&format!("Role '{slug}' not found"))
            })?;

        let role_id: Uuid = existing
            .try_get("", "id")
            .map_err(|e| FieldError::new(e.to_string()))?;
        let current_name: String = existing
            .try_get("", "name")
            .map_err(|e| FieldError::new(e.to_string()))?;
        let current_desc: Option<String> = existing.try_get("", "description").ok();
        let is_system: bool = existing.try_get("", "is_system").unwrap_or(false);

        // Core platform invariant: Super Administrator permissions cannot be modified or reduced
        if slug == "super_admin" && input.permissions.is_some() {
            return Err(<FieldError as GraphQLError>::bad_user_input(
                "Super administrator permissions are immutable and cannot be restricted",
            ));
        }

        let tx = db
            .begin()
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        let new_name = input
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or(current_name);
        let new_desc = input.description.or(current_desc);

        let update_role_sql = "UPDATE roles SET name = ?, description = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?";
        tx.execute_raw(prepare_statement(
            backend,
            update_role_sql,
            vec![
                new_name.clone().into(),
                new_desc.clone().into(),
                role_id.into(),
            ],
        ))
        .await
        .map_err(|e| FieldError::new(e.to_string()))?;

        let mut final_permissions = Vec::new();
        if let Some(new_permissions) = input.permissions {
            let del_links_sql = "DELETE FROM role_permissions WHERE role_id = ?";
            tx.execute_raw(prepare_statement(
                backend,
                del_links_sql,
                vec![role_id.into()],
            ))
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

            for perm_str in &new_permissions {
                let parts: Vec<&str> = perm_str.split(':').collect();
                if parts.len() != 2 {
                    continue;
                }
                let (resource, action) = (parts[0].trim(), parts[1].trim());
                if resource.is_empty() || action.is_empty() {
                    continue;
                }

                let find_perm_sql = "SELECT id FROM permissions WHERE tenant_id = ? AND resource = ? AND action = ? LIMIT 1";
                let perm_row = tx
                    .query_one_raw(prepare_statement(
                        backend,
                        find_perm_sql,
                        vec![tenant.id.into(), resource.into(), action.into()],
                    ))
                    .await
                    .map_err(|e| FieldError::new(e.to_string()))?;

                let perm_id = match perm_row {
                    Some(row) => row
                        .try_get::<Uuid>("", "id")
                        .map_err(|e| FieldError::new(e.to_string()))?,
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
                        .map_err(|e| FieldError::new(e.to_string()))?;
                        new_perm_id
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
                .map_err(|e| FieldError::new(e.to_string()))?;

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
                .map_err(|e| FieldError::new(e.to_string()))?;
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
            .map_err(|e| FieldError::new(e.to_string()))?;

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
                permissions: final_permissions,
            }),
        })
    }

    /// Delete a custom role.
    /// Built-in system roles can never be deleted. Roles assigned to active users cannot be deleted.
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

        let find_role_sql =
            "SELECT id, is_system FROM roles WHERE tenant_id = ? AND slug = ? LIMIT 1";
        let existing = db
            .query_one_raw(prepare_statement(
                backend,
                find_role_sql,
                vec![tenant.id.into(), slug.clone().into()],
            ))
            .await
            .map_err(|e| FieldError::new(e.to_string()))?
            .ok_or_else(|| {
                <FieldError as GraphQLError>::not_found(&format!("Role '{slug}' not found"))
            })?;

        let role_id: Uuid = existing
            .try_get("", "id")
            .map_err(|e| FieldError::new(e.to_string()))?;
        let is_system: bool = existing.try_get("", "is_system").unwrap_or(false);

        // Core platform invariant: Built-in system roles must NEVER be deleted
        let reserved = ["super_admin", "admin", "manager", "customer"];
        if is_system || reserved.contains(&slug.as_str()) {
            return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                "Cannot delete built-in system role '{slug}'"
            )));
        }

        // Integrity invariant: Reject deletion if role is currently assigned to users
        let count_sql = "SELECT COUNT(*) as cnt FROM user_roles WHERE role_id = ?";
        let count_row = db
            .query_one_raw(prepare_statement(backend, count_sql, vec![role_id.into()]))
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;
        let count: i64 = count_row
            .and_then(|r| r.try_get("", "cnt").ok())
            .unwrap_or(0);
        if count > 0 {
            return Err(<FieldError as GraphQLError>::bad_user_input(&format!(
                "Cannot delete role '{slug}' because it is currently assigned to {count} user(s). Reassign them first."
            )));
        }

        let tx = db
            .begin()
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        let del_links_sql = "DELETE FROM role_permissions WHERE role_id = ?";
        tx.execute_raw(prepare_statement(
            backend,
            del_links_sql,
            vec![role_id.into()],
        ))
        .await
        .map_err(|e| FieldError::new(e.to_string()))?;

        let del_role_sql = "DELETE FROM roles WHERE id = ?";
        tx.execute_raw(prepare_statement(
            backend,
            del_role_sql,
            vec![role_id.into()],
        ))
        .await
        .map_err(|e| FieldError::new(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| FieldError::new(e.to_string()))?;

        Ok(DeleteRolePayload {
            success: true,
            slug,
        })
    }

    /// Assign a role to a user (replaces the current role).
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

        let user_role: UserRole = input.role.into();
        let role = user_role.to_string();
        let writer = ctx.data::<RbacGraphqlRoleWriterHandle>()?;

        writer
            .0
            .replace_user_role(&tenant.id, &auth.user_id, &input.user_id, user_role)
            .await
            .map_err(map_role_write_error)?;

        Ok(AssignUserRolePayload {
            success: true,
            user_id: input.user_id.to_string(),
            role,
        })
    }
}
