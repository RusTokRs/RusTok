use std::collections::HashSet;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
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
    #[error("RBAC user {user_id} belongs to tenant {actual_tenant_id}, not {expected_tenant_id}")]
    UserTenantMismatch {
        user_id: Uuid,
        expected_tenant_id: Uuid,
        actual_tenant_id: Uuid,
    },
    #[error("RBAC role assignment does not support database backend {0}")]
    UnsupportedBackend(&'static str),
    #[error(
        "RBAC built-in role slug `{slug}` is occupied by a non-system role in tenant {tenant_id}"
    )]
    BuiltInRoleSlugCollision { tenant_id: Uuid, slug: String },
}

/// Database-backed writer for built-in role assignment and reconciliation.
///
/// Routine role assignment and role-definition reconciliation are deliberately
/// separate. Assigning a user to an existing role must not silently change the
/// permissions of every other user carrying that role.
pub struct RbacRoleAssignmentDbWriter {
    db: DatabaseConnection,
}

impl RbacRoleAssignmentDbWriter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Assign and fully reconcile a built-in role atomically.
    ///
    /// This is intended for bootstrap and installer workflows. Runtime user
    /// administration should use `assign_role_on` inside its owning transaction.
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

    /// Assign and reconcile a built-in role inside the caller's transaction.
    ///
    /// Reconciliation may change the effective permissions of every user with
    /// this role. Hosts with live authorization caches must perform appropriate
    /// post-commit fan-out invalidation.
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
            .assign_role(tenant_id, user_id, role, true)
            .await
    }

    /// Assign a user to a built-in role inside the caller's transaction.
    ///
    /// Existing role definitions are not reconciled. A newly created role is
    /// initialized with its canonical permissions before the user link is added.
    pub async fn assign_role_on<C>(
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
            .assign_role(tenant_id, user_id, role, false)
            .await
    }
}

struct EnsuredRole {
    id: Uuid,
    created: bool,
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
    async fn assign_role(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        role: UserRole,
        reconcile_existing_role: bool,
    ) -> Result<(), RbacRoleAssignmentError> {
        self.ensure_user_tenant(tenant_id, user_id).await?;
        let ensured_role = self.ensure_role(tenant_id, &role).await?;

        if reconcile_existing_role || ensured_role.created {
            self.reconcile_role_permissions(ensured_role.id, tenant_id, &role)
                .await?;
        }
        self.ensure_user_role(user_id, ensured_role.id).await?;

        Ok(())
    }

    async fn reconcile_role_permissions(
        &self,
        role_id: Uuid,
        tenant_id: Uuid,
        role: &UserRole,
    ) -> Result<(), RbacRoleAssignmentError> {
        let mut expected_permission_ids = HashSet::new();
        for permission in Rbac::permissions_for_role(role) {
            let permission_id = self.ensure_permission(tenant_id, permission).await?;
            self.ensure_role_permission(role_id, permission_id).await?;
            expected_permission_ids.insert(permission_id);
        }
        self.remove_stale_role_permissions(role_id, &expected_permission_ids)
            .await
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
    ) -> Result<EnsuredRole, RbacRoleAssignmentError> {
        let slug = role.to_string();
        if let Some((id, is_system)) = self.find_role(tenant_id, &slug).await? {
            return Ok(EnsuredRole {
                id: validate_builtin_role(tenant_id, &slug, id, is_system)?,
                created: false,
            });
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

        let (id, is_system) = self
            .find_role(tenant_id, &slug)
            .await?
            .ok_or(RbacRoleAssignmentError::MissingPersistedRecord("role"))?;
        Ok(EnsuredRole {
            id: validate_builtin_role(tenant_id, &slug, id, is_system)?,
            created: true,
        })
    }

    async fn find_role(
        &self,
        tenant_id: Uuid,
        slug: &str,
    ) -> Result<Option<(Uuid, bool)>, RbacRoleAssignmentError> {
        let backend = self.db.get_database_backend();
        let sql = match backend {
            DbBackend::Sqlite => {
                "SELECT id, is_system FROM roles WHERE tenant_id = ?1 AND slug = ?2 LIMIT 1"
            }
            DbBackend::Postgres | DbBackend::MySql => {
                "SELECT id, is_system FROM roles WHERE tenant_id = $1 AND slug = $2 LIMIT 1"
            }
        };
        self.db
            .query_one(Statement::from_sql_and_values(
                backend,
                sql,
                vec![tenant_id.into(), slug.into()],
            ))
            .await
            .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))?
            .map(|row| {
                let id = row
                    .try_get("", "id")
                    .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))?;
                let is_system = row
                    .try_get("", "is_system")
                    .map_err(|error| RbacRoleAssignmentError::Database(error.to_string()))?;
                Ok((id, is_system))
            })
            .transpose()
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
            DbBackend::Sqlite => {
                "SELECT id FROM permissions WHERE tenant_id = ?1 AND resource = ?2 AND action = ?3 LIMIT 1"
            }
            DbBackend::Postgres | DbBackend::MySql => {
                "SELECT id FROM permissions WHERE tenant_id = $1 AND resource = $2 AND action = $3 LIMIT 1"
            }
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
            DbBackend::Sqlite => "SELECT permission_id FROM role_permissions WHERE role_id = ?1",
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

fn validate_builtin_role(
    tenant_id: Uuid,
    slug: &str,
    role_id: Uuid,
    is_system: bool,
) -> Result<Uuid, RbacRoleAssignmentError> {
    if is_system {
        Ok(role_id)
    } else {
        Err(RbacRoleAssignmentError::BuiltInRoleSlugCollision {
            tenant_id,
            slug: slug.to_string(),
        })
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

    use super::{
        ensure_supported_backend, render_insert_sql, stale_role_permission_ids,
        validate_builtin_role,
    };
    use sea_orm::DbBackend;
    use uuid::Uuid;

    const ROLE_PERMISSION_INSERT: &str = "INSERT INTO role_permissions (id, role_id, permission_id) VALUES ({id}, {relation_role}, {permission}) ON CONFLICT (role_id, permission_id) DO NOTHING";
    const USER_ROLE_INSERT: &str = "INSERT INTO user_roles (id, user_id, role_id) VALUES ({id}, {user}, {role}) ON CONFLICT (user_id, role_id) DO NOTHING";

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
        assert!(
            render_insert_sql(USER_ROLE_INSERT, DbBackend::Postgres)
                .contains("VALUES ($1, $2, $3)")
        );
        assert!(
            render_insert_sql(USER_ROLE_INSERT, DbBackend::Sqlite).contains("VALUES (?1, ?2, ?3)")
        );
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

    #[test]
    fn non_system_role_cannot_occupy_builtin_slug() {
        let tenant_id = Uuid::new_v4();
        let role_id = Uuid::new_v4();

        assert!(validate_builtin_role(tenant_id, "admin", role_id, true).is_ok());
        assert!(validate_builtin_role(tenant_id, "admin", role_id, false).is_err());
    }
}
