use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};
use crate::ports::taxonomy::{
    ForumTaxonomyCategoryLocaleCopy, ForumTaxonomyCategorySnapshot, ForumTaxonomyPort,
    ForumTaxonomySyncCategoryInput,
};

pub(in crate::services) async fn load_category_locale_copy_in_tx(
    port: &dyn ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> ForumResult<Option<ForumTaxonomyCategoryLocaleCopy>> {
    port.load_category_locale_copy_in_tx(txn, tenant_id, category_id, locale)
        .await
}

pub(in crate::services) async fn sync_category_copy_in_tx(
    port: &dyn ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
    icon_key: Option<String>,
    color: Option<String>,
    locale: String,
    name: String,
    slug: String,
    description: Option<String>,
) -> ForumResult<()> {
    port.sync_category_copy_in_tx(
        txn,
        tenant_id,
        ForumTaxonomySyncCategoryInput {
            category_id,
            parent_id,
            position,
            icon_key,
            color,
            locale,
            name,
            slug,
            description,
        },
    )
    .await
}

pub(in crate::services) async fn load_category_owner_snapshot_in_tx(
    port: &dyn ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> ForumResult<ForumTaxonomyCategorySnapshot> {
    port.lock_category_hierarchy_writer_in_tx(txn, tenant_id)
        .await?;
    let categories = port
        .load_scoped_categories(
            txn.into(),
            tenant_id,
            Some(&[category_id]),
            locale,
            Some(rustok_api::PLATFORM_FALLBACK_LOCALE),
        )
        .await?;
    categories
        .into_iter()
        .next()
        .ok_or(ForumError::CategoryNotFound(category_id))
}

pub(in crate::services) async fn load_category_siblings_in_tx(
    port: &dyn ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
) -> ForumResult<Vec<Uuid>> {
    port.load_category_siblings_in_tx(txn, tenant_id, parent_id)
        .await
}

pub(in crate::services) async fn shift_category_siblings_for_insert_in_tx(
    port: &dyn ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    parent_id: Option<Uuid>,
    requested_position: i32,
) -> ForumResult<()> {
    port.shift_category_siblings_for_insert_in_tx(
        txn,
        tenant_id,
        category_id,
        parent_id,
        requested_position,
    )
    .await
}

pub(in crate::services) async fn move_category_in_tx(
    port: &dyn ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
) -> ForumResult<Vec<crate::dto::CategoryPlacementResponse>> {
    port.move_category_in_tx(txn, tenant_id, category_id, parent_id, position)
        .await
        .map(|placements| {
            placements
                .into_iter()
                .map(|p| crate::dto::CategoryPlacementResponse {
                    id: p.term_id,
                    parent_id: p.parent_id,
                    position: p.position,
                })
                .collect()
        })
}

pub(in crate::services) async fn reorder_category_siblings_in_tx(
    port: &dyn ForumTaxonomyPort,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    ordered_ids: &[Uuid],
) -> ForumResult<()> {
    port.reorder_category_siblings_in_tx(txn, tenant_id, parent_id, ordered_ids)
        .await
}
