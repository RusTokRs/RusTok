use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder,
};
use uuid::Uuid;

use crate::entities::{blog_category, blog_category_taxonomy_binding};
use crate::error::{BlogError, BlogResult};

const BLOG_TAXONOMY_SCOPE: &str = "blog";

pub(in crate::services) async fn sync_category_copy_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: String,
    name: String,
    slug: String,
    description: Option<String>,
) -> BlogResult<()> {
    let category = blog_category::Entity::find_by_id(category_id)
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(BlogError::CategoryNotFound(category_id))?;

    rustok_taxonomy::sync_module_category_with_owned_aliases_in_tx(
        txn,
        tenant_id,
        rustok_taxonomy::SyncModuleCategoryInput {
            category_id,
            module_scope: BLOG_TAXONOMY_SCOPE.to_string(),
            canonical_key: canonical_key_for_blog_category(category_id),
            locale,
            name,
            slug,
            aliases: Vec::new(),
            description,
            parent_id: category.parent_id,
            position: category.position,
            icon_key: None,
            color: None,
        },
    )
    .await
    .map_err(map_taxonomy_error)?;

    ensure_same_id_binding_in_tx(txn, tenant_id, category_id).await
}

pub(in crate::services) async fn sync_category_structure_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> BlogResult<()> {
    let category = blog_category::Entity::find_by_id(category_id)
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(BlogError::CategoryNotFound(category_id))?;

    rustok_taxonomy::sync_module_category_structure_with_owned_copy_in_tx(
        txn,
        tenant_id,
        category_id,
        BLOG_TAXONOMY_SCOPE,
        canonical_key_for_blog_category(category_id),
        category.parent_id,
        category.position,
        None,
        None,
    )
    .await
    .map_err(map_taxonomy_error)?;

    ensure_same_id_binding_in_tx(txn, tenant_id, category_id).await
}

pub(in crate::services) async fn sync_siblings_for_parent_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
) -> BlogResult<()> {
    let categories = blog_category::Entity::find()
        .filter(blog_category::Column::TenantId.eq(tenant_id))
        .order_by_asc(blog_category::Column::Position)
        .order_by_asc(blog_category::Column::Id)
        .all(txn)
        .await?;
    for category in categories
        .into_iter()
        .filter(|category| category.parent_id == parent_id)
    {
        sync_category_structure_in_tx(txn, tenant_id, category.id).await?;
    }
    Ok(())
}

async fn ensure_same_id_binding_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> BlogResult<()> {
    if let Some(existing) =
        blog_category_taxonomy_binding::Entity::find_by_id((tenant_id, category_id))
            .one(txn)
            .await?
    {
        if existing.taxonomy_category_id == category_id {
            return Ok(());
        }
        return Err(BlogError::Validation(format!(
            "Blog category {category_id} is bound to a different Taxonomy Category"
        )));
    }

    if let Some(existing) = blog_category_taxonomy_binding::Entity::find()
        .filter(blog_category_taxonomy_binding::Column::TenantId.eq(tenant_id))
        .filter(blog_category_taxonomy_binding::Column::TaxonomyCategoryId.eq(category_id))
        .one(txn)
        .await?
    {
        return Err(BlogError::Validation(format!(
            "Taxonomy Category {category_id} is already bound to Blog category {}",
            existing.blog_category_id
        )));
    }

    blog_category_taxonomy_binding::ActiveModel {
        tenant_id: Set(tenant_id),
        blog_category_id: Set(category_id),
        taxonomy_category_id: Set(category_id),
        created_at: Set(Utc::now().into()),
    }
    .insert(txn)
    .await?;
    Ok(())
}

fn canonical_key_for_blog_category(category_id: Uuid) -> String {
    format!("blog-category-{category_id}")
}

fn map_taxonomy_error(error: rustok_taxonomy::TaxonomyError) -> BlogError {
    match error {
        rustok_taxonomy::TaxonomyError::Database(error) => BlogError::Database(error),
        other => BlogError::Validation(format!(
            "Blog Category Taxonomy synchronization failed: {other}"
        )),
    }
}
