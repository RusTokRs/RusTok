use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use uuid::Uuid;

use crate::entities::forum_category;
use crate::error::{ForumError, ForumResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ForumCategoryTaxonomyBinding {
    pub forum_category_id: Uuid,
    pub taxonomy_category_id: Uuid,
}

/// Transitional CAT-5 migration seam from Forum-owned category rows to canonical
/// Taxonomy Category identities.
///
/// Binding is intentionally narrower than category read/write cutover: legacy
/// Forum hierarchy, localized copy, presentation and Translation ownership stay
/// live until deterministic backfill and parity evidence are complete.
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
        if tenant_id.is_nil() || forum_category_id.is_nil() || taxonomy_category_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum Taxonomy category binding requires non-nil tenant and category IDs"
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
        .await
        .map_err(map_taxonomy_error)?;
        if !taxonomy_exists {
            return Err(ForumError::Validation(
                "Forum category binding must reference a same-tenant Taxonomy Category".to_string(),
            ));
        }

        let category = forum_category::Entity::find_by_id(forum_category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(forum_category_id))?;

        match category.taxonomy_category_id {
            Some(existing) if existing == taxonomy_category_id => {
                txn.commit().await?;
                return Ok(ForumCategoryTaxonomyBinding {
                    forum_category_id,
                    taxonomy_category_id,
                });
            }
            Some(_) => {
                return Err(ForumError::Validation(
                    "Forum category is already bound to a different Taxonomy Category".to_string(),
                ));
            }
            None => {}
        }

        let duplicate = forum_category::Entity::find()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(forum_category::Column::TaxonomyCategoryId.eq(taxonomy_category_id))
            .filter(forum_category::Column::Id.ne(forum_category_id))
            .one(&txn)
            .await?;
        if duplicate.is_some() {
            return Err(ForumError::Validation(
                "Taxonomy Category is already bound to another Forum category".to_string(),
            ));
        }

        let mut active: forum_category::ActiveModel = category.into();
        active.taxonomy_category_id = Set(Some(taxonomy_category_id));
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        txn.commit().await?;

        Ok(ForumCategoryTaxonomyBinding {
            forum_category_id,
            taxonomy_category_id,
        })
    }
}

fn map_taxonomy_error(error: rustok_taxonomy::TaxonomyError) -> ForumError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => ForumError::Database(error),
        other => ForumError::Validation(format!(
            "Taxonomy Category identity validation failed: {other}"
        )),
    }
}
