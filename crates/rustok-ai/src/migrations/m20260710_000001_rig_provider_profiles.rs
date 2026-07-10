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
                )))
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
