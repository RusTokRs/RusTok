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
            DbBackend::Postgres => "SELECT slug FROM ai_provider_profiles \
                 WHERE COALESCE(settings->>'base_url', '') <> '' ORDER BY slug",
            DbBackend::Sqlite => "SELECT slug FROM ai_provider_profiles \
                 WHERE COALESCE(json_extract(settings, '$.base_url'), '') <> '' ORDER BY slug",
            other => {
                return Err(DbErr::Migration(format!(
                    "AI provider target migration does not support database backend {other:?}"
                )))
            }
        };
        let rows = connection
            .query_all(Statement::from_string(backend, custom_endpoint_query.to_string()))
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
                "ALTER TABLE ai_provider_profiles ADD COLUMN provider_target_id TEXT",
                "UPDATE ai_provider_profiles SET provider_target_id = provider_slug",
                "ALTER TABLE ai_provider_profiles DROP COLUMN settings",
            ],
            other => {
                return Err(DbErr::Migration(format!(
                    "AI provider target migration does not support database backend {other:?}"
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
            "AI provider targets are intentionally irreversible because endpoints are deployment-owned"
                .to_string(),
        ))
    }
}
