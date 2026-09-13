use async_trait::async_trait;
use chrono::Utc;
use rustok_api::StoredLocale;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set};
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

    async fn load_model(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
    ) -> Result<Option<Model>, ScriptPresentationStoreError> {
        Entity::find_by_id((tenant_id, script_id, locale.as_str().to_owned()))
            .one(&self.db)
            .await
            .map_err(Into::into)
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
        self.load_model(tenant_id, script_id, locale)
            .await?
            .map(try_into_domain)
            .transpose()
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
        if self
            .load_model(tenant_id, script_id, &locale)
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
        .insert(&self.db)
        .await?;

        try_into_domain(row)
    }

    async fn compare_and_set(
        &self,
        tenant_id: Uuid,
        script_id: Uuid,
        locale: &StoredLocale,
        expected_copy_revision: i64,
        description: Option<String>,
    ) -> Result<ScriptPresentation, ScriptPresentationStoreError> {
        let now = Utc::now().fixed_offset();
        let result = Entity::update_many()
            .col_expr(Column::Description, Expr::value(description))
            .col_expr(
                Column::CopyRevision,
                Expr::col(Column::CopyRevision).add(1),
            )
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::ScriptId.eq(script_id))
            .filter(Column::Locale.eq(locale.as_str()))
            .filter(Column::CopyRevision.eq(expected_copy_revision))
            .exec(&self.db)
            .await?;

        if result.rows_affected == 0 {
            return match self.load_model(tenant_id, script_id, locale).await? {
                Some(_) => Err(ScriptPresentationStoreError::RevisionConflict {
                    expected: expected_copy_revision,
                }),
                None => Err(ScriptPresentationStoreError::NotFound),
            };
        }

        self.find_exact(tenant_id, script_id, locale)
            .await?
            .ok_or(ScriptPresentationStoreError::NotFound)
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
