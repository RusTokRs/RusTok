use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder,
};
use uuid::Uuid;

use crate::entities::{
    forum_category, forum_category_taxonomy_binding, forum_category_translation,
};
use crate::error::{ForumError, ForumResult};

const FORUM_TAXONOMY_SCOPE: &str = "forum";

pub(in crate::services) async fn sync_category_locale_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> ForumResult<()> {
    let category = forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::CategoryNotFound(category_id))?;
    let translation = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(category_id))
        .filter(forum_category_translation::Column::Locale.eq(locale))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(format!(
                "Forum category {category_id} has no localized copy for Taxonomy synchronization"
            ))
        })?;

    rustok_taxonomy::sync_module_category_with_owned_aliases_in_tx(
        txn,
        tenant_id,
        rustok_taxonomy::SyncModuleCategoryInput {
            category_id,
            module_scope: FORUM_TAXONOMY_SCOPE.to_string(),
            canonical_key: canonical_key_for_forum_category(category_id),
            locale: translation.locale,
            name: translation.name,
            slug: translation.slug,
            aliases: Vec::new(),
            description: translation.description,
            parent_id: category.parent_id,
            position: category.position,
            icon_key: category.icon,
            color: category.color,
        },
    )
    .await
    .map_err(map_taxonomy_error)?;

    ensure_same_id_binding_in_tx(txn, tenant_id, category_id).await
}

pub(in crate::services) async fn sync_category_any_locale_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    let translation = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(category_id))
        .order_by_asc(forum_category_translation::Column::Locale)
        .order_by_asc(forum_category_translation::Column::Id)
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(format!(
                "Forum category {category_id} has no localized copy for Taxonomy synchronization"
            ))
        })?;
    sync_category_locale_in_tx(txn, tenant_id, category_id, &translation.locale).await
}

pub(in crate::services) async fn sync_siblings_for_parent_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
) -> ForumResult<()> {
    let categories = forum_category::Entity::find()
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(forum_category::Column::Position)
        .order_by_asc(forum_category::Column::Id)
        .all(txn)
        .await?;
    for category in categories
        .into_iter()
        .filter(|category| category.parent_id == parent_id)
    {
        sync_category_any_locale_in_tx(txn, tenant_id, category.id).await?;
    }
    Ok(())
}

async fn ensure_same_id_binding_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    if let Some(existing) =
        forum_category_taxonomy_binding::Entity::find_by_id((tenant_id, category_id))
            .one(txn)
            .await?
    {
        if existing.taxonomy_category_id == category_id {
            return Ok(());
        }
        return Err(ForumError::Validation(format!(
            "Forum category {category_id} is bound to a different Taxonomy Category"
        )));
    }

    if let Some(existing) = forum_category_taxonomy_binding::Entity::find()
        .filter(forum_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
        .filter(forum_category_taxonomy_binding::Column::TaxonomyCategoryId.eq(category_id))
        .one(txn)
        .await?
    {
        return Err(ForumError::Validation(format!(
            "Taxonomy Category {category_id} is already bound to Forum category {}",
            existing.forum_category_id
        )));
    }

    forum_category_taxonomy_binding::ActiveModel {
        tenant_id: Set(tenant_id),
        forum_category_id: Set(category_id),
        taxonomy_category_id: Set(category_id),
        created_at: Set(Utc::now().into()),
    }
    .insert(txn)
    .await?;
    Ok(())
}

fn canonical_key_for_forum_category(category_id: Uuid) -> String {
    format!("forum-category-{category_id}")
}

fn map_taxonomy_error(error: rustok_taxonomy::TaxonomyError) -> ForumError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => ForumError::Database(error),
        other => ForumError::Validation(format!(
            "Forum Category Taxonomy synchronization failed: {other}"
        )),
    }
}
