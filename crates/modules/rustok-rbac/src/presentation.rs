use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{RuntimeLocale, StoredLocale, TenantRbacCatalog};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, EntityTrait, ExprTrait, QueryFilter,
    QueryOrder, Set,
};
use thiserror::Error;
use uuid::Uuid;

use crate::BuiltinTenantRbacCatalog;

const ROLE_KIND: &str = "role";
const PERMISSION_KIND: &str = "permission";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RbacPresentationResourceKind {
    Role,
    Permission,
}

impl RbacPresentationResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Role => ROLE_KIND,
            Self::Permission => PERMISSION_KIND,
        }
    }

    fn from_stored(value: &str) -> Option<Self> {
        match value {
            ROLE_KIND => Some(Self::Role),
            PERMISSION_KIND => Some(Self::Permission),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RbacLocalizedPresentation {
    pub tenant_id: Uuid,
    pub resource_kind: RbacPresentationResourceKind,
    pub resource_key: String,
    pub locale: StoredLocale,
    pub name: String,
    pub description: Option<String>,
    pub copy_revision: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "rbac_localized_presentations")]
struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    resource_kind: String,
    #[sea_orm(primary_key, auto_increment = false)]
    resource_key: String,
    #[sea_orm(primary_key, auto_increment = false)]
    locale: String,
    name: String,
    description: Option<String>,
    copy_revision: i64,
    created_at: DateTimeWithTimeZone,
    updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Error)]
pub enum RbacPresentationStoreError {
    #[error("RBAC presentation resource is not part of the admitted owner catalog")]
    UnknownResource,
    #[error("RBAC presentation row was not found")]
    NotFound,
    #[error("RBAC presentation row already exists")]
    AlreadyExists,
    #[error("RBAC presentation revision conflict; expected {expected}")]
    RevisionConflict { expected: i64 },
    #[error("RBAC presentation row contains an invalid stored resource kind")]
    InvalidStoredResourceKind,
    #[error("RBAC presentation row contains an invalid stored locale")]
    InvalidStoredLocale,
    #[error("RBAC presentation storage failed: {0}")]
    Storage(String),
}

impl From<DbErr> for RbacPresentationStoreError {
    fn from(error: DbErr) -> Self {
        Self::Storage(error.to_string())
    }
}

#[async_trait]
pub trait RbacPresentationStore: Send + Sync {
    async fn find_exact(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        locale: &StoredLocale,
    ) -> Result<Option<RbacLocalizedPresentation>, RbacPresentationStoreError>;

    async fn list_for_resource(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
    ) -> Result<Vec<RbacLocalizedPresentation>, RbacPresentationStoreError>;

    async fn create_source(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        source_locale: RuntimeLocale,
        name: String,
        description: Option<String>,
    ) -> Result<RbacLocalizedPresentation, RbacPresentationStoreError>;

    async fn compare_and_set_source(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        source_locale: &RuntimeLocale,
        expected_copy_revision: i64,
        name: String,
        description: Option<String>,
    ) -> Result<RbacLocalizedPresentation, RbacPresentationStoreError>;
}

#[derive(Clone)]
pub struct SeaOrmRbacPresentationStore {
    db: DatabaseConnection,
}

impl SeaOrmRbacPresentationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    fn ensure_known_resource(
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
    ) -> Result<(), RbacPresentationStoreError> {
        let catalog = BuiltinTenantRbacCatalog;
        let known = match resource_kind {
            RbacPresentationResourceKind::Role => catalog
                .roles(tenant_id)
                .into_iter()
                .any(|role| role.slug == resource_key),
            RbacPresentationResourceKind::Permission => catalog
                .permissions(tenant_id)
                .into_iter()
                .any(|permission| permission.slug == resource_key),
        };
        if known {
            Ok(())
        } else {
            Err(RbacPresentationStoreError::UnknownResource)
        }
    }

    async fn load_model_on<C>(
        connection: &C,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        locale: &StoredLocale,
    ) -> Result<Option<Model>, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        Entity::find_by_id((
            tenant_id,
            resource_kind.as_str().to_owned(),
            resource_key.to_owned(),
            locale.as_str().to_owned(),
        ))
        .one(connection)
        .await
        .map_err(Into::into)
    }

    async fn find_exact_on<C>(
        connection: &C,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        locale: &StoredLocale,
    ) -> Result<Option<RbacLocalizedPresentation>, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        Self::load_model_on(connection, tenant_id, resource_kind, resource_key, locale)
            .await?
            .map(try_into_domain)
            .transpose()
    }

    async fn create_source_on<C>(
        connection: &C,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        source_locale: RuntimeLocale,
        name: String,
        description: Option<String>,
    ) -> Result<RbacLocalizedPresentation, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        Self::ensure_known_resource(tenant_id, resource_kind, resource_key)?;
        let locale = StoredLocale::from(source_locale);
        if Self::load_model_on(connection, tenant_id, resource_kind, resource_key, &locale)
            .await?
            .is_some()
        {
            return Err(RbacPresentationStoreError::AlreadyExists);
        }

        let now = Utc::now().fixed_offset();
        let row = ActiveModel {
            tenant_id: Set(tenant_id),
            resource_kind: Set(resource_kind.as_str().to_owned()),
            resource_key: Set(resource_key.to_owned()),
            locale: Set(locale.as_str().to_owned()),
            name: Set(name),
            description: Set(description),
            copy_revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(connection)
        .await?;

        try_into_domain(row)
    }

    async fn compare_and_set_source_on<C>(
        connection: &C,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        source_locale: &RuntimeLocale,
        expected_copy_revision: i64,
        name: String,
        description: Option<String>,
    ) -> Result<RbacLocalizedPresentation, RbacPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        Self::ensure_known_resource(tenant_id, resource_kind, resource_key)?;
        let locale = StoredLocale::from(source_locale.clone());
        let current = Self::load_model_on(
            connection,
            tenant_id,
            resource_kind,
            resource_key,
            &locale,
        )
        .await?
        .ok_or(RbacPresentationStoreError::NotFound)?;
        if current.copy_revision != expected_copy_revision {
            return Err(RbacPresentationStoreError::RevisionConflict {
                expected: expected_copy_revision,
            });
        }
        if current.name == name && current.description == description {
            return try_into_domain(current);
        }

        let now = Utc::now().fixed_offset();
        let result = Entity::update_many()
            .col_expr(Column::Name, Expr::value(name))
            .col_expr(Column::Description, Expr::value(description))
            .col_expr(Column::CopyRevision, Expr::col(Column::CopyRevision).add(1))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ResourceKind.eq(resource_kind.as_str()))
            .filter(Column::ResourceKey.eq(resource_key))
            .filter(Column::Locale.eq(locale.as_str()))
            .filter(Column::CopyRevision.eq(expected_copy_revision))
            .exec(connection)
            .await?;

        if result.rows_affected == 0 {
            return match Self::load_model_on(
                connection,
                tenant_id,
                resource_kind,
                resource_key,
                &locale,
            )
            .await?
            {
                Some(_) => Err(RbacPresentationStoreError::RevisionConflict {
                    expected: expected_copy_revision,
                }),
                None => Err(RbacPresentationStoreError::NotFound),
            };
        }

        Self::find_exact_on(connection, tenant_id, resource_kind, resource_key, &locale)
            .await?
            .ok_or(RbacPresentationStoreError::NotFound)
    }
}

#[async_trait]
impl RbacPresentationStore for SeaOrmRbacPresentationStore {
    async fn find_exact(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        locale: &StoredLocale,
    ) -> Result<Option<RbacLocalizedPresentation>, RbacPresentationStoreError> {
        Self::find_exact_on(
            &self.db,
            tenant_id,
            resource_kind,
            resource_key,
            locale,
        )
        .await
    }

    async fn list_for_resource(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
    ) -> Result<Vec<RbacLocalizedPresentation>, RbacPresentationStoreError> {
        Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ResourceKind.eq(resource_kind.as_str()))
            .filter(Column::ResourceKey.eq(resource_key))
            .order_by_asc(Column::Locale)
            .all(&self.db)
            .await?
            .into_iter()
            .map(try_into_domain)
            .collect()
    }

    async fn create_source(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        source_locale: RuntimeLocale,
        name: String,
        description: Option<String>,
    ) -> Result<RbacLocalizedPresentation, RbacPresentationStoreError> {
        Self::create_source_on(
            &self.db,
            tenant_id,
            resource_kind,
            resource_key,
            source_locale,
            name,
            description,
        )
        .await
    }

    async fn compare_and_set_source(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        source_locale: &RuntimeLocale,
        expected_copy_revision: i64,
        name: String,
        description: Option<String>,
    ) -> Result<RbacLocalizedPresentation, RbacPresentationStoreError> {
        Self::compare_and_set_source_on(
            &self.db,
            tenant_id,
            resource_kind,
            resource_key,
            source_locale,
            expected_copy_revision,
            name,
            description,
        )
        .await
    }
}

fn try_into_domain(model: Model) -> Result<RbacLocalizedPresentation, RbacPresentationStoreError> {
    let resource_kind = RbacPresentationResourceKind::from_stored(&model.resource_kind)
        .ok_or(RbacPresentationStoreError::InvalidStoredResourceKind)?;
    let locale = StoredLocale::new(&model.locale)
        .map_err(|_| RbacPresentationStoreError::InvalidStoredLocale)?;
    Ok(RbacLocalizedPresentation {
        tenant_id: model.tenant_id,
        resource_kind,
        resource_key: model.resource_key,
        locale,
        name: model.name,
        description: model.description,
        copy_revision: model.copy_revision,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::*;

    async fn store() -> SeaOrmRbacPresentationStore {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite RBAC presentation database should connect");
        let manager = SchemaManager::new(&database);
        crate::m20260914_000001_localized_presentations::Migration
            .up(&manager)
            .await
            .expect("RBAC presentation migration should apply");
        SeaOrmRbacPresentationStore::new(database)
    }

    #[test]
    fn canonical_source_writes_reject_unknown_provenance() {
        assert!(RuntimeLocale::new("und").is_err());
    }

    #[tokio::test]
    async fn source_copy_is_identity_bound_and_uses_copy_only_cas() {
        let store = store().await;
        let tenant_id = Uuid::new_v4();
        let source_locale = RuntimeLocale::new("en").expect("concrete source locale");
        let stored_locale = StoredLocale::from(source_locale.clone());

        assert!(matches!(
            store
                .create_source(
                    tenant_id,
                    RbacPresentationResourceKind::Role,
                    "not-a-role",
                    source_locale.clone(),
                    "Unknown".to_string(),
                    None,
                )
                .await,
            Err(RbacPresentationStoreError::UnknownResource)
        ));

        let created = store
            .create_source(
                tenant_id,
                RbacPresentationResourceKind::Role,
                "admin",
                source_locale.clone(),
                "Administrator".to_string(),
                Some("Tenant administrator".to_string()),
            )
            .await
            .expect("known role presentation should persist");
        assert_eq!(created.copy_revision, 1);

        let replay = store
            .compare_and_set_source(
                tenant_id,
                RbacPresentationResourceKind::Role,
                "admin",
                &source_locale,
                1,
                "Administrator".to_string(),
                Some("Tenant administrator".to_string()),
            )
            .await
            .expect("exact presentation replay should be idempotent");
        assert_eq!(replay.copy_revision, 1);

        let changed = store
            .compare_and_set_source(
                tenant_id,
                RbacPresentationResourceKind::Role,
                "admin",
                &source_locale,
                1,
                "Admin".to_string(),
                Some("Tenant administrator".to_string()),
            )
            .await
            .expect("copy-only update should succeed");
        assert_eq!(changed.copy_revision, 2);
        assert_eq!(changed.locale, stored_locale);
    }
}
