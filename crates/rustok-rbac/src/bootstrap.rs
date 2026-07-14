use std::collections::HashSet;

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
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
    #[error(
        "RBAC user {user_id} belongs to tenant {actual_tenant_id}, not {expected_tenant_id}"
    )]
    UserTenantMismatch {
        user_id: Uuid,
        expected_tenant_id: Uuid,
        actual_tenant_id: Uuid,
    },
    #[error("RBAC role assignment does not support database backend {0}")]
    UnsupportedBackend(&'static str),
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

    /// Assign and reconcile a built-in role atomically.
    ///
    /// Standalone callers receive an all-or-nothing transaction. Hosts that
    /// already own a wider transaction should call `assign_role_permissions_on`
    /// instead and invalidate process-local authorization caches after commit.
    pub async fn assign_role_permissions(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: UserRole,
    ) -> Result<(), RbacRoleAssignmentError> {
        ensure_supported_backend(self.db.get_database_backend())?;
        let tx = self
            .db
            .begin()
            .await
            .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))?;
        let result = Self::assign_role_permissions_on(&tx, tenant_id, user_id, role).await;

        match result {
            Ok(()) => tx
                .commit()
                .await
                .map_err(|error| RbacRoleAssignmentError::Database(error.to_string())),
            Err(error) => {
                tx.rollback().await.map_err(|rollback_error| {
                    RbacRoleAssignmentError::Database(format!(
                        "role assignment failed: {error}; rollback failed: {rollback_error}"
                    ))
                })?;
                Err(error)
            }
        }
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
        ensure_supported_backend(db.get_database_backend())?;
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
        self.ensure_user_tenant(tenant_id, user_id).await?;
        let role_id = self.ensure_role(tenant_id, &role).await?;
        let mut expected_permission_ids = HashSet::new();

        for permission in Rbac::permissions_for_role(&role) {
            let permission_id = self.ensure_permission(tenant_id, permission).await?;
            self.ensure_role_permission(role_id, permission_id).await?;
            expected_permission_ids.insert(permission_id);
        }

        self.remove_stale_role_permissions(role_id, &expected_permission_ids)
            .await?;
        self.ensure_user_role(user_id, role_id).await?;

        Ok(())
    }

    async fn ensure_user_tenant(
        &self,
        expected_tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), RbacRoleAssignmentError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => "SELECT tenant_id FROM users WHERE id = ?1 LIMIT 1",
            DbBackend::Postgres | DbBackend::MySql => {
                "SELECT tenant_id FROM users WHERE id = $1 LIMIT 1"
            }
        };
        let actual_tenant_id = self
            .query_uuid(sql, "tenant_id", vec![user_id.into()])
            .await?
            .ok_or(RbacRoleAssignmentError::MissingPersistedRecord("user"))?;

        if actual_tenant_id != expected_tenant_id {
            return Err(RbacRoleAssignmentError::UserTenantMismatch {
                user_id,
                expected_tenant_id,
                actual_tenant_id,
            });
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
            DbBackend::Postgres | DbBackend::MySql => "SELECT id FROM permissions WHERE tenant_id = $1 AND resource = $2 AND action = $3 LIMIT 1",
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
            "INSERT INTO role_permissions (id, role_id, permission_id) VALUES ({id}, {relation_role}, {permission}) ON CONFLICT (role_id, permission_id) DO NOTHING",
            vec![
                rustok_core::generate_id().into(),
                role_id.into(),
                permission_id.into(),
            ],
        )
        .await
    }

    async fn remove_stale_role_permissions(
        &self,
        role_id: Uuid,
        expected_permission_ids: &HashSet<Uuid>,
    ) -> Result<(), RbacRoleAssignmentError> {
        let existing_permission_ids = self.load_role_permission_ids(role_id).await?;
        for permission_id in
            stale_role_permission_ids(existing_permission_ids, expected_permission_ids)
        {
            self.delete_role_permission(role_id, permission_id).await?;
        }
        Ok(())
    }

    async fn load_role_permission_ids(
        &self,
        role_id: Uuid,
    ) -> Result<Vec<Uuid>, RbacRoleAssignmentError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => {
                "SELECT permission_id FROM role_permissions WHERE role_id = ?1"
            }
            DbBackend::Postgres | DbBackend::MySql => {
                "SELECT permission_id FROM role_permissions WHERE role_id = $1"
            }
        };
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                backend,
                sql,
                vec![role_id.into()],
            ))
            .await
            .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))?;

        rows.into_iter()
            .map(|row| {
                row.try_get("", "permission_id")
                    .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))
            })
            .collect()
    }

    async fn delete_role_permission(
        &self,
        role_id: Uuid,
        permission_id: Uuid,
    ) -> Result<(), RbacRoleAssignmentError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => {
                "DELETE FROM role_permissions WHERE role_id = ?1 AND permission_id = ?2"
            }
            DbBackend::Postgres | DbBackend::MySql => {
                "DELETE FROM role_permissions WHERE role_id = $1 AND permission_id = $2"
            }
        };
        self.db
            .execute(Statement::from_sql_and_values(
                backend,
                sql,
                vec![role_id.into(), permission_id.into()],
            ))
            .await
            .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))?;
        Ok(())
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
            DbBackend::Postgres | DbBackend::MySql => {
                template.replace("{tenant}", "$1").replace("{slug}", "$2")
            }
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
        self.query_uuid(sql, "id", values).await
    }

    async fn query_uuid(
        &self,
        sql: &str,
        column: &str,
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
                row.try_get("", column)
                    .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))
            })
            .transpose()
    }
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), RbacRoleAssignmentError> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        DbBackend::MySql => Err(RbacRoleAssignmentError::UnsupportedBackend("mysql")),
    }
}

fn stale_role_permission_ids(
    existing_permission_ids: Vec<Uuid>,
    expected_permission_ids: &HashSet<Uuid>,
) -> Vec<Uuid> {
    existing_permission_ids
        .into_iter()
        .filter(|permission_id| !expected_permission_ids.contains(permission_id))
        .collect()
}

fn render_insert_sql(template: &str, backend: DbBackend) -> String {
    let markers = match backend {
        DbBackend::Sqlite => ["?1", "?2", "?3", "?4"],
        DbBackend::Postgres | DbBackend::MySql => ["$1", "$2", "$3", "$4"],
    };
    template
        .replace("{id}", markers[0])
        .replace("{tenant}", markers[1])
        .replace("{name}", markers[2])
        .replace("{slug}", markers[3])
        .replace("{resource}", markers[2])
        .replace("{action}", markers[3])
        .replace("{user}", markers[1])
        .replace("{relation_role}", markers[1])
        .replace("{role}", markers[2])
        .replace("{permission}", markers[2])
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{ensure_supported_backend, render_insert_sql, stale_role_permission_ids};
    use sea_orm::DbBackend;
    use uuid::Uuid;

    const ROLE_PERMISSION_INSERT: &str =
        "INSERT INTO role_permissions (id, role_id, permission_id) VALUES ({id}, {relation_role}, {permission}) ON CONFLICT (role_id, permission_id) DO NOTHING";
    const USER_ROLE_INSERT: &str =
        "INSERT INTO user_roles (id, user_id, role_id) VALUES ({id}, {user}, {role}) ON CONFLICT (user_id, role_id) DO NOTHING";

    #[test]
    fn postgres_role_permission_markers_bind_distinct_ids() {
        let rendered = render_insert_sql(ROLE_PERMISSION_INSERT, DbBackend::Postgres);

        assert!(rendered.contains("VALUES ($1, $2, $3)"));
        assert!(!rendered.contains("VALUES ($1, $3, $3)"));
    }

    #[test]
    fn sqlite_role_permission_markers_bind_distinct_ids() {
        let rendered = render_insert_sql(ROLE_PERMISSION_INSERT, DbBackend::Sqlite);

        assert!(rendered.contains("VALUES (?1, ?2, ?3)"));
        assert!(!rendered.contains("VALUES (?1, ?3, ?3)"));
    }

    #[test]
    fn user_role_markers_keep_user_and_role_positions() {
        assert!(render_insert_sql(USER_ROLE_INSERT, DbBackend::Postgres)
            .contains("VALUES ($1, $2, $3)"));
        assert!(render_insert_sql(USER_ROLE_INSERT, DbBackend::Sqlite)
            .contains("VALUES (?1, ?2, ?3)"));
    }

    #[test]
    fn stale_permission_detection_removes_only_unexpected_links() {
        let retained = Uuid::new_v4();
        let stale = Uuid::new_v4();
        let expected = HashSet::from([retained]);

        assert_eq!(
            stale_role_permission_ids(vec![retained, stale], &expected),
            vec![stale]
        );
    }

    #[test]
    fn unsupported_mysql_backend_is_rejected_before_writes() {
        assert!(ensure_supported_backend(DbBackend::Postgres).is_ok());
        assert!(ensure_supported_backend(DbBackend::Sqlite).is_ok());
        assert!(ensure_supported_backend(DbBackend::MySql).is_err());
    }
}
