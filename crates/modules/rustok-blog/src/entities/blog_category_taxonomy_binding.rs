use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::blog_category;
use crate::error::{BlogError, BlogResult};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "blog_category_taxonomy_bindings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub blog_category_id: Uuid,
    pub taxonomy_category_id: Uuid,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlogCategoryTaxonomyBinding {
    pub blog_category_id: Uuid,
    pub taxonomy_category_id: Uuid,
}

/// Transitional CAT-6 binding seam from a Blog category row to canonical
/// Taxonomy Category identity.
///
/// The relation is Blog-owned. This slice does not move localized copy,
/// hierarchy, presentation, routes, post counts, settings or Translation
/// provider ownership yet.
pub struct BlogCategoryTaxonomyBindingService {
    db: DatabaseConnection,
}

impl BlogCategoryTaxonomyBindingService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn bind(
        &self,
        tenant_id: Uuid,
        blog_category_id: Uuid,
        taxonomy_category_id: Uuid,
    ) -> BlogResult<BlogCategoryTaxonomyBinding> {
        if tenant_id.is_nil() || blog_category_id.is_nil() || taxonomy_category_id.is_nil() {
            return Err(BlogError::Validation(
                "Blog Taxonomy category binding requires non-nil tenant and category IDs"
                    .to_string(),
            ));
        }

        let txn = self.db.begin().await?;
        let taxonomy_exists = rustok_taxonomy::taxonomy_term_identity_exists(
            &txn,
            tenant_id,
            rustok_taxonomy::TaxonomyTermKind::Category,
            taxonomy_category_id,
        )
        .await?;
        if !taxonomy_exists {
            return Err(BlogError::Validation(
                "Blog category binding must reference a same-tenant Taxonomy Category".to_string(),
            ));
        }

        let blog_exists = blog_category::Entity::find_by_id(blog_category_id)
            .filter(blog_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .is_some();
        if !blog_exists {
            return Err(BlogError::CategoryNotFound(blog_category_id));
        }

        if let Some(existing) = Entity::find_by_id((tenant_id, blog_category_id))
            .one(&txn)
            .await?
        {
            if existing.taxonomy_category_id == taxonomy_category_id {
                txn.commit().await?;
                return Ok(BlogCategoryTaxonomyBinding {
                    blog_category_id,
                    taxonomy_category_id,
                });
            }
            return Err(BlogError::Validation(
                "Blog category is already bound to a different Taxonomy Category".to_string(),
            ));
        }

        let duplicate = Entity::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::TaxonomyCategoryId.eq(taxonomy_category_id))
            .one(&txn)
            .await?;
        if duplicate.is_some() {
            return Err(BlogError::Validation(
                "Taxonomy Category is already bound to another Blog category".to_string(),
            ));
        }

        ActiveModel {
            tenant_id: Set(tenant_id),
            blog_category_id: Set(blog_category_id),
            taxonomy_category_id: Set(taxonomy_category_id),
            created_at: Set(chrono::Utc::now().into()),
        }
        .insert(&txn)
        .await?;
        txn.commit().await?;

        Ok(BlogCategoryTaxonomyBinding {
            blog_category_id,
            taxonomy_category_id,
        })
    }
}
