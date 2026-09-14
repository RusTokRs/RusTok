use rustok_api::{RuntimeLocale, StoredLocale};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, DbErr, Statement, Value};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacRolePresentation {
    pub tenant_id: Uuid,
    pub role_slug: String,
    pub locale: StoredLocale,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub copy_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacPermissionPresentation {
    pub tenant_id: Uuid,
    pub permission_key: String,
    pub locale: StoredLocale,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub copy_revision: i64,
}

#[derive(Debug, Error)]
pub enum RbacPresentationStoreError {
    #[error("RBAC presentation identity must not be empty")]
    EmptyIdentity,
    #[error("RBAC presentation already exists")]
    AlreadyExists,
    #[error("RBAC presentation was not found")]
    NotFound,
    #[error("RBAC presentation revision conflict; expected {expected}")]
    RevisionConflict { expected: i64 },
    #[error("RBAC presentation contains an invalid stored locale")]
    InvalidStoredLocale,
    #[error("RBAC presentation storage failed: {0}")]
    Storage(String),
}

impl From<DbErr> for RbacPresentationStoreError {
    fn from(error: DbErr) -> Self {
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
        role_slug: &str,
        locale: &StoredLocale,
    ) -> Result<Option<RbacRolePresentation>, RbacPresentationStoreError> {
        validate_identity(role_slug)?;
        load_exact(&self.db, PresentationKind::Role, tenant_id, role_slug, locale)
            .await?
            .map(role_from_row)
            .transpose()
    }

    pub async fn find_permission_exact(
        &self,
        tenant_id: Uuid,
        permission_key: &str,
        locale: &StoredLocale,
    ) -> Result<Option<RbacPermissionPresentation>, RbacPresentationStoreError> {
        validate_identity(permission_key)?;
        load_exact(
            &self.db,
            PresentationKind::Permission,
            tenant_id,
            permission_key,
            locale,
        )
        .await?
        .map(permission_from_row)
        .transpose()
    }

    /// Creates owner-authored role presentation. RuntimeLocale deliberately
    /// excludes `und`, keeping unknown legacy provenance storage-only.
    pub async fn create_role_source(
        &self,
        tenant_id: Uuid,
        role_slug: &str,
        source_locale: RuntimeLocale,
        display_name: Option<String>,
        description: Option<String>,
    ) -> Result<RbacRolePresentation, RbacPresentationStoreError> {
        let locale = StoredLocale::from(source_locale);
        create(
            &self.db,
            PresentationKind::Role,
            tenant_id,
            role_slug,
            &locale,
            display_name,
            description,
        )
        .await?;
        self.find_role_exact(tenant_id, role_slug, &locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }

    /// Creates owner-authored permission presentation. Permission identity is
    /// the canonical permission key and is never rewritten by this store.
    pub async fn create_permission_source(
        &self,
        tenant_id: Uuid,
        permission_key: &str,
        source_locale: RuntimeLocale,
        display_name: Option<String>,
        description: Option<String>,
    ) -> Result<RbacPermissionPresentation, RbacPresentationStoreError> {
        let locale = StoredLocale::from(source_locale);
        create(
            &self.db,
            PresentationKind::Permission,
            tenant_id,
            permission_key,
            &locale,
            display_name,
            description,
        )
        .await?;
        self.find_permission_exact(tenant_id, permission_key, &locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }

    pub async fn compare_and_set_role_source(
        &self,
        tenant_id: Uuid,
        role_slug: &str,
        source_locale: &RuntimeLocale,
        expected_copy_revision: i64,
        display_name: Option<String>,
        description: Option<String>,
    ) -> Result<RbacRolePresentation, RbacPresentationStoreError> {
        let locale = StoredLocale::from(source_locale.clone());
        compare_and_set(
            &self.db,
            PresentationKind::Role,
            tenant_id,
            role_slug,
            &locale,
            expected_copy_revision,
            display_name,
            description,
        )
        .await?;
        self.find_role_exact(tenant_id, role_slug, &locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }

    pub async fn compare_and_set_permission_source(
        &self,
        tenant_id: Uuid,
        permission_key: &str,
        source_locale: &RuntimeLocale,
        expected_copy_revision: i64,
        display_name: Option<String>,
        description: Option<String>,
    ) -> Result<RbacPermissionPresentation, RbacPresentationStoreError> {
        let locale = StoredLocale::from(source_locale.clone());
        compare_and_set(
            &self.db,
            PresentationKind::Permission,
            tenant_id,
            permission_key,
            &locale,
            expected_copy_revision,
            display_name,
            description,
        )
        .await?;
        self.find_permission_exact(tenant_id, permission_key, &locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }
}

#[derive(Clone, Copy)]
enum PresentationKind {
    Role,
    Permission,
}

impl PresentationKind {
    fn table(self) -> &'static str {
        match self {
            Self::Role => "rbac_role_presentations",
            Self::Permission => "rbac_permission_presentations",
        }
    }

    fn key_column(self) -> &'static str {
        match self {
            Self::Role => "role_slug",
            Self::Permission => "permission_key",
        }
    }
}

#[derive(Debug)]
struct PresentationRow {
    tenant_id: Uuid,
    key: String,
    locale: StoredLocale,
    display_name: Option<String>,
    description: Option<String>,
    copy_revision: i64,
}

fn validate_identity(identity: &str) -> Result<(), RbacPresentationStoreError> {
    if identity.trim().is_empty() {
        Err(RbacPresentationStoreError::EmptyIdentity)
    } else {
        Ok(())
    }
}

fn statement<C: ConnectionTrait>(
    db: &C,
    sql: String,
    values: Vec<Value>,
) -> Result<Statement, RbacPresentationStoreError> {
    let backend = db.get_database_backend();
    if !matches!(backend, DbBackend::Postgres | DbBackend::Sqlite) {
        return Err(RbacPresentationStoreError::Storage(
            "RBAC presentation storage requires PostgreSQL or SQLite".to_string(),
        ));
    }
    let mut index = 0;
    let sql = sql
        .chars()
        .map(|ch| {
            if ch == '?' {
                index += 1;
                match backend {
                    DbBackend::Postgres => format!("${index}"),
                    DbBackend::Sqlite => format!("?{index}"),
                    _ => unreachable!(),
                }
            } else {
                ch.to_string()
            }
        })
        .collect::<String>();
    Ok(Statement::from_sql_and_values(backend, sql, values))
}

async fn load_exact<C: ConnectionTrait>(
    db: &C,
    kind: PresentationKind,
    tenant_id: Uuid,
    key: &str,
    locale: &StoredLocale,
) -> Result<Option<PresentationRow>, RbacPresentationStoreError> {
    let sql = format!(
        "SELECT tenant_id, {key_column} AS presentation_key, locale, display_name, description, copy_revision FROM {table} WHERE tenant_id = ? AND {key_column} = ? AND locale = ?",
        table = kind.table(),
        key_column = kind.key_column(),
    );
    db.query_one_raw(statement(
        db,
        sql,
        vec![
            tenant_id.into(),
            key.to_owned().into(),
            locale.as_str().to_owned().into(),
        ],
    )?)
    .await?
    .map(|row| {
        let locale = row.try_get::<String>("", "locale")?;
        Ok(PresentationRow {
            tenant_id: row.try_get("", "tenant_id")?,
            key: row.try_get("", "presentation_key")?,
            locale: StoredLocale::new(&locale)
                .map_err(|_| RbacPresentationStoreError::InvalidStoredLocale)?,
            display_name: row.try_get("", "display_name")?,
            description: row.try_get("", "description")?,
            copy_revision: row.try_get("", "copy_revision")?,
        })
    })
    .transpose()
}

async fn create<C: ConnectionTrait>(
    db: &C,
    kind: PresentationKind,
    tenant_id: Uuid,
    key: &str,
    locale: &StoredLocale,
    display_name: Option<String>,
    description: Option<String>,
) -> Result<(), RbacPresentationStoreError> {
    validate_identity(key)?;
    let sql = format!(
        "INSERT INTO {table} (tenant_id, {key_column}, locale, display_name, description, copy_revision, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) ON CONFLICT (tenant_id, {key_column}, locale) DO NOTHING",
        table = kind.table(),
        key_column = kind.key_column(),
    );
    let result = db
        .execute_raw(statement(
            db,
            sql,
            vec![
                tenant_id.into(),
                key.to_owned().into(),
                locale.as_str().to_owned().into(),
                display_name.into(),
                description.into(),
            ],
        )?)
        .await?;
    if result.rows_affected() == 0 {
        return Err(RbacPresentationStoreError::AlreadyExists);
    }
    Ok(())
}

async fn compare_and_set<C: ConnectionTrait>(
    db: &C,
    kind: PresentationKind,
    tenant_id: Uuid,
    key: &str,
    locale: &StoredLocale,
    expected_copy_revision: i64,
    display_name: Option<String>,
    description: Option<String>,
) -> Result<(), RbacPresentationStoreError> {
    validate_identity(key)?;
    let current = load_exact(db, kind, tenant_id, key, locale)
        .await?
        .ok_or(RbacPresentationStoreError::NotFound)?;
    if current.copy_revision != expected_copy_revision {
        return Err(RbacPresentationStoreError::RevisionConflict {
            expected: expected_copy_revision,
        });
    }
    if current.display_name == display_name && current.description == description {
        return Ok(());
    }

    let sql = format!(
        "UPDATE {table} SET display_name = ?, description = ?, copy_revision = copy_revision + 1, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = ? AND {key_column} = ? AND locale = ? AND copy_revision = ?",
        table = kind.table(),
        key_column = kind.key_column(),
    );
    let result = db
        .execute_raw(statement(
            db,
            sql,
            vec![
                display_name.into(),
                description.into(),
                tenant_id.into(),
                key.to_owned().into(),
                locale.as_str().to_owned().into(),
                expected_copy_revision.into(),
            ],
        )?)
        .await?;
    if result.rows_affected() == 0 {
        return match load_exact(db, kind, tenant_id, key, locale).await? {
            Some(_) => Err(RbacPresentationStoreError::RevisionConflict {
                expected: expected_copy_revision,
            }),
            None => Err(RbacPresentationStoreError::NotFound),
        };
    }
    Ok(())
}

fn role_from_row(row: PresentationRow) -> Result<RbacRolePresentation, RbacPresentationStoreError> {
    Ok(RbacRolePresentation {
        tenant_id: row.tenant_id,
        role_slug: row.key,
        locale: row.locale,
        display_name: row.display_name,
        description: row.description,
        copy_revision: row.copy_revision,
    })
}

fn permission_from_row(
    row: PresentationRow,
) -> Result<RbacPermissionPresentation, RbacPresentationStoreError> {
    Ok(RbacPermissionPresentation {
        tenant_id: row.tenant_id,
        permission_key: row.key,
        locale: row.locale,
        display_name: row.display_name,
        description: row.description,
        copy_revision: row.copy_revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    async fn store() -> (SeaOrmRbacPresentationStore, Uuid, Uuid) {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            "CREATE TABLE roles (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL, name TEXT NOT NULL, slug TEXT NOT NULL, description TEXT, is_system BOOLEAN NOT NULL, UNIQUE (tenant_id, slug))",
        )
        .await
        .unwrap();
        db.execute_unprepared(
            "CREATE TABLE permissions (id BLOB PRIMARY KEY, tenant_id BLOB NOT NULL, resource TEXT NOT NULL, action TEXT NOT NULL, description TEXT, UNIQUE (tenant_id, resource, action))",
        )
        .await
        .unwrap();
        let tenant_id = Uuid::new_v4();
        let other_tenant_id = Uuid::new_v4();
        db.execute_raw(
            statement(
                &db,
                "INSERT INTO roles (id, tenant_id, name, slug, description, is_system) VALUES (?, ?, ?, ?, ?, TRUE)".to_string(),
                vec![
                    Uuid::new_v4().into(),
                    tenant_id.into(),
                    "Legacy admin".to_string().into(),
                    "admin".to_string().into(),
                    Some("legacy role copy".to_string()).into(),
                ],
            )
            .unwrap(),
        )
        .await
        .unwrap();
        db.execute_raw(
            statement(
                &db,
                "INSERT INTO permissions (id, tenant_id, resource, action, description) VALUES (?, ?, ?, ?, ?)".to_string(),
                vec![
                    Uuid::new_v4().into(),
                    tenant_id.into(),
                    "users".to_string().into(),
                    "manage".to_string().into(),
                    Some("legacy permission copy".to_string()).into(),
                ],
            )
            .unwrap(),
        )
        .await
        .unwrap();
        crate::m20260914_000001_role_permission_presentations::Migration
            .up(&SchemaManager::new(&db))
            .await
            .unwrap();
        (SeaOrmRbacPresentationStore::new(db), tenant_id, other_tenant_id)
    }

    #[test]
    fn owner_authored_locale_cannot_use_unknown_provenance() {
        assert!(RuntimeLocale::new("und").is_err());
    }

    #[tokio::test]
    async fn migration_preserves_unknown_legacy_provenance() {
        let (store, tenant_id, _) = store().await;
        let unknown = StoredLocale::new("und").unwrap();
        let role = store
            .find_role_exact(tenant_id, "admin", &unknown)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(role.display_name.as_deref(), Some("Legacy admin"));
        assert_eq!(role.description.as_deref(), Some("legacy role copy"));
        assert_eq!(role.copy_revision, 1);

        let permission = store
            .find_permission_exact(tenant_id, "users:manage", &unknown)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(permission.display_name, None);
        assert_eq!(
            permission.description.as_deref(),
            Some("legacy permission copy")
        );
        assert_eq!(permission.copy_revision, 1);
    }

    #[tokio::test]
    async fn source_copy_is_tenant_scoped_and_revisioned_independently() {
        let (store, tenant_id, other_tenant_id) = store().await;
        let en = RuntimeLocale::new("en").unwrap();
        let stored_en = StoredLocale::from(en.clone());
        let created = store
            .create_role_source(
                tenant_id,
                "admin",
                en.clone(),
                Some("Administrator".to_string()),
                Some("Tenant administrator".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(created.copy_revision, 1);
        assert!(
            store
                .find_role_exact(other_tenant_id, "admin", &stored_en)
                .await
                .unwrap()
                .is_none()
        );

        let updated = store
            .compare_and_set_role_source(
                tenant_id,
                "admin",
                &en,
                1,
                Some("Admin".to_string()),
                Some("Tenant administrator".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(updated.copy_revision, 2);
        assert!(matches!(
            store
                .compare_and_set_role_source(
                    tenant_id,
                    "admin",
                    &en,
                    1,
                    Some("stale".to_string()),
                    None,
                )
                .await,
            Err(RbacPresentationStoreError::RevisionConflict { expected: 1 })
        ));

        let legacy = StoredLocale::new("und").unwrap();
        let untouched = store
            .find_role_exact(tenant_id, "admin", &legacy)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(untouched.copy_revision, 1);
        assert_eq!(untouched.display_name.as_deref(), Some("Legacy admin"));
    }
}
