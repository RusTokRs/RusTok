use std::collections::BTreeSet;

use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_taxonomy::{SyncModuleCategoryInput, sync_module_category_in_tx};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseTransaction, EntityTrait, QueryFilter,
    QueryOrder, QueryResult, Statement,
};
use uuid::Uuid;

use crate::{
    entities::{
        forum_category, forum_category_taxonomy_binding, forum_category_translation,
    },
    error::{ForumError, ForumResult},
};

const FORUM_TAXONOMY_SCOPE: &str = "forum";

pub(crate) async fn sync_category_locale_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> ForumResult<()> {
    let category = load_category_in_tx(txn, tenant_id, category_id).await?;
    let translation = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(category_id))
        .filter(forum_category_translation::Column::Locale.eq(locale))
        .one(txn)
        .await?
        .ok_or_else(|| {
            ForumError::Validation(format!(
                "Forum category {category_id} has no localized translation for {locale}"
            ))
        })?;
    sync_snapshot_in_tx(txn, tenant_id, category, translation).await
}

pub(crate) async fn sync_category_placement_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<()> {
    let category = load_category_in_tx(txn, tenant_id, category_id).await?;
    let translations = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(category_id))
        .order_by_asc(forum_category_translation::Column::Locale)
        .order_by_asc(forum_category_translation::Column::Id)
        .all(txn)
        .await?;
    let translation = translations
        .iter()
        .find(|translation| translation.locale == PLATFORM_FALLBACK_LOCALE)
        .or_else(|| translations.first())
        .cloned()
        .ok_or_else(|| {
            ForumError::Validation(format!(
                "Forum category {category_id} has no localized translation"
            ))
        })?;
    sync_snapshot_in_tx(txn, tenant_id, category, translation).await
}

pub(crate) async fn sync_category_placements_in_tx<I>(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_ids: I,
) -> ForumResult<()>
where
    I: IntoIterator<Item = Uuid>,
{
    let category_ids = category_ids.into_iter().collect::<BTreeSet<_>>();
    for category_id in category_ids {
        sync_category_placement_in_tx(txn, tenant_id, category_id).await?;
    }
    Ok(())
}

pub(crate) async fn sync_sibling_range_from_position_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    position: i32,
) -> ForumResult<()> {
    let categories = match parent_id {
        Some(parent_id) => {
            forum_category::Entity::find()
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .filter(forum_category::Column::ParentId.eq(parent_id))
                .filter(forum_category::Column::Position.gte(position))
                .order_by_asc(forum_category::Column::Position)
                .order_by_asc(forum_category::Column::Id)
                .all(txn)
                .await?
        }
        None => {
            forum_category::Entity::find()
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .filter(forum_category::Column::ParentId.is_null())
                .filter(forum_category::Column::Position.gte(position))
                .order_by_asc(forum_category::Column::Position)
                .order_by_asc(forum_category::Column::Id)
                .all(txn)
                .await?
        }
    };
    sync_category_placements_in_tx(txn, tenant_id, categories.into_iter().map(|row| row.id)).await
}

async fn sync_snapshot_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category: forum_category::Model,
    translation: forum_category_translation::Model,
) -> ForumResult<()> {
    if translation.category_id != category.id || translation.tenant_id != tenant_id {
        return Err(ForumError::Validation(
            "Forum category localized snapshot does not match its owner".to_string(),
        ));
    }
    let aliases = load_alias_slugs_in_tx(txn, tenant_id, category.id, &translation.locale).await?;
    let category_id = category.id;

    sync_module_category_in_tx(
        txn,
        tenant_id,
        SyncModuleCategoryInput {
            category_id,
            module_scope: FORUM_TAXONOMY_SCOPE.to_string(),
            canonical_key: format!("forum-category-{category_id}"),
            locale: translation.locale,
            name: translation.name,
            slug: translation.slug,
            aliases,
            description: translation.description,
            parent_id: category.parent_id,
            position: category.position,
            icon_key: category.icon,
            color: category.color,
        },
    )
    .await?;

    forum_category_taxonomy_binding::bind_in_tx(txn, tenant_id, category_id, category_id).await?;
    Ok(())
}

async fn load_category_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
) -> ForumResult<forum_category::Model> {
    forum_category::Entity::find_by_id(category_id)
        .filter(forum_category::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::CategoryNotFound(category_id))
}

async fn load_alias_slugs_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    locale: &str,
) -> ForumResult<Vec<String>> {
    let statement = match txn.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT slug
            FROM forum_category_route_aliases
            WHERE tenant_id = $1 AND category_id = $2 AND locale = $3
            ORDER BY slug, alias_id
            "#,
            vec![tenant_id.into(), category_id.into(), locale.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT slug
            FROM forum_category_route_aliases
            WHERE tenant_id = ? AND category_id = ? AND locale = ?
            ORDER BY slug, alias_id
            "#,
            vec![tenant_id.into(), category_id.into(), locale.into()],
        ),
        backend => {
            return Err(ForumError::Validation(format!(
                "Forum Category Taxonomy dual-write does not support {backend:?}"
            )));
        }
    };

    txn.query_all(statement)
        .await?
        .into_iter()
        .map(alias_slug_from_row)
        .collect()
}

fn alias_slug_from_row(row: QueryResult) -> ForumResult<String> {
    row.try_get("", "slug").map_err(ForumError::from)
}
