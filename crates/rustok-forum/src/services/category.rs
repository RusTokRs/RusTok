use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter,
};
use uuid::Uuid;

use crate::entities::forum_category;
use crate::error::{ForumError, ForumResult};

/// Crate-private persistence seam shared by Forum category owner commands.
///
/// Canonical Category reads and localized copy no longer live here: public reads
/// are Taxonomy-backed and public mutations are owned by
/// `CategoryProjectionOwnerService`. This type remains only for transaction
/// helpers reused by topic/reply/import owners.
pub(super) struct CategoryService;

impl CategoryService {
    pub(crate) async fn ensure_exists_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<()> {
        Self::find_category_in_tx(txn, tenant_id, category_id).await?;
        Ok(())
    }

    pub(crate) async fn find_category_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<forum_category::Model> {
        let existing = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?;
        existing.ok_or(ForumError::CategoryNotFound(category_id))
    }

    pub(crate) async fn adjust_counters_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        topic_delta: i32,
        reply_delta: i32,
    ) -> ForumResult<()> {
        let category = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?
            .ok_or(ForumError::CategoryNotFound(category_id))?;

        let mut active: forum_category::ActiveModel = category.clone().into();
        active.topic_count = Set((category.topic_count + topic_delta).max(0));
        active.reply_count = Set((category.reply_count + reply_delta).max(0));
        active.updated_at = Set(Utc::now().into());
        active.update(txn).await?;
        Ok(())
    }
}
