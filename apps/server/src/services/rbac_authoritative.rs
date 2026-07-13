use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::models::_entities::{permissions, role_permissions, roles, user_roles};
use rustok_api::{Action, Permission, Resource};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};

use super::rbac_service::RbacService;

impl RbacService {
    /// Resolve the canonical database-backed permission snapshot without using
    /// the process-local authorization cache.
    ///
    /// Authentication must observe role revocation and demotion immediately.
    /// Authorization entry points use `get_user_permissions`, which honors the
    /// immutable request scope and may use the runtime cache outside a request.
    pub async fn get_user_permissions_authoritative(
        db: &DatabaseConnection,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>> {
        let assigned_roles = user_roles::Entity::find()
            .filter(user_roles::Column::UserId.eq(*user_id))
            .all(db)
            .await?;
        if assigned_roles.is_empty() {
            return Ok(Vec::new());
        }

        let assigned_role_ids = assigned_roles
            .into_iter()
            .map(|assignment| assignment.role_id)
            .collect::<Vec<_>>();
        let tenant_roles = roles::Entity::find()
            .filter(roles::Column::TenantId.eq(*tenant_id))
            .filter(roles::Column::Id.is_in(assigned_role_ids))
            .all(db)
            .await?;
        if tenant_roles.is_empty() {
            return Ok(Vec::new());
        }

        let tenant_role_ids = tenant_roles
            .into_iter()
            .map(|role| role.id)
            .collect::<Vec<_>>();
        let links = role_permissions::Entity::find()
            .filter(role_permissions::Column::RoleId.is_in(tenant_role_ids))
            .all(db)
            .await?;
        if links.is_empty() {
            return Ok(Vec::new());
        }

        let permission_ids = links
            .into_iter()
            .map(|link| link.permission_id)
            .collect::<Vec<_>>();
        let rows = permissions::Entity::find()
            .filter(permissions::Column::TenantId.eq(*tenant_id))
            .filter(permissions::Column::Id.is_in(permission_ids))
            .all(db)
            .await?;

        let mut seen = HashSet::new();
        let mut resolved = Vec::with_capacity(rows.len());
        for row in rows {
            let resource = row
                .resource
                .parse::<Resource>()
                .map_err(Error::BadRequest)?;
            let action = row.action.parse::<Action>().map_err(Error::BadRequest)?;
            let permission = Permission::new(resource, action);
            if seen.insert(permission) {
                resolved.push(permission);
            }
        }

        Ok(resolved)
    }
}
