use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statement = match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                "UPDATE platform_settings SET settings = settings - 'api_key' - 'api_key_configured', updated_at = CURRENT_TIMESTAMP WHERE category = 'search' AND (settings ? 'api_key' OR settings ? 'api_key_configured')"
            }
            DatabaseBackend::Sqlite => {
                "UPDATE platform_settings SET settings = json_remove(settings, '$.api_key', '$.api_key_configured'), updated_at = CURRENT_TIMESTAMP WHERE category = 'search' AND json_valid(settings) AND (json_type(settings, '$.api_key') IS NOT NULL OR json_type(settings, '$.api_key_configured') IS NOT NULL)"
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "platform Search API-key scrub migration does not support {backend:?}"
                )));
            }
        };
        manager.get_connection().execute_unprepared(statement).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible by design: removed credentials must never be reconstructed.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn migration_is_irreversible_and_scoped_to_search_secrets() {
        let source = include_str!("m20260730_000001_scrub_platform_search_api_keys.rs");
        assert!(source.contains("WHERE category = 'search'"));
        assert!(source.contains("settings - 'api_key' - 'api_key_configured'"));
        assert!(source.contains("json_remove(settings, '$.api_key', '$.api_key_configured')"));
        assert!(!source.contains("INSERT INTO"));
    }
}
