//! Canonical owner store for locale-bound RBAC presentation copy.
//!
//! Authorization identity never flows through this store. Every write is scoped to an
//! existing tenant-owned role or permission and advances only the row-local
//! `copy_revision`. The `und` locale is readable for legacy migration evidence but is
//! intentionally not writable through the canonical API.

use rustok_api::normalize_locale_tag;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, QueryResult, Statement};
use thiserror::Error;
use uuid::Uuid;

const UNKNOWN_LEGACY_LOCALE: &str = "und";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolePresentation {
    pub role_id: Uuid,
    pub locale: String,
    pub source_locale: String,
    pub name: String,
    pub description: Option<String>,
    pub copy_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionPresentation {
    pub permission_id: Uuid,
    pub locale: String,
    pub source_locale: String,
    pub label: Option<String>,
    pub description: Option<String>,
    pub copy_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolePresentationWrite {
    pub tenant_id: Uuid,
    pub role_id: Uuid,
    pub locale: String,
    pub source_locale: String,
    pub name: String,
    pub description: Option<String>,
    /// `None` means create-only. `Some(n)` is a compare-and-swap replacement of revision `n`.
    pub expected_copy_revision: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionPresentationWrite {
    pub tenant_id: Uuid,
    pub permission_id: Uuid,
    pub locale: String,
    pub source_locale: String,
    pub label: Option<String>,
    pub description: Option<String>,
    /// `None` means create-only. `Some(n)` is a compare-and-swap replacement of revision `n`.
    pub expected_copy_revision: Option<i64>,
}

#[derive(Debug, Error)]
pub enum RbacPresentationStorageError {
    #[error("RBAC presentation storage does not support database backend {0:?}")]
    UnsupportedBackend(DbBackend),
    #[error("invalid locale tag: {0}")]
    InvalidLocale(String),
    #[error("locale 'und' is reserved for legacy storage with unknown provenance")]
    UnknownLocaleReserved,
    #[error("role presentation name must contain non-whitespace text")]
    EmptyRoleName,
    #[error("permission presentation must provide a label or description")]
    EmptyPermissionPresentation,
    #[error("RBAC presentation copy revision conflict or tenant-owned identity not found")]
    Conflict,
    #[error(transparent)]
    Storage(#[from] DbErr),
}

#[derive(Clone)]
pub struct RbacPresentationStore {
    db: DatabaseConnection,
}

impl RbacPresentationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn write_role_presentation(
        &self,
        request: RolePresentationWrite,
    ) -> Result<RolePresentation, RbacPresentationStorageError> {
        if request.name.trim().is_empty() {
            return Err(RbacPresentationStorageError::EmptyRoleName);
        }
        let locale = normalize_write_locale(&request.locale)?;
        let source_locale = normalize_write_locale(&request.source_locale)?;
        let backend = supported_backend(self.db.get_database_backend())?;

        let row = match request.expected_copy_revision {
            None => {
                self.db
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        role_insert_sql(backend)?,
                        vec![
                            request.role_id.into(),
                            request.tenant_id.into(),
                            locale.clone().into(),
                            source_locale.clone().into(),
                            request.name.into(),
                            request.description.into(),
                        ],
                    ))
                    .await?
            }
            Some(expected_revision) if expected_revision > 0 => {
                self.db
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        role_update_sql(backend)?,
                        vec![
                            request.role_id.into(),
                            request.tenant_id.into(),
                            locale.clone().into(),
                            source_locale.clone().into(),
                            request.name.into(),
                            request.description.into(),
                            expected_revision.into(),
                        ],
                    ))
                    .await?
            }
            Some(_) => return Err(RbacPresentationStorageError::Conflict),
        }
        .ok_or(RbacPresentationStorageError::Conflict)?;

        role_from_row(row)
    }

    pub async fn write_permission_presentation(
        &self,
        request: PermissionPresentationWrite,
    ) -> Result<PermissionPresentation, RbacPresentationStorageError> {
        if request.label.is_none() && request.description.is_none() {
            return Err(RbacPresentationStorageError::EmptyPermissionPresentation);
        }
        let locale = normalize_write_locale(&request.locale)?;
        let source_locale = normalize_write_locale(&request.source_locale)?;
        let backend = supported_backend(self.db.get_database_backend())?;

        let row = match request.expected_copy_revision {
            None => {
                self.db
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        permission_insert_sql(backend)?,
                        vec![
                            request.permission_id.into(),
                            request.tenant_id.into(),
                            locale.clone().into(),
                            source_locale.clone().into(),
                            request.label.into(),
                            request.description.into(),
                        ],
                    ))
                    .await?
            }
            Some(expected_revision) if expected_revision > 0 => {
                self.db
                    .query_one_raw(Statement::from_sql_and_values(
                        backend,
                        permission_update_sql(backend)?,
                        vec![
                            request.permission_id.into(),
                            request.tenant_id.into(),
                            locale.clone().into(),
                            source_locale.clone().into(),
                            request.label.into(),
                            request.description.into(),
                            expected_revision.into(),
                        ],
                    ))
                    .await?
            }
            Some(_) => return Err(RbacPresentationStorageError::Conflict),
        }
        .ok_or(RbacPresentationStorageError::Conflict)?;

        permission_from_row(row)
    }

    pub async fn read_role_presentation(
        &self,
        tenant_id: Uuid,
        role_id: Uuid,
        locale: &str,
    ) -> Result<Option<RolePresentation>, RbacPresentationStorageError> {
        let locale = normalize_read_locale(locale)?;
        let backend = supported_backend(self.db.get_database_backend())?;
        self.db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                role_read_sql(backend)?,
                vec![role_id.into(), tenant_id.into(), locale.into()],
            ))
            .await?
            .map(role_from_row)
            .transpose()
    }

    pub async fn read_permission_presentation(
        &self,
        tenant_id: Uuid,
        permission_id: Uuid,
        locale: &str,
    ) -> Result<Option<PermissionPresentation>, RbacPresentationStorageError> {
        let locale = normalize_read_locale(locale)?;
        let backend = supported_backend(self.db.get_database_backend())?;
        self.db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                permission_read_sql(backend)?,
                vec![permission_id.into(), tenant_id.into(), locale.into()],
            ))
            .await?
            .map(permission_from_row)
            .transpose()
    }
}

fn supported_backend(backend: DbBackend) -> Result<DbBackend, RbacPresentationStorageError> {
    match backend {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(backend),
        _ => Err(RbacPresentationStorageError::UnsupportedBackend(backend)),
    }
}

fn normalize_read_locale(locale: &str) -> Result<String, RbacPresentationStorageError> {
    normalize_locale_tag(locale)
        .ok_or_else(|| RbacPresentationStorageError::InvalidLocale(locale.to_owned()))
}

fn normalize_write_locale(locale: &str) -> Result<String, RbacPresentationStorageError> {
    let normalized = normalize_read_locale(locale)?;
    if normalized == UNKNOWN_LEGACY_LOCALE {
        return Err(RbacPresentationStorageError::UnknownLocaleReserved);
    }
    Ok(normalized)
}

fn role_from_row(row: QueryResult) -> Result<RolePresentation, RbacPresentationStorageError> {
    Ok(RolePresentation {
        role_id: row.try_get("", "role_id")?,
        locale: row.try_get("", "locale")?,
        source_locale: row.try_get("", "source_locale")?,
        name: row.try_get("", "name")?,
        description: row.try_get("", "description")?,
        copy_revision: row.try_get("", "copy_revision")?,
    })
}

fn permission_from_row(
    row: QueryResult,
) -> Result<PermissionPresentation, RbacPresentationStorageError> {
    Ok(PermissionPresentation {
        permission_id: row.try_get("", "permission_id")?,
        locale: row.try_get("", "locale")?,
        source_locale: row.try_get("", "source_locale")?,
        label: row.try_get("", "label")?,
        description: row.try_get("", "description")?,
        copy_revision: row.try_get("", "copy_revision")?,
    })
}

fn role_insert_sql(backend: DbBackend) -> Result<&'static str, RbacPresentationStorageError> {
    sql_by_backend(
        backend,
        "INSERT INTO rbac_role_presentations (role_id, locale, source_locale, name, description, copy_revision, created_at, updated_at) SELECT r.id, $3, $4, $5, $6, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM roles r WHERE r.id = $1 AND r.tenant_id = $2 ON CONFLICT (role_id, locale) DO NOTHING RETURNING role_id, locale, source_locale, name, description, copy_revision",
        "INSERT INTO rbac_role_presentations (role_id, locale, source_locale, name, description, copy_revision, created_at, updated_at) SELECT r.id, ?3, ?4, ?5, ?6, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM roles r WHERE r.id = ?1 AND r.tenant_id = ?2 ON CONFLICT (role_id, locale) DO NOTHING RETURNING role_id, locale, source_locale, name, description, copy_revision",
    )
}

fn role_update_sql(backend: DbBackend) -> Result<&'static str, RbacPresentationStorageError> {
    sql_by_backend(
        backend,
        "UPDATE rbac_role_presentations SET source_locale = $4, name = $5, description = $6, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE role_id = $1 AND locale = $3 AND copy_revision = $7 AND EXISTS (SELECT 1 FROM roles r WHERE r.id = $1 AND r.tenant_id = $2) RETURNING role_id, locale, source_locale, name, description, copy_revision",
        "UPDATE rbac_role_presentations SET source_locale = ?4, name = ?5, description = ?6, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE role_id = ?1 AND locale = ?3 AND copy_revision = ?7 AND EXISTS (SELECT 1 FROM roles r WHERE r.id = ?1 AND r.tenant_id = ?2) RETURNING role_id, locale, source_locale, name, description, copy_revision",
    )
}

fn role_read_sql(backend: DbBackend) -> Result<&'static str, RbacPresentationStorageError> {
    sql_by_backend(
        backend,
        "SELECT p.role_id, p.locale, p.source_locale, p.name, p.description, p.copy_revision FROM rbac_role_presentations p JOIN roles r ON r.id = p.role_id WHERE p.role_id = $1 AND r.tenant_id = $2 AND p.locale = $3",
        "SELECT p.role_id, p.locale, p.source_locale, p.name, p.description, p.copy_revision FROM rbac_role_presentations p JOIN roles r ON r.id = p.role_id WHERE p.role_id = ?1 AND r.tenant_id = ?2 AND p.locale = ?3",
    )
}

fn permission_insert_sql(
    backend: DbBackend,
) -> Result<&'static str, RbacPresentationStorageError> {
    sql_by_backend(
        backend,
        "INSERT INTO rbac_permission_presentations (permission_id, locale, source_locale, label, description, copy_revision, created_at, updated_at) SELECT p.id, $3, $4, $5, $6, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM permissions p WHERE p.id = $1 AND p.tenant_id = $2 ON CONFLICT (permission_id, locale) DO NOTHING RETURNING permission_id, locale, source_locale, label, description, copy_revision",
        "INSERT INTO rbac_permission_presentations (permission_id, locale, source_locale, label, description, copy_revision, created_at, updated_at) SELECT p.id, ?3, ?4, ?5, ?6, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM permissions p WHERE p.id = ?1 AND p.tenant_id = ?2 ON CONFLICT (permission_id, locale) DO NOTHING RETURNING permission_id, locale, source_locale, label, description, copy_revision",
    )
}

fn permission_update_sql(
    backend: DbBackend,
) -> Result<&'static str, RbacPresentationStorageError> {
    sql_by_backend(
        backend,
        "UPDATE rbac_permission_presentations SET source_locale = $4, label = $5, description = $6, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE permission_id = $1 AND locale = $3 AND copy_revision = $7 AND EXISTS (SELECT 1 FROM permissions p WHERE p.id = $1 AND p.tenant_id = $2) RETURNING permission_id, locale, source_locale, label, description, copy_revision",
        "UPDATE rbac_permission_presentations SET source_locale = ?4, label = ?5, description = ?6, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE permission_id = ?1 AND locale = ?3 AND copy_revision = ?7 AND EXISTS (SELECT 1 FROM permissions p WHERE p.id = ?1 AND p.tenant_id = ?2) RETURNING permission_id, locale, source_locale, label, description, copy_revision",
    )
}

fn permission_read_sql(
    backend: DbBackend,
) -> Result<&'static str, RbacPresentationStorageError> {
    sql_by_backend(
        backend,
        "SELECT p.permission_id, p.locale, p.source_locale, p.label, p.description, p.copy_revision FROM rbac_permission_presentations p JOIN permissions permission ON permission.id = p.permission_id WHERE p.permission_id = $1 AND permission.tenant_id = $2 AND p.locale = $3",
        "SELECT p.permission_id, p.locale, p.source_locale, p.label, p.description, p.copy_revision FROM rbac_permission_presentations p JOIN permissions permission ON permission.id = p.permission_id WHERE p.permission_id = ?1 AND permission.tenant_id = ?2 AND p.locale = ?3",
    )
}

fn sql_by_backend(
    backend: DbBackend,
    postgres: &'static str,
    sqlite: &'static str,
) -> Result<&'static str, RbacPresentationStorageError> {
    match backend {
        DbBackend::Postgres => Ok(postgres),
        DbBackend::Sqlite => Ok(sqlite),
        _ => Err(RbacPresentationStorageError::UnsupportedBackend(backend)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_writes_reject_storage_only_unknown_locale() {
        assert!(matches!(
            normalize_write_locale("und"),
            Err(RbacPresentationStorageError::UnknownLocaleReserved)
        ));
    }

    #[test]
    fn exact_reads_may_address_legacy_unknown_locale() {
        assert_eq!(normalize_read_locale("und").expect("normalize und"), "und");
    }

    #[test]
    fn copy_revision_update_is_not_authorization_generation() {
        let sql = role_update_sql(DbBackend::Postgres).expect("postgres SQL");
        assert!(sql.contains("copy_revision = copy_revision + 1"));
        assert!(!sql.contains("invalidation_generation"));
    }
}
