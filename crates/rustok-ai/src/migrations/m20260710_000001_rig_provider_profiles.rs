use sea_orm::{ConnectionTrait, DbBackend, Statement, TryGetable};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = manager.get_database_backend();
        let rows = connection
            .query_all(Statement::from_string(
                backend,
                "SELECT slug FROM ai_provider_profiles WHERE api_key_secret IS NOT NULL AND TRIM(api_key_secret) <> '' ORDER BY slug".to_string(),
            ))
            .await?;
        if !rows.is_empty() {
            let profiles = rows
                .iter()
                .filter_map(|row| String::try_get(row, "", "slug").ok())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(DbErr::Migration(format!(
                "AI provider migration blocked by plaintext api_key_secret values in profiles: {profiles}. Move credentials to external secrets or deactivate and clear each profile before retrying"
            )));
        }

        let statements: &[&str] = match backend {
            DbBackend::Postgres => &[
                "ALTER TABLE ai_provider_profiles ADD COLUMN settings JSONB NOT NULL DEFAULT '{}'::jsonb",
                "ALTER TABLE ai_provider_profiles ADD COLUMN credential_refs JSONB NOT NULL DEFAULT '{}'::jsonb",
                "UPDATE ai_provider_profiles SET settings = jsonb_build_object('base_url', base_url) WHERE TRIM(base_url) <> ''",
                "ALTER TABLE ai_provider_profiles RENAME COLUMN provider_kind TO provider_slug",
                "ALTER TABLE ai_provider_profiles DROP COLUMN base_url",
                "ALTER TABLE ai_provider_profiles DROP COLUMN api_key_secret",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE ai_provider_profiles ADD COLUMN settings JSON NOT NULL DEFAULT '{}'",
                "ALTER TABLE ai_provider_profiles ADD COLUMN credential_refs JSON NOT NULL DEFAULT '{}'",
                "UPDATE ai_provider_profiles SET settings = json_object('base_url', base_url) WHERE TRIM(base_url) <> ''",
                "ALTER TABLE ai_provider_profiles RENAME COLUMN provider_kind TO provider_slug",
                "ALTER TABLE ai_provider_profiles DROP COLUMN base_url",
                "ALTER TABLE ai_provider_profiles DROP COLUMN api_key_secret",
            ],
            other => {
                return Err(DbErr::Migration(format!(
                    "AI Rig provider migration does not support database backend {other:?}"
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
            "AI Rig provider migration is intentionally irreversible because plaintext credential storage was removed"
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Migration;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    async fn legacy_database(api_key_secret: Option<&str>) -> sea_orm::DatabaseConnection {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        database
            .execute_unprepared(
                "CREATE TABLE ai_provider_profiles (\
                    slug TEXT NOT NULL,\
                    provider_kind TEXT NOT NULL,\
                    base_url TEXT NOT NULL DEFAULT '',\
                    api_key_secret TEXT\
                )",
            )
            .await
            .unwrap();
        let secret = api_key_secret
            .map(|value| format!("'{}'", value.replace('\'', "''")))
            .unwrap_or_else(|| "NULL".to_string());
        database
            .execute_unprepared(&format!(
                "INSERT INTO ai_provider_profiles (slug, provider_kind, base_url, api_key_secret) \
                 VALUES ('primary', 'openai_compatible', 'https://gateway.example.test/v1', {secret})"
            ))
            .await
            .unwrap();
        database
    }

    #[tokio::test]
    async fn migrates_slug_and_base_url_without_retaining_plaintext_column() {
        let database = legacy_database(None).await;
        Migration.up(&SchemaManager::new(&database)).await.unwrap();

        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT provider_slug, settings, credential_refs FROM ai_provider_profiles"
                    .to_string(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            String::try_get(&row, "", "provider_slug").unwrap(),
            "openai_compatible"
        );
        assert_eq!(
            String::try_get(&row, "", "settings").unwrap(),
            r#"{"base_url":"https://gateway.example.test/v1"}"#
        );
        assert_eq!(String::try_get(&row, "", "credential_refs").unwrap(), "{}");

        let columns = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA table_info(ai_provider_profiles)".to_string(),
            ))
            .await
            .unwrap()
            .into_iter()
            .map(|row| String::try_get(&row, "", "name").unwrap())
            .collect::<Vec<_>>();
        assert!(columns.contains(&"provider_slug".to_string()));
        assert!(!columns.contains(&"provider_kind".to_string()));
        assert!(!columns.contains(&"base_url".to_string()));
        assert!(!columns.contains(&"api_key_secret".to_string()));
    }

    #[tokio::test]
    async fn preserves_all_supported_legacy_provider_slugs_without_plaintext_fallback() {
        let database = legacy_database(None).await;
        for (slug, provider_kind) in [
            ("anthropic_primary", "anthropic"),
            ("gemini_primary", "gemini"),
        ] {
            database
                .execute_unprepared(&format!(
                    "INSERT INTO ai_provider_profiles (slug, provider_kind, base_url, api_key_secret) \
                     VALUES ('{slug}', '{provider_kind}', 'https://gateway.example.test/{provider_kind}', NULL)"
                ))
                .await
                .unwrap();
        }
        Migration.up(&SchemaManager::new(&database)).await.unwrap();

        let rows = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT slug, provider_slug, settings, credential_refs FROM ai_provider_profiles ORDER BY slug"
                    .to_string(),
            ))
            .await
            .unwrap();
        let migrated = rows
            .iter()
            .map(|row| {
                (
                    String::try_get(row, "", "slug").unwrap(),
                    String::try_get(row, "", "provider_slug").unwrap(),
                    String::try_get(row, "", "settings").unwrap(),
                    String::try_get(row, "", "credential_refs").unwrap(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            migrated,
            vec![
                (
                    "anthropic_primary".to_string(),
                    "anthropic".to_string(),
                    r#"{"base_url":"https://gateway.example.test/anthropic"}"#.to_string(),
                    "{}".to_string(),
                ),
                (
                    "gemini_primary".to_string(),
                    "gemini".to_string(),
                    r#"{"base_url":"https://gateway.example.test/gemini"}"#.to_string(),
                    "{}".to_string(),
                ),
                (
                    "primary".to_string(),
                    "openai_compatible".to_string(),
                    r#"{"base_url":"https://gateway.example.test/v1"}"#.to_string(),
                    "{}".to_string(),
                ),
            ]
        );
    }

    #[tokio::test]
    async fn rejects_plaintext_secret_before_schema_mutation() {
        let database = legacy_database(Some("do-not-migrate")).await;
        let error = Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect_err("plaintext credentials must stop the upgrade");
        assert!(error.to_string().contains("primary"));

        let columns = database
            .query_all(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA table_info(ai_provider_profiles)".to_string(),
            ))
            .await
            .unwrap()
            .into_iter()
            .map(|row| String::try_get(&row, "", "name").unwrap())
            .collect::<Vec<_>>();
        assert!(columns.contains(&"api_key_secret".to_string()));
        assert!(columns.contains(&"provider_kind".to_string()));
    }
}
