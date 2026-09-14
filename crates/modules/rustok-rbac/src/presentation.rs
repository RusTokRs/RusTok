use rustok_api::{RuntimeLocale, StoredLocale};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacRolePresentation {
    pub tenant_id: Uuid,
    pub role_id: Uuid,
    pub locale: StoredLocale,
    pub display_name: String,
    pub description: Option<String>,
    pub copy_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacPermissionPresentation {
    pub tenant_id: Uuid,
    pub permission_id: Uuid,
    pub locale: StoredLocale,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub copy_revision: i64,
}

#[derive(Debug, Error)]
pub enum RbacPresentationStoreError {
    #[error("RBAC presentation parent was not found in the requested tenant")]
    ParentNotFound,
    #[error("RBAC presentation already exists for this exact locale")]
    AlreadyExists,
    #[error("RBAC presentation was not found")]
    NotFound,
    #[error("RBAC presentation revision conflict; expected {expected}")]
    RevisionConflict { expected: i64 },
    #[error("RBAC presentation contains an invalid stored locale")]
    InvalidStoredLocale,
    #[error("RBAC role presentation display name must not be empty")]
    EmptyRoleDisplayName,
    #[error("RBAC permission presentation must contain a display name or description")]
    EmptyPermissionPresentation,
    #[error("RBAC presentation storage does not support database backend {0}")]
    UnsupportedBackend(&'static str),
    #[error("RBAC presentation storage failed: {0}")]
    Storage(String),
}

impl From<sea_orm::DbErr> for RbacPresentationStoreError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Storage(error.to_string())
    }
}

#[derive(Clone)]
pub struct SeaOrmRbacPresentationStore {
    db: DatabaseConnection,
}

impl SeaOrmRbacPresentationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn find_role_exact(
        &self,
        tenant_id: Uuid,
        role_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<RbacRolePresentation>, RbacPresentationStoreError> {
        Self::find_role_exact_on(&self.db, tenant_id, role_id, locale).await
    }

    pub async fn find_permission_exact(
        &self,
        tenant_id: Uuid,
        permission_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<RbacPermissionPresentation>, RbacPresentationStoreError> {
        Self::find_permission_exact_on(&self.db, tenant_id, permission_id, locale).await
    }

    pub async fn create_role_source_on<C>(
        connection: &C,
        tenant_id: Uuid,
        role_id: Uuid,
        locale: RuntimeLocale,
        display_name: String,
        description: Option<String>,
    ) -> Result<RbacRolePresentation, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        if display_name.trim().is_empty() {
            return Err(RbacPresentationStoreError::EmptyRoleDisplayName);
        }
        let locale = StoredLocale::from(locale);
        ensure_parent_on(connection, "roles", tenant_id, role_id).await?;
        if Self::find_role_exact_on(connection, tenant_id, role_id, &locale)
            .await?
            .is_some()
        {
            return Err(RbacPresentationStoreError::AlreadyExists);
        }
        connection
            .execute_raw(statement(
                connection,
                "INSERT INTO rbac_role_presentations (tenant_id, role_id, locale, display_name, description, copy_revision) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                "INSERT INTO rbac_role_presentations (tenant_id, role_id, locale, display_name, description, copy_revision) VALUES ($1, $2, $3, $4, $5, 1)",
                vec![
                    tenant_id.into(),
                    role_id.into(),
                    locale.as_str().to_owned().into(),
                    display_name.into(),
                    description.into(),
                ],
            )?)
            .await?;
        Self::find_role_exact_on(connection, tenant_id, role_id, &locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }

    pub async fn create_permission_source_on<C>(
        connection: &C,
        tenant_id: Uuid,
        permission_id: Uuid,
        locale: RuntimeLocale,
        display_name: Option<String>,
        description: Option<String>,
    ) -> Result<RbacPermissionPresentation, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        validate_permission_copy(&display_name, &description)?;
        let locale = StoredLocale::from(locale);
        ensure_parent_on(connection, "permissions", tenant_id, permission_id).await?;
        if Self::find_permission_exact_on(connection, tenant_id, permission_id, &locale)
            .await?
            .is_some()
        {
            return Err(RbacPresentationStoreError::AlreadyExists);
        }
        connection
            .execute_raw(statement(
                connection,
                "INSERT INTO rbac_permission_presentations (tenant_id, permission_id, locale, display_name, description, copy_revision) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                "INSERT INTO rbac_permission_presentations (tenant_id, permission_id, locale, display_name, description, copy_revision) VALUES ($1, $2, $3, $4, $5, 1)",
                vec![
                    tenant_id.into(),
                    permission_id.into(),
                    locale.as_str().to_owned().into(),
                    display_name.into(),
                    description.into(),
                ],
            )?)
            .await?;
        Self::find_permission_exact_on(connection, tenant_id, permission_id, &locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }

    pub async fn compare_and_set_role_source_on<C>(
        connection: &C,
        tenant_id: Uuid,
        role_id: Uuid,
        locale: &RuntimeLocale,
        expected_copy_revision: i64,
        display_name: String,
        description: Option<String>,
    ) -> Result<RbacRolePresentation, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        if display_name.trim().is_empty() {
            return Err(RbacPresentationStoreError::EmptyRoleDisplayName);
        }
        let stored_locale = StoredLocale::from(locale.clone());
        let current = Self::find_role_exact_on(connection, tenant_id, role_id, &stored_locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)?;
        if current.copy_revision != expected_copy_revision {
            return Err(RbacPresentationStoreError::RevisionConflict {
                expected: expected_copy_revision,
            });
        }
        if current.display_name == display_name && current.description == description {
            return Ok(current);
        }
        let result = connection
            .execute_raw(statement(
                connection,
                "UPDATE rbac_role_presentations SET display_name = ?1, description = ?2, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = ?3 AND role_id = ?4 AND locale = ?5 AND copy_revision = ?6",
                "UPDATE rbac_role_presentations SET display_name = $1, description = $2, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $3 AND role_id = $4 AND locale = $5 AND copy_revision = $6",
                vec![
                    display_name.into(),
                    description.into(),
                    tenant_id.into(),
                    role_id.into(),
                    stored_locale.as_str().to_owned().into(),
                    expected_copy_revision.into(),
                ],
            )?)
            .await?;
        if result.rows_affected() == 0 {
            return Err(RbacPresentationStoreError::RevisionConflict {
                expected: expected_copy_revision,
            });
        }
        Self::find_role_exact_on(connection, tenant_id, role_id, &stored_locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }

    pub async fn compare_and_set_permission_source_on<C>(
        connection: &C,
        tenant_id: Uuid,
        permission_id: Uuid,
        locale: &RuntimeLocale,
        expected_copy_revision: i64,
        display_name: Option<String>,
        description: Option<String>,
    ) -> Result<RbacPermissionPresentation, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        validate_permission_copy(&display_name, &description)?;
        let stored_locale = StoredLocale::from(locale.clone());
        let current =
            Self::find_permission_exact_on(connection, tenant_id, permission_id, &stored_locale)
                .await?
                .ok_or(RbacPresentationStoreError::NotFound)?;
        if current.copy_revision != expected_copy_revision {
            return Err(RbacPresentationStoreError::RevisionConflict {
                expected: expected_copy_revision,
            });
        }
        if current.display_name == display_name && current.description == description {
            return Ok(current);
        }
        let result = connection
            .execute_raw(statement(
                connection,
                "UPDATE rbac_permission_presentations SET display_name = ?1, description = ?2, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = ?3 AND permission_id = ?4 AND locale = ?5 AND copy_revision = ?6",
                "UPDATE rbac_permission_presentations SET display_name = $1, description = $2, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $3 AND permission_id = $4 AND locale = $5 AND copy_revision = $6",
                vec![
                    display_name.into(),
                    description.into(),
                    tenant_id.into(),
                    permission_id.into(),
                    stored_locale.as_str().to_owned().into(),
                    expected_copy_revision.into(),
                ],
            )?)
            .await?;
        if result.rows_affected() == 0 {
            return Err(RbacPresentationStoreError::RevisionConflict {
                expected: expected_copy_revision,
            });
        }
        Self::find_permission_exact_on(connection, tenant_id, permission_id, &stored_locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }

    async fn find_role_exact_on<C>(
        connection: &C,
        tenant_id: Uuid,
        role_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<RbacRolePresentation>, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        connection
            .query_one_raw(statement(
                connection,
                "SELECT tenant_id, role_id, locale, display_name, description, copy_revision FROM rbac_role_presentations WHERE tenant_id = ?1 AND role_id = ?2 AND locale = ?3",
                "SELECT tenant_id, role_id, locale, display_name, description, copy_revision FROM rbac_role_presentations WHERE tenant_id = $1 AND role_id = $2 AND locale = $3",
                vec![
                    tenant_id.into(),
                    role_id.into(),
                    locale.as_str().to_owned().into(),
                ],
            )?)
            .await?
            .map(|row| {
                let raw_locale: String = row.try_get("", "locale")?;
                let locale = StoredLocale::new(&raw_locale)
                    .map_err(|_| RbacPresentationStoreError::InvalidStoredLocale)?;
                Ok(RbacRolePresentation {
                    tenant_id: row.try_get("", "tenant_id")?,
                    role_id: row.try_get("", "role_id")?,
                    locale,
                    display_name: row.try_get("", "display_name")?,
                    description: row.try_get("", "description")?,
                    copy_revision: row.try_get("", "copy_revision")?,
                })
            })
            .transpose()
    }

    async fn find_permission_exact_on<C>(
        connection: &C,
        tenant_id: Uuid,
        permission_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<RbacPermissionPresentation>, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        connection
            .query_one_raw(statement(
                connection,
                "SELECT tenant_id, permission_id, locale, display_name, description, copy_revision FROM rbac_permission_presentations WHERE tenant_id = ?1 AND permission_id = ?2 AND locale = ?3",
                "SELECT tenant_id, permission_id, locale, display_name, description, copy_revision FROM rbac_permission_presentations WHERE tenant_id = $1 AND permission_id = $2 AND locale = $3",
                vec![
                    tenant_id.into(),
                    permission_id.into(),
                    locale.as_str().to_owned().into(),
                ],
            )?)
            .await?
            .map(|row| {
                let raw_locale: String = row.try_get("", "locale")?;
                let locale = StoredLocale::new(&raw_locale)
                    .map_err(|_| RbacPresentationStoreError::InvalidStoredLocale)?;
                Ok(RbacPermissionPresentation {
                    tenant_id: row.try_get("", "tenant_id")?,
                    permission_id: row.try_get("", "permission_id")?,
                    locale,
                    display_name: row.try_get("", "display_name")?,
                    description: row.try_get("", "description")?,
                    copy_revision: row.try_get("", "copy_revision")?,
                })
            })
            .transpose()
    }
}

async fn ensure_parent_on<C>(
    connection: &C,
    table: &'static str,
    tenant_id: Uuid,
    parent_id: Uuid,
) -> Result<(), RbacPresentationStoreError>
where
    C: ConnectionTrait,
{
    let (sqlite, postgres) = match table {
        "roles" => (
            "SELECT id FROM roles WHERE tenant_id = ?1 AND id = ?2",
            "SELECT id FROM roles WHERE tenant_id = $1 AND id = $2",
        ),
        "permissions" => (
            "SELECT id FROM permissions WHERE tenant_id = ?1 AND id = ?2",
            "SELECT id FROM permissions WHERE tenant_id = $1 AND id = $2",
        ),
        _ => unreachable!("RBAC presentation parent table is static"),
    };
    let exists = connection
        .query_one_raw(statement(
            connection,
            sqlite,
            postgres,
            vec![tenant_id.into(), parent_id.into()],
        )?)
        .await?
        .is_some();
    if exists {
        Ok(())
    } else {
        Err(RbacPresentationStoreError::ParentNotFound)
    }
}

fn validate_permission_copy(
    display_name: &Option<String>,
    description: &Option<String>,
) -> Result<(), RbacPresentationStoreError> {
    let has_display_name = display_name
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let has_description = description
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    if has_display_name || has_description {
        Ok(())
    } else {
        Err(RbacPresentationStoreError::EmptyPermissionPresentation)
    }
}

fn statement<C>(
    connection: &C,
    sqlite: &'static str,
    postgres: &'static str,
    values: Vec<Value>,
) -> Result<Statement, RbacPresentationStoreError>
where
    C: ConnectionTrait,
{
    let backend = connection.get_database_backend();
    let sql = match backend {
        DbBackend::Sqlite => sqlite,
        DbBackend::Postgres => postgres,
        DbBackend::MySql => {
            return Err(RbacPresentationStoreError::UnsupportedBackend("mysql"));
        }
    };
    Ok(Statement::from_sql_and_values(backend, sql, values))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    async fn fixture() -> (DatabaseConnection, Uuid, Uuid, Uuid) {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON").await.unwrap();
        for ddl in [
            "CREATE TABLE roles (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL, name TEXT NOT NULL, slug TEXT NOT NULL, description TEXT, is_system BOOLEAN NOT NULL, updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE (tenant_id, slug))",
            "CREATE UNIQUE INDEX uq_rbac_roles_tenant_id_id ON roles (tenant_id, id)",
            "CREATE TABLE permissions (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL, resource TEXT NOT NULL, action TEXT NOT NULL, description TEXT, UNIQUE (tenant_id, resource, action))",
        ] {
            db.execute_unprepared(ddl).await.unwrap();
        }
        let tenant_id = Uuid::new_v4();
        let role_id = Uuid::new_v4();
        let permission_id = Uuid::new_v4();
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES (?1, ?2, ?3, ?4, ?5, TRUE)",
            vec![
                role_id.into(),
                tenant_id.into(),
                "Legacy Manager".into(),
                "manager".into(),
                "Legacy role copy".into(),
            ],
        ))
        .await
        .unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO permissions (id, tenant_id, resource, action, description) VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                permission_id.into(),
                tenant_id.into(),
                "users".into(),
                "list".into(),
                "Legacy permission copy".into(),
            ],
        ))
        .await
        .unwrap();
        let manager = SchemaManager::new(&db);
        crate::m20260914_000001_role_permission_presentations::Migration
            .up(&manager)
            .await
            .unwrap();
        (db, tenant_id, role_id, permission_id)
    }

    #[test]
    fn canonical_source_locale_cannot_be_unknown() {
        assert!(RuntimeLocale::new("und").is_err());
    }

    #[tokio::test]
    async fn legacy_copy_is_truthfully_backfilled_as_unknown() {
        let (db, tenant_id, role_id, permission_id) = fixture().await;
        let store = SeaOrmRbacPresentationStore::new(db);
        let unknown = StoredLocale::new("und").unwrap();
        let role = store
            .find_role_exact(tenant_id, role_id, &unknown)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(role.display_name, "Legacy Manager");
        assert_eq!(role.description.as_deref(), Some("Legacy role copy"));
        assert_eq!(role.copy_revision, 1);
        let permission = store
            .find_permission_exact(tenant_id, permission_id, &unknown)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(permission.display_name, None);
        assert_eq!(
            permission.description.as_deref(),
            Some("Legacy permission copy")
        );
    }

    #[tokio::test]
    async fn concrete_source_writes_are_tenant_safe_and_copy_revisioned() {
        let (db, tenant_id, role_id, permission_id) = fixture().await;
        let store = SeaOrmRbacPresentationStore::new(db.clone());
        let en = RuntimeLocale::new("en").unwrap();
        let stored_en = StoredLocale::from(en.clone());

        let created = SeaOrmRbacPresentationStore::create_role_source_on(
            &db,
            tenant_id,
            role_id,
            en.clone(),
            "Manager".to_string(),
            Some("Manages tenant users".to_string()),
        )
        .await
        .unwrap();
        assert_eq!(created.copy_revision, 1);
        let unchanged = SeaOrmRbacPresentationStore::compare_and_set_role_source_on(
            &db,
            tenant_id,
            role_id,
            &en,
            1,
            "Manager".to_string(),
            Some("Manages tenant users".to_string()),
        )
        .await
        .unwrap();
        assert_eq!(unchanged.copy_revision, 1);
        let changed = SeaOrmRbacPresentationStore::compare_and_set_role_source_on(
            &db,
            tenant_id,
            role_id,
            &en,
            1,
            "Tenant Manager".to_string(),
            Some("Manages tenant users".to_string()),
        )
        .await
        .unwrap();
        assert_eq!(changed.copy_revision, 2);
        assert!(matches!(
            SeaOrmRbacPresentationStore::compare_and_set_role_source_on(
                &db,
                tenant_id,
                role_id,
                &en,
                1,
                "Stale".to_string(),
                None,
            )
            .await,
            Err(RbacPresentationStoreError::RevisionConflict { expected: 1 })
        ));
        assert_eq!(
            store
                .find_role_exact(tenant_id, role_id, &stored_en)
                .await
                .unwrap()
                .unwrap()
                .copy_revision,
            2
        );

        let fr = RuntimeLocale::new("fr").unwrap();
        let permission = SeaOrmRbacPresentationStore::create_permission_source_on(
            &db,
            tenant_id,
            permission_id,
            fr,
            Some("Lister les utilisateurs".to_string()),
            None,
        )
        .await
        .unwrap();
        assert_eq!(permission.copy_revision, 1);

        assert!(matches!(
            SeaOrmRbacPresentationStore::create_role_source_on(
                &db,
                Uuid::new_v4(),
                role_id,
                RuntimeLocale::new("de").unwrap(),
                "Manager".to_string(),
                None,
            )
            .await,
            Err(RbacPresentationStoreError::ParentNotFound)
        ));
    }

    #[tokio::test]
    async fn presentation_write_participates_in_caller_transaction() {
        let (db, tenant_id, role_id, _) = fixture().await;
        let locale = RuntimeLocale::new("es").unwrap();
        let stored = StoredLocale::from(locale.clone());
        let tx = db.begin().await.unwrap();
        SeaOrmRbacPresentationStore::create_role_source_on(
            &tx,
            tenant_id,
            role_id,
            locale,
            "Gestor".to_string(),
            None,
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();
        let store = SeaOrmRbacPresentationStore::new(db);
        assert!(
            store
                .find_role_exact(tenant_id, role_id, &stored)
                .await
                .unwrap()
                .is_none()
        );
    }
}
