use std::collections::BTreeMap;

use rustok_api::normalize_locale_tag;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Debug, FromQueryResult)]
struct SchemaTranslationRow {
    id: Uuid,
    schema_id: Uuid,
    locale: String,
    name: String,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        let connection = manager.get_connection();
        let rows = SchemaTranslationRow::find_by_statement(Statement::from_string(
            connection.get_database_backend(),
            r#"
SELECT id, schema_id, locale, name
FROM product_attribute_schema_translations
ORDER BY schema_id, locale, id
"#
            .to_string(),
        ))
        .all(connection)
        .await?;

        let mut normalized_owners = BTreeMap::<(Uuid, String), Uuid>::new();
        let mut normalized_rows = Vec::with_capacity(rows.len());

        for row in rows {
            if row.name.trim().is_empty() {
                return Err(DbErr::Migration(format!(
                    "product attribute schema translation {} for schema {} has an empty name",
                    row.id, row.schema_id
                )));
            }
            let normalized_locale = normalize_locale_tag(&row.locale).ok_or_else(|| {
                DbErr::Migration(format!(
                    "product attribute schema translation {} for schema {} has invalid locale {:?}",
                    row.id, row.schema_id, row.locale
                ))
            })?;

            let key = (row.schema_id, normalized_locale.clone());
            if let Some(existing_id) = normalized_owners.get(&key) {
                return Err(DbErr::Migration(format!(
                    "product attribute schema locale normalization collision for schema {} locale {} between translations {} and {}",
                    row.schema_id, normalized_locale, existing_id, row.id
                )));
            }
            normalized_owners.insert(key, row.id);
            normalized_rows.push((row.id, row.locale, normalized_locale));
        }

        for (id, stored_locale, normalized_locale) in normalized_rows {
            if stored_locale == normalized_locale {
                continue;
            }
            connection
                .execute(Statement::from_sql_and_values(
                    connection.get_database_backend(),
                    "UPDATE product_attribute_schema_translations SET locale = $1 WHERE id = $2",
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
