use chrono::{DateTime, FixedOffset};
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement, TransactionTrait};
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

#[derive(Debug, FromQueryResult)]
struct CountRow {
    count: i64,
}

#[derive(Debug, FromQueryResult)]
struct TaxonomyTermRow {
    id: Uuid,
    tenant_id: Uuid,
    kind: String,
    scope_type: String,
    scope_value: String,
    canonical_key: String,
}

#[derive(Debug, FromQueryResult)]
struct TaxonomyTranslationRow {
    name: String,
    slug: String,
    description: Option<String>,
}

#[derive(Debug, FromQueryResult)]
struct TaxonomyTermOwnerRow {
    term_id: Uuid,
}

#[derive(Debug, FromQueryResult)]
struct HierarchyRow {
    parent_term_id: Option<Uuid>,
    position: i32,
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

        let has_taxonomy_terms = CountRow::find_by_statement(Statement::from_string(
            DatabaseBackend::Postgres,
            r#"
                SELECT COUNT(*)::BIGINT AS count
                FROM information_schema.tables
                WHERE table_schema = current_schema()
                  AND table_name = 'taxonomy_terms'
            "#,
        ))
        .one(&txn)
        .await?
        .map(|row| row.count)
        .unwrap_or_default()
            != 0;

        if has_taxonomy_terms {
            backfill_product_categories(&txn).await?;
        }

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

    let existing_by_id = TaxonomyTermRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT id, tenant_id, kind, scope_type, scope_value, canonical_key FROM taxonomy_terms WHERE id = $1",
        vec![category.id.into()],
    ))
    .one(txn)
    .await?;

    if let Some(existing) = existing_by_id {
        if existing.tenant_id != category.tenant_id
            || existing.kind != "category"
            || existing.scope_type != "module"
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

    let existing_by_key = TaxonomyTermRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            SELECT id, tenant_id, kind, scope_type, scope_value, canonical_key
            FROM taxonomy_terms
            WHERE tenant_id = $1
              AND kind = 'category'
              AND scope_type = 'module'
              AND scope_value = $2
              AND canonical_key = $3
            LIMIT 1
        "#,
        vec![
            category.tenant_id.into(),
            PRODUCT_SCOPE_VALUE.into(),
            canonical_key.clone().into(),
        ],
    ))
    .one(txn)
    .await?;

    if let Some(existing) = existing_by_key {
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: canonical key {canonical_key:?} is already owned by Taxonomy Category {}",
            existing.id,
        )));
    }

    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO taxonomy_terms (
                id, tenant_id, kind, scope_type, scope_value, canonical_key, revision, created_at, updated_at
            ) VALUES ($1, $2, 'category', 'module', $3, $4, 1, $5, $6)
        "#,
        vec![
            category.id.into(),
            category.tenant_id.into(),
            PRODUCT_SCOPE_VALUE.into(),
            canonical_key.into(),
            category.created_at.into(),
            category.updated_at.into(),
        ],
    ))
    .await?;

    Ok(())
}

async fn ensure_category_translations_and_routes(
    txn: &sea_orm::DatabaseTransaction,
    category: &ProductCategoryRow,
) -> Result<(), DbErr> {
    let route_key = exact_taxonomy_route_key(&category.slug, category.id)?;
    let translations =
        ProductCategoryTranslationRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
                SELECT id, locale, name, description
                FROM catalog_category_translations
                WHERE category_id = $1
                ORDER BY locale, id
            "#,
            vec![category.id.into()],
        ))
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

        let existing = TaxonomyTranslationRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
                SELECT name, slug, description
                FROM taxonomy_term_translations
                WHERE tenant_id = $1 AND term_id = $2 AND locale = $3
            "#,
            vec![
                category.tenant_id.into(),
                category.id.into(),
                locale.clone().into(),
            ],
        ))
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
                let id_owner =
                    TaxonomyTermOwnerRow::find_by_statement(Statement::from_sql_and_values(
                        DatabaseBackend::Postgres,
                        "SELECT term_id FROM taxonomy_term_translations WHERE id = $1",
                        vec![translation.id.into()],
                    ))
                    .one(txn)
                    .await?;

                if let Some(id_owner) = id_owner {
                    return Err(DbErr::Migration(format!(
                        "Product Category Taxonomy backfill blocked: translation UUID {} is already used by Taxonomy term {}",
                        translation.id, id_owner.term_id,
                    )));
                }

                txn.execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    r#"
                        INSERT INTO taxonomy_term_translations (
                            id, term_id, tenant_id, locale, name, slug, description, revision, created_at, updated_at
                        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 1, $8, $9)
                    "#,
                    vec![
                        translation.id.into(),
                        category.id.into(),
                        category.tenant_id.into(),
                        locale.clone().into(),
                        translation.name.clone().into(),
                        route_key.clone().into(),
                        translation.description.clone().into(),
                        category.created_at.into(),
                        category.updated_at.into(),
                    ],
                ))
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
    let existing = TaxonomyTermOwnerRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            SELECT term_id
            FROM taxonomy_term_route_keys
            WHERE tenant_id = $1
              AND kind = 'category'
              AND scope_type = 'module'
              AND scope_value = $2
              AND locale = $3
              AND route_key = $4
            LIMIT 1
        "#,
        vec![
            tenant_id.into(),
            PRODUCT_SCOPE_VALUE.into(),
            locale.into(),
            route_key.into(),
        ],
    ))
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

    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO taxonomy_term_route_keys (
                tenant_id, kind, scope_type, scope_value, locale, route_key, term_id
            ) VALUES ($1, 'category', 'module', $2, $3, $4, $5)
        "#,
        vec![
            tenant_id.into(),
            PRODUCT_SCOPE_VALUE.into(),
            locale.into(),
            route_key.into(),
            term_id.into(),
        ],
    ))
    .await?;

    Ok(())
}

async fn ensure_translation_change(
    txn: &sea_orm::DatabaseTransaction,
    category: &ProductCategoryRow,
    locale: &str,
) -> Result<(), DbErr> {
    let exists = CountRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            SELECT COUNT(*)::BIGINT AS count
            FROM translation_changes
            WHERE tenant_id = $1
              AND term_id = $2
              AND locale = $3
              AND operation = 'upsert'
              AND lifecycle = 'active'
        "#,
        vec![category.tenant_id.into(), category.id.into(), locale.into()],
    ))
    .one(txn)
    .await?
    .map(|row| row.count)
    .unwrap_or_default()
        != 0;

    if exists {
        return Ok(());
    }

    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO translation_changes (
                id, tenant_id, term_id, locale, resource_revision, target_revision, operation, lifecycle, created_at
            ) VALUES ($1, $2, $3, $4, 1, 1, 'upsert', 'active', $5)
        "#,
        vec![
            Uuid::new_v4().into(),
            category.tenant_id.into(),
            category.id.into(),
            locale.into(),
            category.updated_at.into(),
        ],
    ))
    .await?;

    Ok(())
}

async fn ensure_category_hierarchy(
    txn: &sea_orm::DatabaseTransaction,
    category: &ProductCategoryRow,
) -> Result<(), DbErr> {
    let existing = HierarchyRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            SELECT parent_term_id, position
            FROM taxonomy_category_hierarchy
            WHERE tenant_id = $1 AND term_id = $2
        "#,
        vec![category.tenant_id.into(), category.id.into()],
    ))
    .one(txn)
    .await?;

    match existing {
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
            txn.execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                    INSERT INTO taxonomy_category_hierarchy (
                        tenant_id, term_id, parent_term_id, position
                    ) VALUES ($1, $2, $3, $4)
                "#,
                vec![
                    category.tenant_id.into(),
                    category.id.into(),
                    category.parent_id.into(),
                    category.position.into(),
                ],
            ))
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

    txn.execute_raw(Statement::from_sql_and_values(
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
    let normalized = rustok_api::normalize_locale_tag(value).ok_or_else(|| {
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
    let key = crate::services::catalog::helpers::slugify(value);
    let normalized = if key.is_empty() {
        return Err(DbErr::Migration(format!(
            "Product Category Taxonomy backfill blocked: category {category_id} has an empty route key",
        )));
    } else {
        key
    };
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
