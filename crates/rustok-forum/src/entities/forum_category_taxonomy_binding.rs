use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::forum_category;
use crate::error::{ForumError, ForumResult};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_category_taxonomy_bindings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub forum_category_id: Uuid,
    pub taxonomy_category_id: Uuid,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForumCategoryTaxonomyBinding {
    pub forum_category_id: Uuid,
    pub taxonomy_category_id: Uuid,
}

/// Transitional CAT-5 binding seam from a Forum policy row to canonical
/// Taxonomy Category identity.
///
/// The relation is Forum-owned. It does not move legacy localized copy,
/// hierarchy, presentation, counters or Translation provider ownership yet.
pub struct ForumCategoryTaxonomyBindingService {
    db: DatabaseConnection,
}

impl ForumCategoryTaxonomyBindingService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn bind(
        &self,
        tenant_id: Uuid,
        forum_category_id: Uuid,
        taxonomy_category_id: Uuid,
    ) -> ForumResult<ForumCategoryTaxonomyBinding> {
        let txn = self.db.begin().await?;
        let binding = bind_in_tx(
            &txn,
            tenant_id,
            forum_category_id,
            taxonomy_category_id,
        )
        .await?;
        txn.commit().await?;
        Ok(binding)
    }
}

pub(crate) async fn bind_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    forum_category_id: Uuid,
    taxonomy_category_id: Uuid,
) -> ForumResult<ForumCategoryTaxonomyBinding> {
    if tenant_id.is_nil() || forum_category_id.is_nil() || taxonomy_category_id.is_nil() {
        return Err(ForumError::Validation(
            "Forum Taxonomy category binding requires non-nil tenant and category IDs".to_string(),
        ));
    }

    let taxonomy_exists = rustok_taxonomy::taxonomy_term_identity_exists(
        txn,
        tenant_id,
        rustok_taxonomy::TaxonomyTermKind::Category,
        taxonomy_category_id,
    )
    .await
    .map_err(map_taxonomy_error)?;
    if !taxonomy_exists {
        return Err(ForumError::Validation(
            "Forum category binding must reference a same-tenant Taxonomy Category".to_string(),
        ));
    }

    let forum_exists = forum_category::Entity::find_by_id(forum_category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .is_some();
    if !forum_exists {
        return Err(ForumError::CategoryNotFound(forum_category_id));
    }

    if let Some(existing) = Entity::find_by_id((tenant_id, forum_category_id))
        .one(txn)
        .await?
    {
        if existing.taxonomy_category_id == taxonomy_category_id {
            return Ok(ForumCategoryTaxonomyBinding {
                forum_category_id,
                taxonomy_category_id,
            });
        }
        return Err(ForumError::Validation(
            "Forum category is already bound to a different Taxonomy Category".to_string(),
        ));
    }

    let duplicate = Entity::find()
        .filter(Column::TenantId.eq(tenant_id))
        .filter(Column::TaxonomyCategoryId.eq(taxonomy_category_id))
        .one(txn)
        .await?;
    if duplicate.is_some() {
        return Err(ForumError::Validation(
            "Taxonomy Category is already bound to another Forum category".to_string(),
        ));
    }

    ActiveModel {
        tenant_id: Set(tenant_id),
        forum_category_id: Set(forum_category_id),
        taxonomy_category_id: Set(taxonomy_category_id),
        created_at: Set(chrono::Utc::now().into()),
    }
    .insert(txn)
    .await?;

    Ok(ForumCategoryTaxonomyBinding {
        forum_category_id,
        taxonomy_category_id,
    })
}

fn map_taxonomy_error(error: rustok_taxonomy::TaxonomyError) -> ForumError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => ForumError::Database(error),
        other => ForumError::Validation(format!(
            "Taxonomy Category identity validation failed: {other}"
        )),
    }
}
