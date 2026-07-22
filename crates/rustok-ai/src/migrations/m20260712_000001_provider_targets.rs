use sea_orm::{ConnectionTrait, DbBackend, Statement, TryGetable};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = manager.get_database_backend();
        let custom_endpoint_query = match backend {
            DbBackend::Postgres => {
                "SELECT slug FROM ai_provider_profiles \
                 WHERE COALESCE(settings->>'base_url', '') <> '' ORDER BY slug"
            }
            DbBackend::Sqlite => {
                "SELECT slug FROM ai_provider_profiles \
                 WHERE COALESCE(json_extract(settings, '$.base_url'), '') <> '' ORDER BY slug"
            }
            other => {
                return Err(DbErr::Migration(format!(
                    "AI provider target migration does not support database backend {other:?}"
                )));
            }
        };
        let rows = connection
            .query_all(Statement::from_string(
                backend,
                custom_endpoint_query.to_string(),
            ))
            .await?;
        if !rows.is_empty() {
            let profiles = rows
                .iter()
                .filter_map(|row| String::try_get(row, "", "slug").ok())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(DbErr::Migration(format!(
                "AI provider target migration blocked by custom tenant endpoints in profiles: {profiles}. Create deployment-owned targets and clear legacy settings before retrying"
            )));
        }

        let statements: &[&str] = match backend {
            DbBackend::Postgres => &[
                "ALTER TABLE ai_provider_profiles ADD COLUMN provider_target_id TEXT",
                "UPDATE ai_provider_profiles SET provider_target_id = provider_slug",
                "ALTER TABLE ai_provider_profiles ALTER COLUMN provider_target_id SET NOT NULL",
                "ALTER TABLE ai_provider_profiles DROP COLUMN settings",
            ],
            DbBackend::Sqlite => &[
                // SQLite cannot add a non-null column without a default. The default is
                // immediately overwritten for every existing row, and is only retained as
                // a database-level guard for rows created outside the application.
                "ALTER TABLE ai_provider_profiles ADD COLUMN provider_target_id TEXT NOT NULL DEFAULT ''",
                "UPDATE ai_provider_profiles SET provider_target_id = provider_slug",
                "ALTER TABLE ai_provider_profiles DROP COLUMN settings",
            ],
            other => {
                return Err(DbErr::Migration(format!(
                    "AI provider target migration does not support database backend {other:?}"
                )));
            }
        };
        for statement in statements {
            connection.execute_unprepared(statement).await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "AI provider targets are intentionally irreversible because endpoints are deployment-owned"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Migration;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    async fn legacy_database(settings: &str) -> sea_orm::DatabaseConnection {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        database
            .execute_unprepared(
                "CREATE TABLE ai_provider_profiles (\
                    slug TEXT NOT NULL,\
                    provider_slug TEXT NOT NULL,\
                    settings JSON NOT NULL)",
            )
            .await
            .unwrap();
        database
            .execute_unprepared(&format!(
                "INSERT INTO ai_provider_profiles (slug, provider_slug, settings) \
                 VALUES ('primary', 'openai_compatible', '{}')",
                settings.replace('\'', "''")
            ))
            .await
            .unwrap();
        database
    }

    #[tokio::test]
    async fn migrates_standard_profile_to_its_deployment_target_id() {
        let database = legacy_database("{}").await;
        Migration.up(&SchemaManager::new(&database)).await.unwrap();
        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT provider_target_id FROM ai_provider_profiles".to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            String::try_get(&row, "", "provider_target_id").unwrap(),
            "openai_compatible"
        );
        let columns = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA table_info(ai_provider_profiles)".to_string(),
            ))
            .await
            .unwrap()
            .into_iter()
            .map(|row| {
                (
                    String::try_get(&row, "", "name").unwrap(),
                    i64::try_get(&row, "", "notnull").unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert!(columns.contains(&("provider_target_id".to_string(), 1)));
        assert!(!columns.iter().any(|(name, _)| name == "settings"));
    }

    #[tokio::test]
    async fn maps_each_legacy_provider_slug_to_its_named_deployment_target() {
        let database = legacy_database("{}").await;
        for (slug, provider_slug) in [
            ("anthropic_primary", "anthropic"),
            ("gemini_primary", "gemini"),
        ] {
            database
                .execute_unprepared(&format!(
                    "INSERT INTO ai_provider_profiles (slug, provider_slug, settings) \
                     VALUES ('{slug}', '{provider_slug}', '{{}}')"
                ))
                .await
                .unwrap();
        }
        Migration.up(&SchemaManager::new(&database)).await.unwrap();
        let rows = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT slug, provider_slug, provider_target_id FROM ai_provider_profiles ORDER BY slug"
                    .to_string(),
            ))
            .await
            .unwrap();
        let mappings = rows
            .iter()
            .map(|row| {
                (
                    String::try_get(row, "", "slug").unwrap(),
                    String::try_get(row, "", "provider_slug").unwrap(),
                    String::try_get(row, "", "provider_target_id").unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            mappings,
            vec![
                (
                    "anthropic_primary".to_string(),
                    "anthropic".to_string(),
                    "anthropic".to_string()
                ),
                (
                    "gemini_primary".to_string(),
                    "gemini".to_string(),
                    "gemini".to_string()
                ),
                (
                    "primary".to_string(),
                    "openai_compatible".to_string(),
                    "openai_compatible".to_string(),
                ),
            ]
        );
    }

    #[tokio::test]
    async fn rejects_custom_tenant_endpoint_without_a_deployment_mapping() {
        let database = legacy_database(r#"{"base_url":"https://gateway.example.test/v1"}"#).await;
        let error = Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect_err("custom endpoint must require operator mapping");
        assert!(error.to_string().contains("primary"));
    }
}
