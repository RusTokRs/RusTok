use std::collections::HashSet;

use chrono::{Duration, Utc};
use rustok_api::PLATFORM_FALLBACK_LOCALE;
use rustok_taxonomy::{
    TaxonomyScopeType, TaxonomyTermKind, normalize_term_locale, normalize_term_route_key,
    entities::{
        taxonomy_term, taxonomy_term_route_key, taxonomy_term_translation, translation_change,
    },
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionTrait, sea_query::Expr,
};
use sea_orm_migration::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::entities::{product, product_tag};

const PRODUCT_SCOPE_VALUE: &str = "product";
const BACKFILL_BATCH_SIZE: u64 = 256;
const METADATA_TAG_CONSTRAINT: &str = "ck_products_metadata_tags_absent";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "product metadata-tag canonicalization requires PostgreSQL".to_owned(),
            ));
        }

        let txn = manager.get_connection().begin().await?;
        backfill_legacy_metadata_tags(&txn).await?;
        txn.execute_unprepared(&format!(
            r#"
ALTER TABLE products
    DROP CONSTRAINT IF EXISTS {METADATA_TAG_CONSTRAINT};
ALTER TABLE products
    ADD CONSTRAINT {METADATA_TAG_CONSTRAINT}
    CHECK (NOT (metadata ? 'tags')) NOT VALID;
ALTER TABLE products
    VALIDATE CONSTRAINT {METADATA_TAG_CONSTRAINT};
"#,
        ))
        .await?;
        txn.commit().await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // The old representation is intentionally not recreated: after this
        // migration Product tag identity is owned only by product_tags + Taxonomy.
        Ok(())
    }
}

async fn backfill_legacy_metadata_tags(txn: &sea_orm::DatabaseTransaction) -> Result<(), DbErr> {
    let mut last_product_id = None;

    loop {
        let mut query = product::Entity::find()
            .order_by_asc(product::Column::Id)
            .limit(BACKFILL_BATCH_SIZE);
        if let Some(last_product_id) = last_product_id {
            query = query.filter(product::Column::Id.gt(last_product_id));
        }

        let products = query.all(txn).await?;
        if products.is_empty() {
            break;
        }
        last_product_id = products.last().map(|product| product.id);

        for product in products {
            let Some(labels) = legacy_metadata_tags(&product.metadata)? else {
                continue;
            };

            let existing_relation = product_tag::Entity::find()
                .filter(product_tag::Column::TenantId.eq(product.tenant_id))
                .filter(product_tag::Column::ProductId.eq(product.id))
                .one(txn)
                .await?;

            if existing_relation.is_none() && !labels.is_empty() {
                let locale = legacy_tag_locale(&product.metadata);
                let term_ids =
                    ensure_product_tag_terms(txn, product.tenant_id, &locale, &labels).await?;
                let created_at = Utc::now();
                for (position, term_id) in term_ids.into_iter().enumerate() {
                    product_tag::ActiveModel {
                        product_id: Set(product.id),
                        term_id: Set(term_id),
                        tenant_id: Set(product.tenant_id),
                        created_at: Set(
                            (created_at + Duration::microseconds(position as i64)).into(),
                        ),
                    }
                    .insert(txn)
                    .await?;
                }
            }

            let mut metadata = product.metadata.clone();
            if let Some(object) = metadata.as_object_mut() {
                object.remove("tags");
            }
            let updated = product::Entity::update_many()
                .col_expr(product::Column::Metadata, Expr::value(metadata))
                .filter(product::Column::Id.eq(product.id))
                .filter(product::Column::TenantId.eq(product.tenant_id))
                .exec(txn)
                .await?;
            if updated.rows_affected != 1 {
                return Err(DbErr::Migration(format!(
                    "product metadata-tag canonicalization lost product {} during backfill",
                    product.id
                )));
            }
        }
    }

    Ok(())
}

fn legacy_metadata_tags(metadata: &Value) -> Result<Option<Vec<String>>, DbErr> {
    let Some(object) = metadata.as_object() else {
        return Ok(None);
    };
    let Some(raw_tags) = object.get("tags") else {
        return Ok(None);
    };

    let mut seen = HashSet::new();
    let mut labels = Vec::new();
    if let Some(items) = raw_tags.as_array() {
        for item in items {
            let Some(label) = item.as_str().map(str::trim).filter(|label| !label.is_empty()) else {
                continue;
            };
            if label.chars().count() > 120 {
                return Err(DbErr::Migration(
                    "product metadata-tag canonicalization blocked: tag name exceeds 120 characters"
                        .to_owned(),
                ));
            }
            let dedupe_key = label.to_ascii_lowercase();
            if seen.insert(dedupe_key) {
                labels.push(label.to_owned());
            }
        }
    }

    Ok(Some(labels))
}

fn legacy_tag_locale(metadata: &Value) -> String {
    metadata
        .get("locale")
        .and_then(Value::as_str)
        .and_then(normalize_term_locale)
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_owned())
}

async fn ensure_product_tag_terms(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    labels: &[String],
) -> Result<Vec<Uuid>, DbErr> {
    let mut term_ids = Vec::new();
    let mut seen = HashSet::new();

    for label in labels {
        let route_key = normalize_term_route_key(label).ok_or_else(|| {
            DbErr::Migration(format!(
                "product metadata-tag canonicalization blocked: tag {label:?} has an empty normalized route key"
            ))
        })?;

        let term_id = match find_existing_term(txn, tenant_id, locale, &route_key).await? {
            Some(term_id) => term_id,
            None => create_product_term(txn, tenant_id, locale, label, &route_key).await?,
        };
        if seen.insert(term_id) {
            term_ids.push(term_id);
        }
    }

    Ok(term_ids)
}

async fn find_existing_term(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    route_key: &str,
) -> Result<Option<Uuid>, DbErr> {
    for (scope_type, scope_value) in [
        (TaxonomyScopeType::Module, PRODUCT_SCOPE_VALUE),
        (TaxonomyScopeType::Global, ""),
    ] {
        for candidate_locale in locale_candidates(locale) {
            if let Some(route) = taxonomy_term_route_key::Entity::find()
                .filter(taxonomy_term_route_key::Column::TenantId.eq(tenant_id))
                .filter(taxonomy_term_route_key::Column::Kind.eq(TaxonomyTermKind::Tag))
                .filter(taxonomy_term_route_key::Column::ScopeType.eq(scope_type))
                .filter(taxonomy_term_route_key::Column::ScopeValue.eq(scope_value))
                .filter(taxonomy_term_route_key::Column::Locale.eq(candidate_locale))
                .filter(taxonomy_term_route_key::Column::RouteKey.eq(route_key))
                .one(txn)
                .await?
            {
                return Ok(Some(route.term_id));
            }
        }

        if let Some(term) = taxonomy_term::Entity::find()
            .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
            .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
            .filter(taxonomy_term::Column::ScopeType.eq(scope_type))
            .filter(taxonomy_term::Column::ScopeValue.eq(scope_value))
            .filter(taxonomy_term::Column::CanonicalKey.eq(route_key))
            .one(txn)
            .await?
        {
            return Ok(Some(term.id));
        }
    }

    Ok(None)
}

async fn create_product_term(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    label: &str,
    route_key: &str,
) -> Result<Uuid, DbErr> {
    if taxonomy_term::Entity::find()
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(TaxonomyTermKind::Tag))
        .filter(taxonomy_term::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term::Column::ScopeValue.eq(PRODUCT_SCOPE_VALUE))
        .filter(taxonomy_term::Column::CanonicalKey.eq(route_key))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(DbErr::Migration(format!(
            "product metadata-tag canonicalization found a concurrent canonical-key owner for {route_key}"
        )));
    }

    if taxonomy_term_route_key::Entity::find()
        .filter(taxonomy_term_route_key::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term_route_key::Column::Kind.eq(TaxonomyTermKind::Tag))
        .filter(taxonomy_term_route_key::Column::ScopeType.eq(TaxonomyScopeType::Module))
        .filter(taxonomy_term_route_key::Column::ScopeValue.eq(PRODUCT_SCOPE_VALUE))
        .filter(taxonomy_term_route_key::Column::Locale.eq(locale))
        .filter(taxonomy_term_route_key::Column::RouteKey.eq(route_key))
        .one(txn)
        .await?
        .is_some()
    {
        return Err(DbErr::Migration(format!(
            "product metadata-tag canonicalization found a concurrent route-key owner for {route_key}"
        )));
    }

    let term_id = Uuid::new_v4();
    taxonomy_term::ActiveModel {
        id: Set(term_id),
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Tag),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set(PRODUCT_SCOPE_VALUE.to_owned()),
        canonical_key: Set(route_key.to_owned()),
        revision: Set(1),
        created_at: Set(Utc::now().fixed_offset()),
        updated_at: Set(Utc::now().fixed_offset()),
    }
    .insert(txn)
    .await?;

    taxonomy_term_translation::ActiveModel {
        id: Set(Uuid::new_v4()),
        term_id: Set(term_id),
        tenant_id: Set(tenant_id),
        locale: Set(locale.to_owned()),
        name: Set(label.to_owned()),
        slug: Set(route_key.to_owned()),
        description: Set(None),
        revision: Set(1),
        created_at: Set(Utc::now().fixed_offset()),
        updated_at: Set(Utc::now().fixed_offset()),
    }
    .insert(txn)
    .await?;

    taxonomy_term_route_key::ActiveModel {
        tenant_id: Set(tenant_id),
        kind: Set(TaxonomyTermKind::Tag),
        scope_type: Set(TaxonomyScopeType::Module),
        scope_value: Set(PRODUCT_SCOPE_VALUE.to_owned()),
        locale: Set(locale.to_owned()),
        route_key: Set(route_key.to_owned()),
        term_id: Set(term_id),
    }
    .insert(txn)
    .await?;

    translation_change::ActiveModel {
        id: Set(Uuid::new_v4()),
        tenant_id: Set(tenant_id),
        term_id: Set(term_id),
        locale: Set(locale.to_owned()),
        resource_revision: Set(1),
        target_revision: Set(1),
        operation: Set("upsert".to_owned()),
        lifecycle: Set("active".to_owned()),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(txn)
    .await?;

    Ok(term_id)
}

fn locale_candidates(locale: &str) -> Vec<String> {
    if locale == PLATFORM_FALLBACK_LOCALE {
        vec![locale.to_owned()]
    } else {
        vec![locale.to_owned(), PLATFORM_FALLBACK_LOCALE.to_owned()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn legacy_metadata_tags_preserve_order_and_dedupe_ascii_case() {
        let tags = legacy_metadata_tags(&json!({
            "tags": [" Sale ", "new", "sale", null, 7, ""]
        }))
        .expect("metadata tags should parse")
        .expect("tags key should be present");

        assert_eq!(tags, vec!["Sale".to_owned(), "new".to_owned()]);
    }

    #[test]
    fn legacy_metadata_tags_distinguish_absent_key_from_empty_payload() {
        assert_eq!(
            legacy_metadata_tags(&json!({"featured": true})).expect("metadata should parse"),
            None
        );
        assert_eq!(
            legacy_metadata_tags(&json!({"tags": "not-an-array"}))
                .expect("metadata should parse"),
            Some(Vec::new())
        );
    }

    #[test]
    fn legacy_tag_locale_normalizes_and_falls_back() {
        assert_eq!(legacy_tag_locale(&json!({"locale": "EN-us"})), "en-US");
        assert_eq!(
            legacy_tag_locale(&json!({"locale": "not a locale"})),
            PLATFORM_FALLBACK_LOCALE
        );
        assert_eq!(
            locale_candidates("fr"),
            vec!["fr".to_owned(), PLATFORM_FALLBACK_LOCALE.to_owned()]
        );
    }
}
