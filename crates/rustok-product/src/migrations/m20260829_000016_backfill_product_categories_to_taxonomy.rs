use chrono::{DateTime, FixedOffset};
use rustok_taxonomy::{
    TaxonomyScopeType, TaxonomyTermKind,
    entities::{
        taxonomy_category_hierarchy, taxonomy_term, taxonomy_term_route_key,
        taxonomy_term_translation, translation_change,
    },
    normalize_term_locale, normalize_term_route_key,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait,
    FromQueryResult, QueryFilter, Statement, TransactionTrait,
};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

const PRODUCT_SCOPE_VALUE: &str = "product";
const TAXONOMY_ROUTE_KEY_MAX_BYTES: usize = 120;

#[derive(Debug, FromQueryResult)]
struct ProductCategoryRow {
    id: Uuid,
    tenant_id: Uuid,
    parent_id: Option<Uuid>,
    code: String,
    slug: String,
    position: i32,
    created_at: DateTime<FixedOffset>,
    updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, FromQueryResult)]
struct ProductCategoryTranslationRow {
    id: Uuid,
    locale: String,
    name: String,
    description: Option<String>,
}

#[derive(Debug, FromQueryResult)]
struct ExistingTaxonomyBindingRow {
    taxonomy_category_id: Uuid,
}

#[derive(Debug, FromQueryResult)]
struct ExistingProductBindingRow {
    catalog_category_id: Uuid,
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        let txn = manager.get_connection().begin().await?;
        backfill_product_categories(&txn).await?;
        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // This is an intentionally monotonic copy. Product category storage,
        // localized SEO metadata, hierarchy/closure and runtime reads/writes
        // remain live until a later verified cutover, so rollback must not
        // delete truthful Taxonomy data or same-ID bindings.
        Ok(())
    }
}

async fn backfill_product_categories(txn: &sea_orm::DatabaseTransaction) -> Result<(), DbErr> {
    let categories = ProductCategoryRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        r#"
            SELECT id, tenant_id, parent_id, code, slug, position, created_at, updated_at
            FROM catalog_categories
            ORDER BY tenant_id, id
        "#,
    ))
    .all(txn)
    .await?;

    for category in &categories {
        ensure_taxonomy_term(txn, category).await?;
        ensure_category_translations_and_routes(txn, category).await?;
    }

    // Every Taxonomy identity must exist before parent references are copied.
    for category in &categories {
        ensure_category_hierarchy(txn, category).await?;
    }

    // Bind only after identity/localized copy/routes/hierarchy are complete.
    for category in &categories {
        ensure_product_binding(txn, category).await?;
    }

    Ok(())
}

async fn ensure_taxonomy_term(
    txn: &sea_orm::DatabaseTransaction,
    category: &ProductCategoryRow,
) -> Result<(), DbErr> {
    let canonical_key = canonical_key_for_product_category(category.id);

    if let Some(existing) = taxonomy_term::Entity::find_by_id(category.id)
        .one(txn)
        .await?
    {
        if existing.tenant_id != category.tenant_id
            || existing.kind != TaxonomyTermKind::Category
            || existing.scope_type != TaxonomyScopeType::Module
            || existing.scope_value != PRODUCT_SCOPE_VALUE
            || existing.canonical_key != canonical_key
        {
            return Err(DbErr::Migration(format!(
                "Product Category Taxonomy backfill blocked: UUID {} is already owned by an incompatible Taxonomy term",
                category.id,
            )));
        }
        return Ok(());
    }

    if let Some(existing) = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(category.tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Category))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(PRODUCT_SCOPE_VALUE))
        .filter(taxonomy_term::Column::CanonicalKey.eq(&canonical_key))
        .one(txn)
        .await?
    {
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: canonical key {canonical_key:?} is already owned by Taxonomy Category {}",
            existing.id,
        )));
    }

    taxonomy_term::ActiveModel {
        id: Set(category.id),
        tenant_id: Set(category.tenant_id),
        kind: Set(TaxonomyTermKind::Category),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set(PRODUCT_SCOPE_VALUE.to_owned()),
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
    category: &ProductCategoryRow,
) -> Result<(), DbErr> {
    let route_key = exact_taxonomy_route_key(&category.slug, category.id)?;
    let translations = ProductCategoryTranslationRow::find_by_statement(
        Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
                SELECT id, locale, name, description
                FROM catalog_category_translations
                WHERE category_id = $1
                ORDER BY locale, id
            "#,
            vec![category.id.into()],
        ),
    )
    .all(txn)
    .await?;

    if translations.is_empty() {
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: category {} ({}) has no localized copy",
            category.id, category.code,
        )));
    }

    for translation in translations {
        let locale = exact_taxonomy_locale(&translation.locale, category.id)?;

        let existing = taxonomy_term_translation::Entity::find()
            .filter(taxonomy_term_translation::Column::TenantId.eq(category.tenant_id))
            .filter(taxonomy_term_translation::Column::TermId.eq(category.id))
            .filter(taxonomy_term_translation::Column::Locale.eq(&locale))
            .one(txn)
            .await?;
        match existing {
            Some(existing)
                if existing.name == translation.name
                    && existing.slug == route_key
                    && existing.description == translation.description => {}
            Some(_) => {
                return Err(DbErr::Migration(format!(
                    "Product Category Taxonomy backfill blocked: Taxonomy localized copy already differs for category {} locale {locale}",
                    category.id,
                )));
            }
            None => {
                if let Some(id_owner) = taxonomy_term_translation::Entity::find_by_id(translation.id)
                    .one(txn)
                    .await?
                {
                    return Err(DbErr::Migration(format!(
                        "Product Category Taxonomy backfill blocked: translation UUID {} is already used by Taxonomy term {}",
                        translation.id, id_owner.term_id,
                    )));
                }

                taxonomy_term_translation::ActiveModel {
                    id: Set(translation.id),
                    term_id: Set(category.id),
                    tenant_id: Set(category.tenant_id),
                    locale: Set(locale.clone()),
                    name: Set(translation.name.clone()),
                    // Product owns one base category slug, not locale-specific slugs.
                    // Preserve it deterministically for every imported locale.
                    slug: Set(route_key.clone()),
                    description: Set(translation.description.clone()),
                    revision: Set(1),
                    created_at: Set(category.created_at),
                    updated_at: Set(category.updated_at),
                }
                .insert(txn)
                .await?;
            }
        }

        ensure_route_owner(txn, category.tenant_id, category.id, &locale, &route_key).await?;
        ensure_translation_change(txn, category, &locale).await?;
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
        .filter(taxonomy_term_route_key::Column::ScopeValue.eq(PRODUCT_SCOPE_VALUE))
        .filter(taxonomy_term_route_key::Column::Locale.eq(locale))
        .filter(taxonomy_term_route_key::Column::RouteKey.eq(route_key))
        .one(txn)
        .await?;

    if let Some(existing) = existing {
        if existing.term_id == term_id {
            return Ok(());
        }
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: route {locale}/{route_key} is already owned by Taxonomy Category {}",
            existing.term_id,
        )));
    }

    taxonomy_term_route_key::ActiveModel {
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Category),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set(PRODUCT_SCOPE_VALUE.to_owned()),
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
    category: &ProductCategoryRow,
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

async fn ensure_category_hierarchy(
    txn: &sea_orm::DatabaseTransaction,
    category: &ProductCategoryRow,
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
            "Product Category Taxonomy backfill blocked: hierarchy already differs for category {}",
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

async fn ensure_product_binding(
    txn: &sea_orm::DatabaseTransaction,
    category: &ProductCategoryRow,
) -> Result<(), DbErr> {
    let existing = ExistingTaxonomyBindingRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            SELECT taxonomy_category_id
            FROM product_catalog_category_taxonomy_bindings
            WHERE tenant_id = $1 AND catalog_category_id = $2
        "#,
        vec![category.tenant_id.into(), category.id.into()],
    ))
    .one(txn)
    .await?;

    if let Some(existing) = existing {
        if existing.taxonomy_category_id == category.id {
            return Ok(());
        }
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: Product category {} is already bound to Taxonomy Category {}",
            category.id, existing.taxonomy_category_id,
        )));
    }

    let reverse = ExistingProductBindingRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            SELECT catalog_category_id
            FROM product_catalog_category_taxonomy_bindings
            WHERE tenant_id = $1 AND taxonomy_category_id = $2
        "#,
        vec![category.tenant_id.into(), category.id.into()],
    ))
    .one(txn)
    .await?;

    if let Some(existing) = reverse {
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: Taxonomy Category {} is already bound to Product category {}",
            category.id, existing.catalog_category_id,
        )));
    }

    txn.execute(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO product_catalog_category_taxonomy_bindings (
                tenant_id, catalog_category_id, taxonomy_category_id, created_at
            ) VALUES ($1, $2, $2, $3)
        "#,
        vec![
            category.tenant_id.into(),
            category.id.into(),
            category.created_at.into(),
        ],
    ))
    .await?;

    Ok(())
}

fn canonical_key_for_product_category(category_id: Uuid) -> String {
    format!("product-category-{category_id}")
}

fn exact_taxonomy_locale(value: &str, category_id: Uuid) -> Result<String, DbErr> {
    let normalized = normalize_term_locale(value).ok_or_else(|| {
        DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: category {category_id} has invalid locale {value:?}",
        ))
    })?;
    if normalized != value {
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: category {category_id} locale {value:?} is not canonically normalized as {normalized:?}",
        )));
    }
    Ok(normalized)
}

fn exact_taxonomy_route_key(value: &str, category_id: Uuid) -> Result<String, DbErr> {
    let normalized = normalize_term_route_key(value).ok_or_else(|| {
        DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: category {category_id} has an empty route key",
        ))
    })?;
    if normalized != value {
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: category {category_id} route {value:?} normalizes to {normalized:?}",
        )));
    }
    if normalized.len() > TAXONOMY_ROUTE_KEY_MAX_BYTES {
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: category {category_id} route exceeds {TAXONOMY_ROUTE_KEY_MAX_BYTES} bytes",
        )));
    }
    Ok(normalized)
}
