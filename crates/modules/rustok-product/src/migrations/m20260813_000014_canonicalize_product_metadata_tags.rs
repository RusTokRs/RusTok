use std::collections::HashSet;

use chrono::{Duration, Utc};
use rustok_api::PLATFORM_FALLBACK_LOCALE;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait,
    FromQueryResult, QueryFilter, Statement, TransactionTrait,
};
use sea_orm_migration::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::entities::product_tag;

const PRODUCT_SCOPE_VALUE: &str = "product";
const BACKFILL_BATCH_SIZE: u64 = 256;
const METADATA_TAG_CONSTRAINT: &str = "ck_products_metadata_tags_absent";

#[derive(Debug, FromQueryResult)]
struct MigrationProductRow {
    id: Uuid,
    tenant_id: Uuid,
    metadata: Value,
}

#[derive(Debug, FromQueryResult)]
struct CountRow {
    count: i64,
}

#[derive(Debug, FromQueryResult)]
struct TermIdRow {
    id: Uuid,
}

#[derive(Debug, FromQueryResult)]
struct RouteKeyTermIdRow {
    term_id: Uuid,
}

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
            backfill_legacy_metadata_tags(&txn).await?;
        }

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
    let mut last_product_id: Option<Uuid> = None;

    loop {
        let (sql, values) = match last_product_id {
            Some(last_id) => (
                "SELECT id, tenant_id, metadata FROM products WHERE id > $1 ORDER BY id ASC LIMIT $2",
                vec![last_id.into(), (BACKFILL_BATCH_SIZE as i64).into()],
            ),
            None => (
                "SELECT id, tenant_id, metadata FROM products ORDER BY id ASC LIMIT $1",
                vec![(BACKFILL_BATCH_SIZE as i64).into()],
            ),
        };
        let products = MigrationProductRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            sql,
            values,
        ))
        .all(txn)
        .await?;
        if products.is_empty() {
            break;
        }
        last_product_id = products.last().map(|product| product.id);

        for product in products {
            let Some(labels) = legacy_metadata_tags(&product.metadata)? else {
                continue;
            };

            if !labels.is_empty() {
                let existing_term_ids: HashSet<Uuid> = product_tag::Entity::find()
                    .filter(product_tag::Column::TenantId.eq(product.tenant_id))
                    .filter(product_tag::Column::ProductId.eq(product.id))
                    .all(txn)
                    .await?
                    .into_iter()
                    .map(|tag| tag.term_id)
                    .collect();

                let locale = legacy_tag_locale(&product.metadata);
                let term_ids =
                    ensure_product_tag_terms(txn, product.tenant_id, &locale, &labels).await?;
                let created_at = Utc::now();
                for (position, term_id) in term_ids.into_iter().enumerate() {
                    if !existing_term_ids.contains(&term_id) {
                        product_tag::ActiveModel {
                            product_id: Set(product.id),
                            term_id: Set(term_id),
                            tenant_id: Set(product.tenant_id),
                            created_at: Set(
                                (created_at + Duration::microseconds(position as i64)).into()
                            ),
                        }
                        .insert(txn)
                        .await?;
                    }
                }
            }

            let mut metadata = product.metadata.clone();
            if let Some(object) = metadata.as_object_mut() {
                object.remove("tags");
            }
            let updated = txn
                .execute_raw(Statement::from_sql_and_values(
                    DatabaseBackend::Postgres,
                    "UPDATE products SET metadata = $1 WHERE id = $2 AND tenant_id = $3",
                    vec![metadata.into(), product.id.into(), product.tenant_id.into()],
                ))
                .await?;
            if updated.rows_affected() != 1 {
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
            let Some(label) = item
                .as_str()
                .map(str::trim)
                .filter(|label| !label.is_empty())
            else {
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

fn normalize_term_route_key(label: &str) -> Option<String> {
    let key = crate::services::catalog::helpers::slugify(label);
    (!key.is_empty()).then_some(key)
}

fn normalize_term_locale(raw: &str) -> Option<String> {
    rustok_api::normalize_locale_tag(raw)
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
    for (scope_type, scope_value) in [("module", PRODUCT_SCOPE_VALUE), ("global", "")] {
        for candidate_locale in locale_candidates(locale) {
            let row = RouteKeyTermIdRow::find_by_statement(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                    SELECT term_id
                    FROM taxonomy_term_route_keys
                    WHERE tenant_id = $1
                      AND kind = 'tag'
                      AND scope_type = $2
                      AND scope_value = $3
                      AND locale = $4
                      AND route_key = $5
                    LIMIT 1
                "#,
                vec![
                    tenant_id.into(),
                    scope_type.into(),
                    scope_value.into(),
                    candidate_locale.into(),
                    route_key.into(),
                ],
            ))
            .one(txn)
            .await?;
            if let Some(route) = row {
                return Ok(Some(route.term_id));
            }
        }

        let row = TermIdRow::find_by_statement(Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
                SELECT id
                FROM taxonomy_terms
                WHERE tenant_id = $1
                  AND kind = 'tag'
                  AND scope_type = $2
                  AND scope_value = $3
                  AND canonical_key = $4
                LIMIT 1
            "#,
            vec![
                tenant_id.into(),
                scope_type.into(),
                scope_value.into(),
                route_key.into(),
            ],
        ))
        .one(txn)
        .await?;
        if let Some(term) = row {
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
    let existing_canonical = TermIdRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            SELECT id
            FROM taxonomy_terms
            WHERE tenant_id = $1
              AND kind = 'tag'
              AND scope_type = 'module'
              AND scope_value = $2
              AND canonical_key = $3
            LIMIT 1
        "#,
        vec![
            tenant_id.into(),
            PRODUCT_SCOPE_VALUE.into(),
            route_key.into(),
        ],
    ))
    .one(txn)
    .await?;

    if existing_canonical.is_some() {
        return Err(DbErr::Migration(format!(
            "product metadata-tag canonicalization found a concurrent canonical-key owner for {route_key}"
        )));
    }

    let existing_route = RouteKeyTermIdRow::find_by_statement(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            SELECT term_id
            FROM taxonomy_term_route_keys
            WHERE tenant_id = $1
              AND kind = 'tag'
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

    if existing_route.is_some() {
        return Err(DbErr::Migration(format!(
            "product metadata-tag canonicalization found a concurrent route-key owner for {route_key}"
        )));
    }

    let term_id = Uuid::new_v4();
    let now = Utc::now().fixed_offset();

    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO taxonomy_terms (
                id, tenant_id, kind, scope_type, scope_value, canonical_key, revision, created_at, updated_at
            ) VALUES ($1, $2, 'tag', 'module', $3, $4, 1, $5, $5)
        "#,
        vec![
            term_id.into(),
            tenant_id.into(),
            PRODUCT_SCOPE_VALUE.into(),
            route_key.into(),
            now.into(),
        ],
    ))
    .await?;

    let translation_id = Uuid::new_v4();
    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO taxonomy_term_translations (
                id, term_id, tenant_id, locale, name, slug, description, revision, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, NULL, 1, $7, $7)
        "#,
        vec![
            translation_id.into(),
            term_id.into(),
            tenant_id.into(),
            locale.into(),
            label.into(),
            route_key.into(),
            now.into(),
        ],
    ))
    .await?;

    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO taxonomy_term_route_keys (
                tenant_id, kind, scope_type, scope_value, locale, route_key, term_id
            ) VALUES ($1, 'tag', 'module', $2, $3, $4, $5)
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

    let change_id = Uuid::new_v4();
    txn.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
            INSERT INTO translation_changes (
                id, tenant_id, term_id, locale, resource_revision, target_revision, operation, lifecycle, created_at
            ) VALUES ($1, $2, $3, $4, 1, 1, 'upsert', 'active', $5)
        "#,
        vec![
            change_id.into(),
            tenant_id.into(),
            term_id.into(),
            locale.into(),
            now.into(),
        ],
    ))
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
            legacy_metadata_tags(&json!({"tags": "not-an-array"})).expect("metadata should parse"),
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
