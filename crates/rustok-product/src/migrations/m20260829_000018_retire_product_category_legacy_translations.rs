use std::collections::HashMap;

use rustok_taxonomy::{TaxonomyScopeType, TaxonomyTermKind, entities::taxonomy_term};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseTransaction, EntityTrait,
    FromQueryResult, QueryFilter, Statement, TransactionTrait,
};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

const PRODUCT_SCOPE_VALUE: &str = "product";

#[derive(Debug, FromQueryResult)]
struct ProductCategoryOwnershipRow {
    id: Uuid,
    tenant_id: Uuid,
    taxonomy_category_id: Option<Uuid>,
}

#[derive(Debug, FromQueryResult)]
struct CountRow {
    count: i64,
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
        ensure_complete_taxonomy_ownership(&txn).await?;
        ensure_complete_taxonomy_locale_ownership(&txn).await?;
        ensure_complete_product_seo_ownership(&txn).await?;
        txn.execute(Statement::from_string(
            DatabaseBackend::Postgres,
            "DROP TABLE IF EXISTS catalog_category_translations".to_owned(),
        ))
        .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Intentionally irreversible on PostgreSQL. Canonical Category copy is
        // Taxonomy-owned and Product-only localized SEO has already moved to
        // catalog_category_seo_translations. Recreating an empty donor table
        // would falsely imply that the retired localized copy can be restored.
        Ok(())
    }
}

async fn ensure_complete_taxonomy_ownership(txn: &DatabaseTransaction) -> Result<(), DbErr> {
    let categories = ProductCategoryOwnershipRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        r#"
            SELECT
                category.id,
                category.tenant_id,
                binding.taxonomy_category_id
            FROM catalog_categories category
            LEFT JOIN product_catalog_category_taxonomy_bindings binding
              ON binding.tenant_id = category.tenant_id
             AND binding.catalog_category_id = category.id
            ORDER BY category.tenant_id, category.id
        "#,
    ))
    .all(txn)
    .await?;

    if categories.is_empty() {
        return Ok(());
    }

    let taxonomy_ids = categories
        .iter()
        .map(|category| {
            let taxonomy_id = category.taxonomy_category_id.ok_or_else(|| {
                DbErr::Migration(format!(
                    "Product Category legacy-storage retirement blocked: category {} in tenant {} has no Taxonomy binding",
                    category.id, category.tenant_id,
                ))
            })?;
            if taxonomy_id != category.id {
                return Err(DbErr::Migration(format!(
                    "Product Category legacy-storage retirement blocked: category {} is bound to non-canonical Taxonomy UUID {taxonomy_id}",
                    category.id,
                )));
            }
            Ok(taxonomy_id)
        })
        .collect::<Result<Vec<_>, DbErr>>()?;

    let terms = taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::Id.is_in(taxonomy_ids))
        .all(txn)
        .await?;
    let term_by_id = terms
        .into_iter()
        .map(|term| (term.id, term))
        .collect::<HashMap<_, _>>();

    for category in categories {
        let taxonomy_id = category
            .taxonomy_category_id
            .expect("binding checked above");
        let term = term_by_id.get(&taxonomy_id).ok_or_else(|| {
            DbErr::Migration(format!(
                "Product Category legacy-storage retirement blocked: Taxonomy Category {taxonomy_id} is missing for Product category {}",
                category.id,
            ))
        })?;
        let expected_key = format!("product-category-{}", category.id);
        if term.tenant_id != category.tenant_id
            || term.kind != TaxonomyTermKind::Category
            || term.scope_type != TaxonomyScopeType::Module
            || term.scope_value != PRODUCT_SCOPE_VALUE
            || term.canonical_key != expected_key
        {
            return Err(DbErr::Migration(format!(
                "Product Category legacy-storage retirement blocked: Taxonomy term {taxonomy_id} has incompatible tenant/kind/scope/canonical-key ownership",
            )));
        }
    }

    Ok(())
}

async fn ensure_complete_taxonomy_locale_ownership(txn: &DatabaseTransaction) -> Result<(), DbErr> {
    let missing = CountRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        r#"
            SELECT COUNT(*)::BIGINT AS count
            FROM catalog_category_translations legacy
            JOIN catalog_categories category
              ON category.id = legacy.category_id
            LEFT JOIN taxonomy_term_translations taxonomy_copy
              ON taxonomy_copy.tenant_id = category.tenant_id
             AND taxonomy_copy.term_id = category.id
             AND taxonomy_copy.locale = legacy.locale
            WHERE taxonomy_copy.term_id IS NULL
        "#,
    ))
    .one(txn)
    .await?
    .map(|row| row.count)
    .unwrap_or_default();

    if missing != 0 {
        return Err(DbErr::Migration(format!(
            "Product Category legacy-storage retirement blocked: {missing} legacy localized row(s) have no same-locale Taxonomy canonical copy",
        )));
    }

    Ok(())
}

async fn ensure_complete_product_seo_ownership(txn: &DatabaseTransaction) -> Result<(), DbErr> {
    let mismatch = CountRow::find_by_statement(Statement::from_string(
        DatabaseBackend::Postgres,
        r#"
            SELECT COUNT(*)::BIGINT AS count
            FROM catalog_category_translations legacy
            JOIN catalog_categories category
              ON category.id = legacy.category_id
            LEFT JOIN catalog_category_seo_translations seo
              ON seo.tenant_id = category.tenant_id
             AND seo.category_id = legacy.category_id
             AND seo.locale = legacy.locale
            WHERE (legacy.meta_title IS NOT NULL OR legacy.meta_description IS NOT NULL)
              AND (
                  seo.category_id IS NULL
                  OR seo.meta_title IS DISTINCT FROM legacy.meta_title
                  OR seo.meta_description IS DISTINCT FROM legacy.meta_description
              )
        "#,
    ))
    .one(txn)
    .await?
    .map(|row| row.count)
    .unwrap_or_default();

    if mismatch != 0 {
        return Err(DbErr::Migration(format!(
            "Product Category legacy-storage retirement blocked: {mismatch} localized SEO row(s) are missing or incompatible in catalog_category_seo_translations",
        )));
    }

    Ok(())
}
