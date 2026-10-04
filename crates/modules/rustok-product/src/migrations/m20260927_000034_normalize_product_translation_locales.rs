use std::collections::BTreeMap;

use rustok_api::normalize_locale_tag;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Debug, FromQueryResult)]
struct ProductTranslationRow {
    id: Uuid,
    product_id: Uuid,
    locale: String,
    title: String,
}

#[derive(Debug, FromQueryResult)]
struct VariantTranslationRow {
    id: Uuid,
    variant_id: Uuid,
    locale: String,
}

#[derive(Debug, FromQueryResult)]
struct ImageTranslationRow {
    id: Uuid,
    image_id: Uuid,
    locale: String,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        let connection = manager.get_connection();

        // 1. Normalize product_translations
        let product_rows = ProductTranslationRow::find_by_statement(Statement::from_string(
            connection.get_database_backend(),
            r#"
SELECT id, product_id, locale, title
FROM product_translations
ORDER BY product_id, locale, id
"#
            .to_string(),
        ))
        .all(connection)
        .await?;

        let mut normalized_product_owners = BTreeMap::<(Uuid, String), Uuid>::new();
        let mut normalized_product_rows = Vec::with_capacity(product_rows.len());

        for row in product_rows {
            if row.title.trim().is_empty() {
                return Err(DbErr::Migration(format!(
                    "product translation {} for product {} has an empty title",
                    row.id, row.product_id
                )));
            }
            let normalized_locale = normalize_locale_tag(&row.locale).ok_or_else(|| {
                DbErr::Migration(format!(
                    "product translation {} for product {} has invalid locale {:?}",
                    row.id, row.product_id, row.locale
                ))
            })?;

            let key = (row.product_id, normalized_locale.clone());
            if let Some(existing_id) = normalized_product_owners.get(&key) {
                return Err(DbErr::Migration(format!(
                    "product translation locale normalization collision for product {} locale {} between translations {} and {}",
                    row.product_id, normalized_locale, existing_id, row.id
                )));
            }
            normalized_product_owners.insert(key, row.id);
            normalized_product_rows.push((row.id, row.locale, normalized_locale));
        }

        for (id, stored_locale, normalized_locale) in normalized_product_rows {
            if stored_locale == normalized_locale {
                continue;
            }
            connection
                .execute_raw(Statement::from_sql_and_values(
                    connection.get_database_backend(),
                    "UPDATE product_translations SET locale = $1 WHERE id = $2",
                    vec![normalized_locale.into(), id.into()],
                ))
                .await?;
        }

        // 2. Normalize product_variant_translations
        let variant_rows = VariantTranslationRow::find_by_statement(Statement::from_string(
            connection.get_database_backend(),
            r#"
SELECT id, variant_id, locale
FROM product_variant_translations
ORDER BY variant_id, locale, id
"#
            .to_string(),
        ))
        .all(connection)
        .await?;

        let mut normalized_variant_owners = BTreeMap::<(Uuid, String), Uuid>::new();
        let mut normalized_variant_rows = Vec::with_capacity(variant_rows.len());

        for row in variant_rows {
            let normalized_locale = normalize_locale_tag(&row.locale).ok_or_else(|| {
                DbErr::Migration(format!(
                    "variant translation {} for variant {} has invalid locale {:?}",
                    row.id, row.variant_id, row.locale
                ))
            })?;

            let key = (row.variant_id, normalized_locale.clone());
            if let Some(existing_id) = normalized_variant_owners.get(&key) {
                return Err(DbErr::Migration(format!(
                    "variant translation locale normalization collision for variant {} locale {} between translations {} and {}",
                    row.variant_id, normalized_locale, existing_id, row.id
                )));
            }
            normalized_variant_owners.insert(key, row.id);
            normalized_variant_rows.push((row.id, row.locale, normalized_locale));
        }

        for (id, stored_locale, normalized_locale) in normalized_variant_rows {
            if stored_locale == normalized_locale {
                continue;
            }
            connection
                .execute_raw(Statement::from_sql_and_values(
                    connection.get_database_backend(),
                    "UPDATE product_variant_translations SET locale = $1 WHERE id = $2",
                    vec![normalized_locale.into(), id.into()],
                ))
                .await?;
        }

        // 3. Normalize Product Image translations. Image translations are consumed by the same
        // exact-locale Translation boundary as product and variant copy; leaving this table out
        // would make rows written through older image CRUD unreachable by that boundary.
        let image_rows = ImageTranslationRow::find_by_statement(Statement::from_string(
            connection.get_database_backend(),
            r#"
SELECT id, image_id, locale
FROM product_image_translations
ORDER BY image_id, locale, id
"#
            .to_string(),
        ))
        .all(connection)
        .await?;

        let mut normalized_image_owners = BTreeMap::<(Uuid, String), Uuid>::new();
        let mut normalized_image_rows = Vec::with_capacity(image_rows.len());

        for row in image_rows {
            let normalized_locale = normalize_locale_tag(&row.locale).ok_or_else(|| {
                DbErr::Migration(format!(
                    "product image translation {} for image {} has invalid locale {:?}",
                    row.id, row.image_id, row.locale
                ))
            })?;

            let key = (row.image_id, normalized_locale.clone());
            if let Some(existing_id) = normalized_image_owners.get(&key) {
                return Err(DbErr::Migration(format!(
                    "product image translation locale normalization collision for image {} locale {} between translations {} and {}",
                    row.image_id, normalized_locale, existing_id, row.id
                )));
            }
            normalized_image_owners.insert(key, row.id);
            normalized_image_rows.push((row.id, row.locale, normalized_locale));
        }

        for (id, stored_locale, normalized_locale) in normalized_image_rows {
            if stored_locale == normalized_locale {
                continue;
            }
            connection
                .execute_raw(Statement::from_sql_and_values(
                    connection.get_database_backend(),
                    "UPDATE product_image_translations SET locale = $1 WHERE id = $2",
                    vec![normalized_locale.into(), id.into()],
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_normalization_matches_shared_platform_contract() {
        assert_eq!(normalize_locale_tag(" EN_us ").as_deref(), Some("en-US"));
        assert_eq!(normalize_locale_tag("ru_RU").as_deref(), Some("ru-RU"));
        assert!(normalize_locale_tag(" ").is_none());
    }
}
