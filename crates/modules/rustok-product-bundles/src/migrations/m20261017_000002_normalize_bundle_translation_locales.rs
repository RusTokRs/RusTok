use std::collections::BTreeMap;

use rustok_api::normalize_locale_tag;
use sea_orm::{ConnectionTrait, DatabaseBackend, FromQueryResult, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Debug, FromQueryResult)]
struct BundleTranslationRow {
    id: Uuid,
    bundle_id: Uuid,
    locale: String,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        // SeaORM leaves SQLite migrations unwrapped by default. A locale normalization rewrites
        // multiple rows, so a storage failure must not leave a Bundle half-normalized.
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = connection.get_database_backend();
        if !matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::Sqlite) {
            return Ok(());
        }

        // The original Bundle schema used VARCHAR(16), while the shared locale contract permits
        // canonical extension-free tags through 32 characters. SQLite already stores this column
        // as TEXT; PostgreSQL must expand it before normalized rows are written.
        if backend == DatabaseBackend::Postgres {
            connection
                .execute_unprepared(
                    "ALTER TABLE product_bundle_translations ALTER COLUMN locale TYPE VARCHAR(32)",
                )
                .await?;
        }

        let rows = BundleTranslationRow::find_by_statement(Statement::from_string(
            backend,
            r#"
SELECT id, bundle_id, locale
FROM product_bundle_translations
ORDER BY bundle_id, locale, id
"#
            .to_owned(),
        ))
        .all(connection)
        .await?;

        let mut normalized_owners = BTreeMap::<(Uuid, String), Uuid>::new();
        let mut normalized_rows = Vec::with_capacity(rows.len());
        for row in rows {
            let normalized_locale = normalize_locale_tag(&row.locale).ok_or_else(|| {
                DbErr::Migration(format!(
                    "Product Bundle translation {} for bundle {} has invalid locale {:?}",
                    row.id, row.bundle_id, row.locale
                ))
            })?;
            let key = (row.bundle_id, normalized_locale.clone());
            if let Some(existing_id) = normalized_owners.get(&key) {
                return Err(DbErr::Migration(format!(
                    "Product Bundle locale normalization collision for bundle {} locale {} between translations {} and {}",
                    row.bundle_id, normalized_locale, existing_id, row.id
                )));
            }
            normalized_owners.insert(key, row.id);
            normalized_rows.push((row.id, row.locale, normalized_locale));
        }

        let update_sql = match backend {
            DatabaseBackend::Postgres => {
                "UPDATE product_bundle_translations SET locale = $1 WHERE id = $2"
            }
            DatabaseBackend::Sqlite => {
                "UPDATE product_bundle_translations SET locale = ?1 WHERE id = ?2"
            }
            _ => unreachable!("unsupported backend was filtered above"),
        };
        for (id, stored_locale, normalized_locale) in normalized_rows {
            if stored_locale == normalized_locale {
                continue;
            }
            connection
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    update_sql,
                    vec![normalized_locale.into(), id.into()],
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Canonical locale tags are semantically equivalent to accepted legacy spellings. They
        // may now require the shared 32-character storage width, so this expand/normalize step is
        // intentionally irreversible.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::normalize_locale_tag;
    use sea_orm_migration::MigrationTrait;

    use super::Migration;

    #[test]
    fn locale_normalization_is_transactional_on_sqlite() {
        let migration = Migration;
        assert_eq!(migration.use_transaction(), Some(true));
    }

    #[test]
    fn locale_normalization_uses_the_shared_platform_contract() {
        assert_eq!(normalize_locale_tag(" EN_us ").as_deref(), Some("en-US"));
        assert_eq!(normalize_locale_tag("ru_RU").as_deref(), Some("ru-RU"));
        assert!(normalize_locale_tag(" ").is_none());
    }
}
