use async_trait::async_trait;
use chrono::Utc;
use rustok_api::{RuntimeLocale, StoredLocale};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    ExprTrait, QueryFilter, QueryOrder, Set,
};
use thiserror::Error;
use uuid::Uuid;

use crate::model::ScriptPresentation;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "alloy_script_presentations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub script_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub locale: String,
    pub description: Option<String>,
    pub copy_revision: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Error)]
pub enum ScriptPresentationStoreError {
    #[error("Alloy script presentation was not found")]
    NotFound,
    #[error("Alloy script presentation already exists")]
    AlreadyExists,
    #[error("Alloy script presentation revision conflict; expected {expected}")]
    RevisionConflict { expected: i64 },
    #[error("Alloy script presentation contains an invalid stored locale")]
    InvalidStoredLocale,
    #[error("Alloy script presentation storage failed: {0}")]
    Storage(String),
}

impl From<DbErr> for ScriptPresentationStoreError {
    fn from(error: DbErr) -> Self {
        Self::Storage(error.to_string())
    }
}

#[async_trait]
pub trait ScriptPresentationStore: Send + Sync {
    async fn find_exact(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<ScriptPresentation>, ScriptPresentationStoreError>;

    async fn list_for_script(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
    ) -> Result<Vec<ScriptPresentation>, ScriptPresentationStoreError>;

    async fn create(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: StoredLocale,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError>;

    async fn compare_and_set(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
        expected_copy_revision: i64,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError>;
}

#[derive(Clone)]
pub struct SeaOrmScriptPresentationStore {
    db: DatabaseConnection,
}

impl SeaOrmScriptPresentationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn find_exact(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<ScriptPresentation>, ScriptPresentationStoreError> {
        Self::find_exact_on(&self.db, tenant_id, script_id, locale).await
    }

    /// Creates new owner-authored presentation copy inside a caller-owned
    /// transaction. `RuntimeLocale` is deliberately used here instead of
    /// `StoredLocale`: newly authored copy must have concrete provenance and
    /// can never silently become the storage-only `und` locale.
    pub async fn create_source_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        script_id: Uuid,
        source_locale: RuntimeLocale,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError> {
        Self::create_on(
            transaction,
            tenant_id,
            script_id,
            StoredLocale::from(source_locale),
            description,
        )
        .await
    }

    /// Applies a copy-only CAS for owner-authored presentation inside the
    /// caller's existing transaction. The concrete source locale requirement
    /// keeps legacy `und` provenance read-only at canonical authoring edges.
    pub async fn compare_and_set_source_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        script_id: Uuid,
        source_locale: &RuntimeLocale,
        expected_copy_revision: i64,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError> {
        Self::compare_and_set_on(
            transaction,
            tenant_id,
            script_id,
            &StoredLocale::from(source_locale.clone()),
            expected_copy_revision,
            description,
        )
        .await
    }

    async fn ensure_script_owned_on<C>(
        connection: &C,
        tenant_id: Uuid,
        script_id: Uuid,
    ) -> Result<(), ScriptPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        match super::ScriptsEntity::find_by_id(script_id)
            .one(connection)
            .await?
        {
            Some(script) if script.tenant_id == tenant_id => Ok(()),
            _ => Err(ScriptPresentationStoreError::NotFound),
        }
    }

    async fn load_model_on<C>(
        connection: &C,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<Model>, ScriptPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        Entity::find_by_id((tenant_id, script_id, locale.as_str().to_owned()))
            .one(connection)
            .await
            .map_err(Into::into)
    }

    async fn find_exact_on<C>(
        connection: &C,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<ScriptPresentation>, ScriptPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        Self::load_model_on(connection, tenant_id, script_id, locale)
            .await?
            .map(try_into_domain)
            .transpose()
    }

    async fn create_on<C>(
        connection: &C,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: StoredLocale,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        Self::ensure_script_owned_on(connection, tenant_id, script_id).await?;
        if Self::load_model_on(connection, tenant_id, script_id, &locale)
            .await?
            .is_some()
        {
            return Err(ScriptPresentationStoreError::AlreadyExists);
        }

        let now = Utc::now().fixed_offset();
        let row = ActiveModel {
            tenant_id: Set(tenant_id),
            script_id: Set(script_id),
            locale: Set(locale.as_str().to_owned()),
            description: Set(description),
            copy_revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(connection)
        .await?;

        try_into_domain(row)
    }

    async fn compare_and_set_on<C>(
        connection: &C,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
        expected_copy_revision: i64,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError>
    where
        C: ConnectionTrait,
    {
        let current = Self::load_model_on(connection, tenant_id, script_id, locale)
            .await?
            .ok_or(ScriptPresentationStoreError::NotFound)?;
        if current.copy_revision != expected_copy_revision {
            return Err(ScriptPresentationStoreError::RevisionConflict {
                expected: expected_copy_revision,
            });
        }
        if current.description == description {
            return try_into_domain(current);
        }

        let now = Utc::now().fixed_offset();
        let result = Entity::update_many()
            .col_expr(Column::Description, Expr::value(description))
            .col_expr(Column::CopyRevision, Expr::col(Column::CopyRevision).add(1))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ScriptId.eq(script_id))
            .filter(Column::Locale.eq(locale.as_str()))
            .filter(Column::CopyRevision.eq(expected_copy_revision))
            .exec(connection)
            .await?;

        if result.rows_affected == 0 {
            return match Self::load_model_on(connection, tenant_id, script_id, locale).await? {
                Some(_) => Err(ScriptPresentationStoreError::RevisionConflict {
                    expected: expected_copy_revision,
                }),
                None => Err(ScriptPresentationStoreError::NotFound),
            };
        }

        Self::find_exact_on(connection, tenant_id, script_id, locale)
            .await?
            .ok_or(ScriptPresentationStoreError::NotFound)
    }
}

#[async_trait]
impl ScriptPresentationStore for SeaOrmScriptPresentationStore {
    async fn find_exact(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<ScriptPresentation>, ScriptPresentationStoreError> {
        Self::find_exact_on(&self.db, tenant_id, script_id, locale).await
    }

    async fn list_for_script(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
    ) -> Result<Vec<ScriptPresentation>, ScriptPresentationStoreError> {
        Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ScriptId.eq(script_id))
            .order_by_asc(Column::Locale)
            .all(&self.db)
            .await?
            .into_iter()
            .map(try_into_domain)
            .collect()
    }

    async fn create(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: StoredLocale,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError> {
        Self::create_on(&self.db, tenant_id, script_id, locale, description).await
    }

    async fn compare_and_set(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
        expected_copy_revision: i64,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError> {
        Self::compare_and_set_on(
            &self.db,
            tenant_id,
            script_id,
            locale,
            expected_copy_revision,
            description,
        )
        .await
    }
}

fn try_into_domain(model: Model) -> Result<ScriptPresentation, ScriptPresentationStoreError> {
    let locale = StoredLocale::new(&model.locale)
        .map_err(|_| ScriptPresentationStoreError::InvalidStoredLocale)?;
    Ok(ScriptPresentation {
        tenant_id: model.tenant_id,
        script_id: model.script_id,
        locale,
        description: model.description,
        copy_revision: model.copy_revision,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RhaiWorkspace, Script, ScriptRegistry, ScriptTrigger};
    use sea_orm::{Database, TransactionTrait};
    use sea_orm_migration::prelude::SchemaManager;

    async fn store_with_script() -> (SeaOrmScriptPresentationStore, Uuid, Uuid) {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory database should connect");
        let manager = SchemaManager::new(&database);
        for migration in crate::migrations::migrations() {
            migration
                .up(&manager)
                .await
                .expect("Alloy migrations should apply");
        }
        let tenant_id = Uuid::new_v4();
        let storage = crate::SeaOrmStorage::new(database.clone()).for_tenant(tenant_id);
        let mut script = Script::new(
            "localized-owner",
            RhaiWorkspace::single_source("40 + 2"),
            ScriptTrigger::Manual,
        );
        script.tenant_id = tenant_id;
        let script = storage.save(script).await.expect("script should save");
        (
            SeaOrmScriptPresentationStore::new(database),
            tenant_id,
            script.id,
        )
    }

    #[test]
    fn new_source_locales_cannot_use_unknown_provenance() {
        assert!(RuntimeLocale::new("und").is_err());
    }

    #[tokio::test]
    async fn source_write_participates_in_caller_transaction() {
        let (store, tenant_id, script_id) = store_with_script().await;
        let source_locale = RuntimeLocale::new("en").expect("concrete source locale");
        let stored_locale = StoredLocale::from(source_locale.clone());

        let transaction = store
            .connection()
            .begin()
            .await
            .expect("presentation transaction should begin");
        store
            .create_source_in_transaction(
                &transaction,
                tenant_id,
                script_id,
                source_locale.clone(),
                Some("rolled back copy".to_string()),
            )
            .await
            .expect("presentation should be visible inside its transaction");
        transaction
            .rollback()
            .await
            .expect("presentation transaction should roll back");
        assert!(
            store
                .find_exact(tenant_id, script_id, &stored_locale)
                .await
                .expect("presentation lookup should succeed")
                .is_none()
        );

        let transaction = store
            .connection()
            .begin()
            .await
            .expect("presentation transaction should begin");
        let created = store
            .create_source_in_transaction(
                &transaction,
                tenant_id,
                script_id,
                source_locale,
                Some("committed copy".to_string()),
            )
            .await
            .expect("presentation should be created");
        transaction
            .commit()
            .await
            .expect("presentation transaction should commit");
        assert_eq!(created.copy_revision, 1);
        assert_eq!(created.locale, stored_locale);
    }

    #[tokio::test]
    async fn transactional_source_cas_preserves_copy_only_revision_semantics() {
        let (store, tenant_id, script_id) = store_with_script().await;
        let source_locale = RuntimeLocale::new("pt_br").expect("concrete source locale");
        let stored_locale = StoredLocale::from(source_locale.clone());
        store
            .create(
                tenant_id,
                script_id,
                stored_locale.clone(),
                Some("Primeira descrição".to_string()),
            )
            .await
            .expect("initial presentation should persist");

        let transaction = store
            .connection()
            .begin()
            .await
            .expect("presentation transaction should begin");
        let unchanged = store
            .compare_and_set_source_in_transaction(
                &transaction,
                tenant_id,
                script_id,
                &source_locale,
                1,
                Some("Primeira descrição".to_string()),
            )
            .await
            .expect("exact copy replay should succeed");
        assert_eq!(unchanged.copy_revision, 1);
        let changed = store
            .compare_and_set_source_in_transaction(
                &transaction,
                tenant_id,
                script_id,
                &source_locale,
                1,
                Some("Descrição revista".to_string()),
            )
            .await
            .expect("semantic copy change should succeed");
        assert_eq!(changed.copy_revision, 2);
        transaction
            .commit()
            .await
            .expect("presentation transaction should commit");
    }
}
