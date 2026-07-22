//! Transactional repair of canonical built-in RBAC role definitions.

use std::collections::{BTreeSet, HashMap, HashSet};

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_core::{Rbac, UserRole};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RbacSystemRoleRepairOptions {
    pub tenant_id: Option<Uuid>,
    pub apply: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacAffectedUser {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacSystemRoleRepairReport {
    pub applied: bool,
    pub tenants_scanned: u64,
    pub roles_scanned: u64,
    pub roles_created: u64,
    pub permissions_created: u64,
    pub role_permission_links_added: u64,
    pub role_permission_links_removed: u64,
    pub affected_users: Vec<RbacAffectedUser>,
    pub runtime_restart_required: bool,
}

impl RbacSystemRoleRepairReport {
    pub fn changes_total(&self) -> u64 {
        self.roles_created
            .saturating_add(self.permissions_created)
            .saturating_add(self.role_permission_links_added)
            .saturating_add(self.role_permission_links_removed)
    }
}

#[derive(Debug, Error)]
pub enum RbacSystemRoleRepairError {
    #[error("RBAC system-role repair database error: {0}")]
    Database(String),
    #[error("RBAC system-role repair does not support database backend {0}")]
    UnsupportedBackend(&'static str),
    #[error("RBAC tenant {0} does not exist")]
    TenantNotFound(Uuid),
    #[error(
        "RBAC built-in role slug `{slug}` is occupied by a non-system role in tenant {tenant_id}"
    )]
    BuiltInRoleSlugCollision { tenant_id: Uuid, slug: String },
}

pub async fn repair_system_roles(
    db: &DatabaseConnection,
    options: RbacSystemRoleRepairOptions,
) -> Result<RbacSystemRoleRepairReport, RbacSystemRoleRepairError> {
    ensure_supported_backend(db.get_database_backend())?;

    if options.apply {
        let tx = db.begin().await.map_err(database_error)?;
        let result = repair_system_roles_in_transaction(&tx, options).await;
        match result {
            Ok(mut report) => {
                tx.commit().await.map_err(database_error)?;
                report.applied = true;
                report.runtime_restart_required = !report.affected_users.is_empty();
                Ok(report)
            }
            Err(error) => {
                tx.rollback().await.map_err(|rollback_error| {
                    RbacSystemRoleRepairError::Database(format!(
                        "system-role repair failed: {error}; rollback failed: {rollback_error}"
                    ))
                })?;
                Err(error)
            }
        }
    } else {
        repair_system_roles_in_transaction(db, options).await
    }
}

/// Apply or plan canonical system-role repair on a caller-owned connection.
///
/// When `options.apply` is true, callers must pass a transaction and commit it
/// themselves. The returned report intentionally leaves `applied` false until
/// the transaction owner has completed that commit.
pub async fn repair_system_roles_in_transaction<C>(
    db: &C,
    options: RbacSystemRoleRepairOptions,
) -> Result<RbacSystemRoleRepairReport, RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    ensure_supported_backend(db.get_database_backend())?;
    let tenant_ids = load_tenant_ids(db, options.tenant_id).await?;
    if let Some(tenant_id) = options.tenant_id {
        if tenant_ids.is_empty() {
            return Err(RbacSystemRoleRepairError::TenantNotFound(tenant_id));
        }
    }

    let mut report = RbacSystemRoleRepairReport {
        tenants_scanned: tenant_ids.len() as u64,
        ..Default::default()
    };
    let mut affected_users = BTreeSet::new();
    let mut planned_permissions = HashSet::new();

    for tenant_id in tenant_ids {
        for role in built_in_roles() {
            repair_role(
                db,
                tenant_id,
                role,
                options.apply,
                &mut report,
                &mut affected_users,
                &mut planned_permissions,
            )
            .await?;
        }
    }

    report.affected_users = affected_users
        .into_iter()
        .map(|(tenant_id, user_id)| RbacAffectedUser { tenant_id, user_id })
        .collect();
    report.runtime_restart_required = options.apply && !report.affected_users.is_empty();
    Ok(report)
}

async fn repair_role<C>(
    db: &C,
    tenant_id: Uuid,
    role: UserRole,
    apply: bool,
    report: &mut RbacSystemRoleRepairReport,
    affected_users: &mut BTreeSet<(Uuid, Uuid)>,
    planned_permissions: &mut HashSet<(Uuid, String, String)>,
) -> Result<(), RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    report.roles_scanned = report.roles_scanned.saturating_add(1);
    let slug = role.to_string();
    let existing = find_role(db, tenant_id, &slug).await?;
    let role_id = match existing {
        Some((role_id, true)) => Some(role_id),
        Some((_role_id, false)) => {
            return Err(RbacSystemRoleRepairError::BuiltInRoleSlugCollision { tenant_id, slug });
        }
        None => {
            report.roles_created = report.roles_created.saturating_add(1);
            if apply {
                create_system_role(db, tenant_id, &slug).await?;
                let (role_id, is_system) =
                    find_role(db, tenant_id, &slug).await?.ok_or_else(|| {
                        RbacSystemRoleRepairError::Database(
                            "system role insert completed without a persisted role".to_string(),
                        )
                    })?;
                if !is_system {
                    return Err(RbacSystemRoleRepairError::BuiltInRoleSlugCollision {
                        tenant_id,
                        slug,
                    });
                }
                Some(role_id)
            } else {
                None
            }
        }
    };

    let expected_permissions = Rbac::permissions_for_role(&role)
        .iter()
        .map(|permission| {
            (
                permission.resource.to_string(),
                permission.action.to_string(),
            )
        })
        .collect::<HashSet<_>>();

    let existing_links = match role_id {
        Some(role_id) => load_role_permission_links(db, role_id).await?,
        None => Vec::new(),
    };
    let mut existing_by_key = HashMap::new();
    let mut stale_permission_ids = Vec::new();
    for link in existing_links {
        match (link.tenant_id, link.resource, link.action) {
            (Some(permission_tenant_id), Some(resource), Some(action))
                if permission_tenant_id == tenant_id
                    && expected_permissions.contains(&(resource.clone(), action.clone())) =>
            {
                existing_by_key.insert((resource, action), link.permission_id);
            }
            _ => stale_permission_ids.push(link.permission_id),
        }
    }

    let mut role_changed = !stale_permission_ids.is_empty() || role_id.is_none();
    report.role_permission_links_removed = report
        .role_permission_links_removed
        .saturating_add(stale_permission_ids.len() as u64);

    if apply {
        if let Some(role_id) = role_id {
            for permission_id in stale_permission_ids {
                delete_role_permission(db, role_id, permission_id).await?;
            }
        }
    }

    for (resource, action) in expected_permissions {
        if existing_by_key.contains_key(&(resource.clone(), action.clone())) {
            continue;
        }
        role_changed = true;
        let existing_permission_id = find_permission(db, tenant_id, &resource, &action).await?;
        if existing_permission_id.is_none()
            && planned_permissions.insert((tenant_id, resource.clone(), action.clone()))
        {
            report.permissions_created = report.permissions_created.saturating_add(1);
        }
        report.role_permission_links_added = report.role_permission_links_added.saturating_add(1);

        if apply {
            let permission_id = match existing_permission_id {
                Some(permission_id) => permission_id,
                None => {
                    create_permission(db, tenant_id, &resource, &action).await?;
                    find_permission(db, tenant_id, &resource, &action)
                        .await?
                        .ok_or_else(|| {
                            RbacSystemRoleRepairError::Database(
                                "permission insert completed without a persisted permission"
                                    .to_string(),
                            )
                        })?
                }
            };
            let role_id = role_id.ok_or_else(|| {
                RbacSystemRoleRepairError::Database(
                    "system role repair has no persisted role id".to_string(),
                )
            })?;
            create_role_permission(db, role_id, permission_id).await?;
        }
    }

    if role_changed {
        if let Some(role_id) = role_id {
            for user_id in load_role_user_ids(db, role_id).await? {
                affected_users.insert((tenant_id, user_id));
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
struct RolePermissionLink {
    permission_id: Uuid,
    tenant_id: Option<Uuid>,
    resource: Option<String>,
    action: Option<String>,
}

async fn load_tenant_ids<C>(
    db: &C,
    tenant_id: Option<Uuid>,
) -> Result<Vec<Uuid>, RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let (sql, values) = match tenant_id {
        Some(tenant_id) => (
            match backend {
                DbBackend::Sqlite => "SELECT id FROM tenants WHERE id = ?1",
                DbBackend::Postgres | DbBackend::MySql => "SELECT id FROM tenants WHERE id = $1",
            },
            vec![tenant_id.into()],
        ),
        None => ("SELECT id FROM tenants ORDER BY id", Vec::new()),
    };
    let rows = db
        .query_all(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| row.try_get("", "id").map_err(database_error))
        .collect()
}

async fn find_role<C>(
    db: &C,
    tenant_id: Uuid,
    slug: &str,
) -> Result<Option<(Uuid, bool)>, RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => {
            "SELECT id, is_system FROM roles WHERE tenant_id = ?1 AND slug = ?2 LIMIT 1"
        }
        DbBackend::Postgres | DbBackend::MySql => {
            "SELECT id, is_system FROM roles WHERE tenant_id = $1 AND slug = $2 LIMIT 1"
        }
    };
    db.query_one(Statement::from_sql_and_values(
        backend,
        sql,
        vec![tenant_id.into(), slug.into()],
    ))
    .await
    .map_err(database_error)?
    .map(|row| {
        Ok((
            row.try_get("", "id").map_err(database_error)?,
            row.try_get("", "is_system").map_err(database_error)?,
        ))
    })
    .transpose()
}

async fn create_system_role<C>(
    db: &C,
    tenant_id: Uuid,
    slug: &str,
) -> Result<(), RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => {
            "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES (?1, ?2, ?3, ?4, NULL, TRUE) ON CONFLICT (tenant_id, slug) DO NOTHING"
        }
        DbBackend::Postgres | DbBackend::MySql => {
            "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES ($1, $2, $3, $4, NULL, TRUE) ON CONFLICT (tenant_id, slug) DO NOTHING"
        }
    };
    execute(
        db,
        sql,
        vec![
            rustok_core::generate_id().into(),
            tenant_id.into(),
            slug.into(),
            slug.into(),
        ],
    )
    .await
}

async fn find_permission<C>(
    db: &C,
    tenant_id: Uuid,
    resource: &str,
    action: &str,
) -> Result<Option<Uuid>, RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => {
            "SELECT id FROM permissions WHERE tenant_id = ?1 AND resource = ?2 AND action = ?3 LIMIT 1"
        }
        DbBackend::Postgres | DbBackend::MySql => {
            "SELECT id FROM permissions WHERE tenant_id = $1 AND resource = $2 AND action = $3 LIMIT 1"
        }
    };
    db.query_one(Statement::from_sql_and_values(
        backend,
        sql,
        vec![tenant_id.into(), resource.into(), action.into()],
    ))
    .await
    .map_err(database_error)?
    .map(|row| row.try_get("", "id").map_err(database_error))
    .transpose()
}

async fn create_permission<C>(
    db: &C,
    tenant_id: Uuid,
    resource: &str,
    action: &str,
) -> Result<(), RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => {
            "INSERT INTO permissions (id, tenant_id, resource, action, description) VALUES (?1, ?2, ?3, ?4, NULL) ON CONFLICT (tenant_id, resource, action) DO NOTHING"
        }
        DbBackend::Postgres | DbBackend::MySql => {
            "INSERT INTO permissions (id, tenant_id, resource, action, description) VALUES ($1, $2, $3, $4, NULL) ON CONFLICT (tenant_id, resource, action) DO NOTHING"
        }
    };
    execute(
        db,
        sql,
        vec![
            rustok_core::generate_id().into(),
            tenant_id.into(),
            resource.into(),
            action.into(),
        ],
    )
    .await
}

async fn load_role_permission_links<C>(
    db: &C,
    role_id: Uuid,
) -> Result<Vec<RolePermissionLink>, RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => {
            "SELECT rp.permission_id, p.tenant_id, p.resource, p.action FROM role_permissions rp LEFT JOIN permissions p ON p.id = rp.permission_id WHERE rp.role_id = ?1"
        }
        DbBackend::Postgres | DbBackend::MySql => {
            "SELECT rp.permission_id, p.tenant_id, p.resource, p.action FROM role_permissions rp LEFT JOIN permissions p ON p.id = rp.permission_id WHERE rp.role_id = $1"
        }
    };
    let rows = db
        .query_all(Statement::from_sql_and_values(
            backend,
            sql,
            vec![role_id.into()],
        ))
        .await
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| {
            Ok(RolePermissionLink {
                permission_id: row.try_get("", "permission_id").map_err(database_error)?,
                tenant_id: row.try_get("", "tenant_id").map_err(database_error)?,
                resource: row.try_get("", "resource").map_err(database_error)?,
                action: row.try_get("", "action").map_err(database_error)?,
            })
        })
        .collect()
}

async fn create_role_permission<C>(
    db: &C,
    role_id: Uuid,
    permission_id: Uuid,
) -> Result<(), RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => {
            "INSERT INTO role_permissions (id, role_id, permission_id) VALUES (?1, ?2, ?3) ON CONFLICT (role_id, permission_id) DO NOTHING"
        }
        DbBackend::Postgres | DbBackend::MySql => {
            "INSERT INTO role_permissions (id, role_id, permission_id) VALUES ($1, $2, $3) ON CONFLICT (role_id, permission_id) DO NOTHING"
        }
    };
    execute(
        db,
        sql,
        vec![
            rustok_core::generate_id().into(),
            role_id.into(),
            permission_id.into(),
        ],
    )
    .await
}

async fn delete_role_permission<C>(
    db: &C,
    role_id: Uuid,
    permission_id: Uuid,
) -> Result<(), RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => {
            "DELETE FROM role_permissions WHERE role_id = ?1 AND permission_id = ?2"
        }
        DbBackend::Postgres | DbBackend::MySql => {
            "DELETE FROM role_permissions WHERE role_id = $1 AND permission_id = $2"
        }
    };
    execute(db, sql, vec![role_id.into(), permission_id.into()]).await
}

async fn load_role_user_ids<C>(
    db: &C,
    role_id: Uuid,
) -> Result<Vec<Uuid>, RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    let backend = db.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => "SELECT user_id FROM user_roles WHERE role_id = ?1",
        DbBackend::Postgres | DbBackend::MySql => {
            "SELECT user_id FROM user_roles WHERE role_id = $1"
        }
    };
    let rows = db
        .query_all(Statement::from_sql_and_values(
            backend,
            sql,
            vec![role_id.into()],
        ))
        .await
        .map_err(database_error)?;
    rows.into_iter()
        .map(|row| row.try_get("", "user_id").map_err(database_error))
        .collect()
}

async fn execute<C>(
    db: &C,
    sql: &str,
    values: Vec<sea_orm::Value>,
) -> Result<(), RbacSystemRoleRepairError>
where
    C: ConnectionTrait,
{
    db.execute(Statement::from_sql_and_values(
        db.get_database_backend(),
        sql,
        values,
    ))
    .await
    .map_err(database_error)?;
    Ok(())
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), RbacSystemRoleRepairError> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        DbBackend::MySql => Err(RbacSystemRoleRepairError::UnsupportedBackend("mysql")),
    }
}

fn built_in_roles() -> [UserRole; 4] {
    [
        UserRole::SuperAdmin,
        UserRole::Admin,
        UserRole::Manager,
        UserRole::Customer,
    ]
}

fn database_error(error: impl std::fmt::Display) -> RbacSystemRoleRepairError {
    RbacSystemRoleRepairError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{RbacSystemRoleRepairReport, built_in_roles};

    #[test]
    fn built_in_role_inventory_is_stable() {
        let slugs = built_in_roles().map(|role| role.to_string());
        assert_eq!(
            slugs,
            ["super_admin", "admin", "manager", "customer"].map(str::to_string)
        );
    }

    #[test]
    fn changes_total_excludes_affected_user_cardinality() {
        let report = RbacSystemRoleRepairReport {
            roles_created: 1,
            permissions_created: 2,
            role_permission_links_added: 3,
            role_permission_links_removed: 4,
            ..Default::default()
        };
        assert_eq!(report.changes_total(), 10);
    }
}
