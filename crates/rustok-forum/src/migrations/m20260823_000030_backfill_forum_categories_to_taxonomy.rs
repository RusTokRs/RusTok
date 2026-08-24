use rustok_taxonomy::{
    TaxonomyScopeType, TaxonomyTermKind,
    entities::{
        taxonomy_category_hierarchy, taxonomy_category_presentation, taxonomy_term,
        taxonomy_term_alias, taxonomy_term_route_key, taxonomy_term_translation,
        translation_change,
    },
    normalize_taxonomy_category_color, normalize_taxonomy_category_icon_key, normalize_term_locale,
    normalize_term_route_key,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseBackend, EntityTrait,
    QueryFilter, QueryOrder, TransactionTrait,
};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

use crate::entities::{
    forum_category, forum_category_taxonomy_binding, forum_category_translation,
};

const FORUM_SCOPE_VALUE: &str = "forum";
const TAXONOMY_ROUTE_KEY_MAX_BYTES: usize = 120;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {}
            backend => {
                return Err(DbErr::Custom(format!(
                    "Forum Category Taxonomy backfill does not support {backend:?}",
                )));
            }
        }

        let txn = manager.get_connection().begin().await?;
        backfill_forum_categories(&txn).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // The backfill is intentionally monotonic. Legacy Forum category rows,
        // translations, hierarchy and route aliases remain authoritative until
        // a later read/write cutover, so rolling code back does not require or
        // justify deleting the copied Taxonomy Category data or bindings.
        Ok(())
    }
}

async fn backfill_forum_categories(txn: &sea_orm::DatabaseTransaction) -> Result<(), DbErr> {
    let categories = forum_category::Entity::find()
        .order_by_asc(forum_category::Column::TenantId)
        .order_by_asc(forum_category::Column::Id)
        .all(txn)
        .await?;

    for category in &categories {
        ensure_taxonomy_term(txn, category).await?;
        ensure_category_translations_and_routes(txn, category).await?;
        ensure_category_presentation(txn, category).await?;
    }

    // Every Category identity must exist before hierarchy rows are copied so a
    // child never references a parent that has not been imported yet.
    for category in &categories {
        ensure_category_hierarchy(txn, category).await?;
    }

    for category in &categories {
        ensure_forum_binding(txn, category).await?;
    }

    Ok(())
}

async fn ensure_taxonomy_term(
    txn: &sea_orm::DatabaseTransaction,
    category: &forum_category::Model,
) -> Result<(), DbErr> {
    let canonical_key = canonical_key_for_forum_category(category.id);

    if let Some(existing) = taxonomy_term::Entity::find_by_id(category.id)
        .one(txn)
        .await?
    {
        if existing.tenant_id != category.tenant_id
            || existing.kind != TaxonomyTermKind::Category
            || existing.scope_type != TaxonomyScopeType::Module
            || existing.scope_value != FORUM_SCOPE_VALUE
            || existing.canonical_key != canonical_key
        {
            return Err(DbErr::Migration(format!(
                "Forum Category Taxonomy backfill blocked: UUID {} is already owned by an incompatible Taxonomy term",
                category.id,
            )));
        }
        return Ok(());
    }

    if let Some(existing) = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(category.tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(FORUM_SCOPE_VALUE))
        .filter(taxonomy_term::Column::CanonicalKey.eq(&canonical_key))
        .one(txn)
        .await?
    {
        return Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: canonical key {canonical_key:?} is already owned by Taxonomy Category {}",
            existing.id,
        )));
    }

    taxonomy_term::ActiveModel {
        id: Set(category.id),
        tenant_id: Set(category.tenant_id),
        kind: Set(TaxonomyTermKind::Category),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set(FORUM_SCOPE_VALUE.to_owned()),
        canonical_key: Set(canonical_key),
        revision: Set(1),
        created_at: Set(category.created_at),
        updated_at: Set(category.updated_at),
    }
    .insert(txn)
    .await?;

    Ok(())
}

async fn ensure_category_translations_and_routes(
    txn: &sea_orm::DatabaseTransaction,
    category: &forum_category::Model,
) -> Result<(), DbErr> {
    let translations = forum_category_translation::Entity::find()
        .filter(forum_category_translation::Column::TenantId.eq(category.tenant_id))
        .filter(forum_category_translation::Column::CategoryId.eq(category.id))
        .order_by_asc(forum_category_translation::Column::Locale)
        .order_by_asc(forum_category_translation::Column::Id)
        .all(txn)
        .await?;
    if translations.is_empty() {
        return Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: category {} has no localized copy",
            category.id,
        )));
    }

    for translation in translations {
        let locale = exact_taxonomy_locale(&translation.locale, category.id)?;
        let slug = exact_taxonomy_route_key(&translation.slug, category.id, &locale)?;

        let existing = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TenantId.eq(category.tenant_id))
            .filter(taxonomy_term_translation::Column::TermId.eq(category.id))
            .filter(taxonomy_term_translation::Column::Locale.eq(&locale))
            .one(txn)
            .await?;
        match existing {
            Some(existing)
                if existing.name == translation.name
                    && existing.slug == slug
                    && existing.description == translation.description => {}
            Some(_) => {
                return Err(DbErr::Migration(format!(
                    "Forum Category Taxonomy backfill blocked: Taxonomy localized copy already differs for category {} locale {locale}",
                    category.id,
                )));
            }
            None => {
                if let Some(id_owner) =
                    taxonomy_term_translation::Entity::find_by_id(translation.id)
                        .one(txn)
                        .await?
                {
                    return Err(DbErr::Migration(format!(
                        "Forum Category Taxonomy backfill blocked: translation UUID {} is already used by Taxonomy term {}",
                        translation.id, id_owner.term_id,
                    )));
                }
                let now = category.updated_at;
                taxonomy_term_translation::ActiveModel {
                    id: Set(translation.id),
                    term_id: Set(category.id),
                    tenant_id: Set(category.tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(translation.name.clone()),
                    slug: Set(slug.clone()),
                    description: Set(translation.description.clone()),
                    revision: Set(1),
                    created_at: Set(category.created_at),
                    updated_at: Set(now),
                }
                .insert(txn)
                .await?;
            }
        }

        ensure_route_owner(txn, category.tenant_id, category.id, &locale, &slug).await?;
        ensure_translation_change(txn, category, &locale).await?;
    }

    let aliases = legacy_category_route_alias::Entity::find()
        .filter(legacy_category_route_alias::Column::TenantId.eq(category.tenant_id))
        .filter(legacy_category_route_alias::Column::CategoryId.eq(category.id))
        .order_by_asc(legacy_category_route_alias::Column::Locale)
        .order_by_asc(legacy_category_route_alias::Column::CreatedAt)
        .order_by_asc(legacy_category_route_alias::Column::AliasId)
        .all(txn)
        .await?;

    for alias in aliases {
        let locale = exact_taxonomy_locale(&alias.locale, category.id)?;
        let slug = exact_taxonomy_route_key(&alias.slug, category.id, &locale)?;

        match taxonomy_term_alias::Entity::find_by_id(alias.alias_id)
            .one(txn)
            .await?
        {
            Some(existing)
                if existing.term_id == category.id
                    && existing.tenant_id == category.tenant_id
                    && existing.locale == locale
                    && existing.slug == slug => {}
            Some(existing) => {
                return Err(DbErr::Migration(format!(
                    "Forum Category Taxonomy backfill blocked: alias UUID {} is already used by Taxonomy term {}",
                    alias.alias_id, existing.term_id,
                )));
            }
            None => {
                taxonomy_term_alias::ActiveModel {
                    id: Set(alias.alias_id),
                    term_id: Set(category.id),
                    tenant_id: Set(category.tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(slug.clone()),
                    slug: Set(slug.clone()),
                    created_at: Set(alias.created_at),
                }
                .insert(txn)
                .await?;
            }
        }

        ensure_route_owner(txn, category.tenant_id, category.id, &locale, &slug).await?;
    }

    Ok(())
}

async fn ensure_route_owner(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    term_id: Uuid,
    locale: &str,
    route_key: &str,
) -> Result<(), DbErr> {
    let existing = taxonomy_term_route_key::Entity::find()
        .filter(taxonomy_term_route_key::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_route_key::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term_route_key::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term_route_key::Column::ScopeValue.eq(FORUM_SCOPE_VALUE))
        .filter(taxonomy_term_route_key::Column::Locale.eq(locale))
        .filter(taxonomy_term_route_key::Column::RouteKey.eq(route_key))
        .one(txn)
        .await?;
    if let Some(existing) = existing {
        if existing.term_id == term_id {
            return Ok(());
        }
        return Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: route {locale}/{route_key} is already owned by Taxonomy Category {}",
            existing.term_id,
        )));
    }

    taxonomy_term_route_key::ActiveModel {
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Category),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set(FORUM_SCOPE_VALUE.to_owned()),
        locale: Set(locale.to_owned()),
        route_key: Set(route_key.to_owned()),
        term_id: Set(term_id),
    }
    .insert(txn)
    .await?;
    Ok(())
}

async fn ensure_translation_change(
    txn: &sea_orm::DatabaseTransaction,
    category: &forum_category::Model,
    locale: &str,
) -> Result<(), DbErr> {
    let exists = translation_change::Entity::find()
        .filter(translation_change::Column::TenantId.eq(category.tenant_id))
        .filter(translation_change::Column::TermId.eq(category.id))
        .filter(translation_change::Column::Locale.eq(locale))
        .filter(translation_change::Column::Operation.eq("upsert"))
        .filter(translation_change::Column::Lifecycle.eq("active"))
        .one(txn)
        .await?
        .is_some();
    if exists {
        return Ok(());
    }

    translation_change::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(category.tenant_id),
        term_id: Set(category.id),
        locale: Set(locale.to_owned()),
        resource_revision: Set(1),
        target_revision: Set(1),
        operation: Set("upsert".to_owned()),
        lifecycle: Set("active".to_owned()),
        created_at: Set(category.updated_at),
    }
    .insert(txn)
    .await?;
    Ok(())
}

async fn ensure_category_presentation(
    txn: &sea_orm::DatabaseTransaction,
    category: &forum_category::Model,
) -> Result<(), DbErr> {
    let icon_key = category
        .icon
        .as_deref()
        .map(normalize_taxonomy_category_icon_key)
        .transpose()
        .map_err(|error| DbErr::Migration(error.to_string()))?
        .flatten();
    let color = category
        .color
        .as_deref()
        .map(normalize_taxonomy_category_color)
        .transpose()
        .map_err(|error| DbErr::Migration(error.to_string()))?
        .flatten();

    match taxonomy_category_presentation::Entity::find_by_id((category.tenant_id, category.id))
        .one(txn)
        .await?
    {
        Some(existing)
            if existing.icon_key == icon_key
                && existing.color == color
                && existing.image_media_id.is_none()
                && existing.cover_media_id.is_none() =>
        {
            Ok(())
        }
        Some(_) => Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: canonical presentation already differs for category {}",
            category.id,
        ))),
        None if icon_key.is_none() && color.is_none() => Ok(()),
        None => {
            taxonomy_category_presentation::ActiveModel {
                tenant_id: Set(category.tenant_id),
                term_id: Set(category.id),
                icon_key: Set(icon_key),
                color: Set(color),
                image_media_id: Set(None),
                cover_media_id: Set(None),
                revision: Set(1),
                created_at: Set(category.created_at),
                updated_at: Set(category.updated_at),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
    }
}

async fn ensure_category_hierarchy(
    txn: &sea_orm::DatabaseTransaction,
    category: &forum_category::Model,
) -> Result<(), DbErr> {
    match taxonomy_category_hierarchy::Entity::find_by_id((category.tenant_id, category.id))
        .one(txn)
        .await?
    {
        Some(existing)
            if existing.parent_term_id == category.parent_id
                && existing.position == category.position =>
        {
            Ok(())
        }
        Some(_) => Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: hierarchy already differs for category {}",
            category.id,
        ))),
        None => {
            taxonomy_category_hierarchy::ActiveModel {
                tenant_id: Set(category.tenant_id),
                term_id: Set(category.id),
                parent_term_id: Set(category.parent_id),
                position: Set(category.position),
            }
            .insert(txn)
            .await?;
            Ok(())
        }
    }
}

async fn ensure_forum_binding(
    txn: &sea_orm::DatabaseTransaction,
    category: &forum_category::Model,
) -> Result<(), DbErr> {
    if let Some(existing) =
        forum_category_taxonomy_binding::Entity::find_by_id((category.tenant_id, category.id))
            .one(txn)
            .await?
    {
        if existing.taxonomy_category_id == category.id {
            return Ok(());
        }
        return Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: Forum category {} is already bound to Taxonomy Category {}",
            category.id, existing.taxonomy_category_id,
        )));
    }

    if let Some(existing) = forum_category_taxonomy_binding::Entity::find()
        .filter(forum_category_taxonomy_binding::Column::TenantId.eq(category.tenant_id))
        .filter(forum_category_taxonomy_binding::Column::TaxonomyCategoryId.eq(category.id))
        .one(txn)
        .await?
    {
        return Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: Taxonomy Category {} is already bound to Forum category {}",
            category.id, existing.forum_category_id,
        )));
    }

    forum_category_taxonomy_binding::ActiveModel {
        tenant_id: Set(category.tenant_id),
        forum_category_id: Set(category.id),
        taxonomy_category_id: Set(category.id),
        created_at: Set(category.created_at),
    }
    .insert(txn)
    .await?;
    Ok(())
}

fn canonical_key_for_forum_category(category_id: Uuid) -> String {
    format!("forum-category-{category_id}")
}

fn exact_taxonomy_locale(value: &str, category_id: Uuid) -> Result<String, DbErr> {
    let normalized = normalize_term_locale(value).ok_or_else(|| {
        DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: category {category_id} has invalid locale {value:?}",
        ))
    })?;
    if normalized != value {
        return Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: category {category_id} locale {value:?} is not canonically normalized as {normalized:?}",
        )));
    }
    Ok(normalized)
}

fn exact_taxonomy_route_key(value: &str, category_id: Uuid, locale: &str) -> Result<String, DbErr> {
    let normalized = normalize_term_route_key(value).ok_or_else(|| {
        DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: category {category_id} locale {locale} has an empty route key",
        ))
    })?;
    if normalized != value {
        return Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: category {category_id} locale {locale} route {value:?} normalizes to {normalized:?}",
        )));
    }
    if normalized.len() > TAXONOMY_ROUTE_KEY_MAX_BYTES {
        return Err(DbErr::Migration(format!(
            "Forum Category Taxonomy backfill blocked: category {category_id} locale {locale} route exceeds {TAXONOMY_ROUTE_KEY_MAX_BYTES} bytes",
        )));
    }
    Ok(normalized)
}

mod legacy_category_route_alias {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "forum_category_route_aliases")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub tenant_id: Uuid,
        #[sea_orm(primary_key, auto_increment = false)]
        pub alias_id: Uuid,
        pub category_id: Uuid,
        pub locale: String,
        pub slug: String,
        pub reason: String,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_is_stable_and_locale_independent() {
        let id = Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").expect("fixture UUID");
        assert_eq!(
            canonical_key_for_forum_category(id),
            "forum-category-aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
        );
    }

    #[test]
    fn route_backfill_requires_exact_taxonomy_normalization() {
        let id = Uuid::new_v4();
        assert_eq!(
            exact_taxonomy_route_key("general-support", id, "en")
                .expect("canonical route should pass"),
            "general-support"
        );
        assert!(exact_taxonomy_route_key("General Support", id, "en").is_err());
        assert!(exact_taxonomy_route_key(&"a".repeat(121), id, "en").is_err());
    }
}
