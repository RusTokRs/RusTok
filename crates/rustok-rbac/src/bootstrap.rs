use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use thiserror::Error;
use uuid::Uuid;

use rustok_api::Permission;
use rustok_core::{Rbac, UserRole};

#[derive(Debug, Error)]
pub enum RbacRoleAssignmentError {
    #[error("RBAC role assignment database error: {0}")]
    Database(String),
    #[error("RBAC role assignment did not persist {0}")]
    MissingPersistedRecord(&'static str),
}

/// Database-backed writer for idempotent built-in role assignment.
///
/// The writer owns the roles, permissions and relation-table persistence rules.
/// Host runtimes remain responsible only for invalidating any process-local
/// authorization caches after a successful assignment.
pub struct RbacRoleAssignmentDbWriter {
    db: DatabaseConnection,
}

impl RbacRoleAssignmentDbWriter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn assign_role_permissions(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: UserRole,
    ) -> Result<(), RbacRoleAssignmentError> {
        Self::assign_role_permissions_on(&self.db, tenant_id, user_id, role).await
    }

    /// Execute role assignment on an existing SeaORM connection or transaction.
    ///
    /// This keeps the persistence operation inside the caller's transaction;
    /// process-local cache invalidation remains a post-commit host concern.
    pub async fn assign_role_permissions_on<C>(
        db: &C,
        tenant_id: Uuid,
        user_id: Uuid,
        role: UserRole,
    ) -> Result<(), RbacRoleAssignmentError>
    where
        C: ConnectionTrait,
    {
        ConnectionRoleAssignmentWriter { db }
            .assign_role_permissions(tenant_id, user_id, role)
            .await
    }
}

struct ConnectionRoleAssignmentWriter<'a, C>
where
    C: ConnectionTrait,
{
    db: &'a C,
}

impl<C> ConnectionRoleAssignmentWriter<'_, C>
where
    C: ConnectionTrait,
{
    async fn assign_role_permissions(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: UserRole,
    ) -> Result<(), RbacRoleAssignmentError> {
        let role_id = self.ensure_role(tenant_id, &role).await?;
        self.ensure_user_role(user_id, role_id).await?;

        for permission in Rbac::permissions_for_role(&role) {
            let permission_id = self.ensure_permission(tenant_id, permission).await?;
            self.ensure_role_permission(role_id, permission_id).await?;
        }

        Ok(())
    }

    async fn ensure_role(
        &self,
        tenant_id: Uuid,
        role: &UserRole,
    ) -> Result<Uuid, RbacRoleAssignmentError> {
        let slug = role.to_string();
        if let Some(id) = self
            .find_id(
                "SELECT id FROM roles WHERE tenant_id = {tenant} AND slug = {slug} LIMIT 1",
                tenant_id,
                &slug,
            )
            .await?
        {
            return Ok(id);
        }

        self.execute(
            "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES ({id}, {tenant}, {name}, {slug}, NULL, TRUE) ON CONFLICT (tenant_id, slug) DO NOTHING",
            vec![
                rustok_core::generate_id().into(),
                tenant_id.into(),
                slug.clone().into(),
                slug.clone().into(),
            ],
        )
        .await?;

        self.find_id(
            "SELECT id FROM roles WHERE tenant_id = {tenant} AND slug = {slug} LIMIT 1",
            tenant_id,
            &slug,
        )
        .await?
        .ok_or(RbacRoleAssignmentError::MissingPersistedRecord("role"))
    }

    async fn ensure_permission(
        &self,
        tenant_id: Uuid,
        permission: &Permission,
    ) -> Result<Uuid, RbacRoleAssignmentError> {
        let resource = permission.resource.to_string();
        let action = permission.action.to_string();
        let backend = self.db.get_database_backend();
        let select = match backend {
            DbBackend::Sqlite => "SELECT id FROM permissions WHERE tenant_id = ?1 AND resource = ?2 AND action = ?3 LIMIT 1",
            _ => "SELECT id FROM permissions WHERE tenant_id = $1 AND resource = $2 AND action = $3 LIMIT 1",
        };
        if let Some(id) = self
            .query_id(
                select,
                vec![
                    tenant_id.into(),
                    resource.clone().into(),
                    action.clone().into(),
                ],
            )
            .await?
        {
            return Ok(id);
        }

        self.execute(
            "INSERT INTO permissions (id, tenant_id, resource, action, description) VALUES ({id}, {tenant}, {resource}, {action}, NULL) ON CONFLICT (tenant_id, resource, action) DO NOTHING",
            vec![
                rustok_core::generate_id().into(),
                tenant_id.into(),
                resource.clone().into(),
                action.clone().into(),
            ],
        )
        .await?;

        self.query_id(
            select,
            vec![tenant_id.into(), resource.into(), action.into()],
        )
        .await?
        .ok_or(RbacRoleAssignmentError::MissingPersistedRecord(
            "permission",
        ))
    }

    async fn ensure_user_role(
        &self,
        user_id: Uuid,
        role_id: Uuid,
    ) -> Result<(), RbacRoleAssignmentError> {
        self.execute(
            "INSERT INTO user_roles (id, user_id, role_id) VALUES ({id}, {user}, {role}) ON CONFLICT (user_id, role_id) DO NOTHING",
            vec![rustok_core::generate_id().into(), user_id.into(), role_id.into()],
        )
        .await
    }

    async fn ensure_role_permission(
        &self,
        role_id: Uuid,
        permission_id: Uuid,
    ) -> Result<(), RbacRoleAssignmentError> {
        self.execute(
            "INSERT INTO role_permissions (id, role_id, permission_id) VALUES ({id}, {role}, {permission}) ON CONFLICT (role_id, permission_id) DO NOTHING",
            vec![
                rustok_core::generate_id().into(),
                role_id.into(),
                permission_id.into(),
            ],
        )
        .await
    }

    async fn find_id(
        &self,
        template: &str,
        tenant_id: Uuid,
        slug: &str,
    ) -> Result<Option<Uuid>, RbacRoleAssignmentError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => template.replace("{tenant}", "?1").replace("{slug}", "?2"),
            _ => template.replace("{tenant}", "$1").replace("{slug}", "$2"),
        };
        self.query_id(sql.as_str(), vec![tenant_id.into(), slug.into()])
            .await
    }

    async fn execute(
        &self,
        template: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<(), RbacRoleAssignmentError> {
        let backend = self.db.get_database_backend();
        let sql = render_insert_sql(template, backend);
        self.db
            .execute(Statement::from_sql_and_values(backend, sql, values))
            .await
            .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))?;
        Ok(())
    }

    async fn query_id(
        &self,
        sql: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<Option<Uuid>, RbacRoleAssignmentError> {
        self.db
            .query_one(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                sql,
                values,
            ))
            .await
            .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))?
            .map(|row| {
                row.try_get("", "id")
                    .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))
            })
            .transpose()
    }
}

fn render_insert_sql(template: &str, backend: DbBackend) -> String {
    let markers = match backend {
        DbBackend::Sqlite => ["?1", "?2", "?3", "?4"],
        _ => ["$1", "$2", "$3", "$4"],
    };
    template
        .replace("{id}", markers[0])
        .replace("{tenant}", markers[1])
        .replace("{name}", markers[2])
        .replace("{slug}", markers[3])
        .replace("{resource}", markers[2])
        .replace("{action}", markers[3])
        .replace("{user}", markers[1])
        .replace("{role}", markers[2])
        .replace("{permission}", markers[2])
}