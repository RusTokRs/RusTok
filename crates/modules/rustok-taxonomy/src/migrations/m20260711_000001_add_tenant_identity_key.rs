use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE taxonomy_terms ADD CONSTRAINT uq_taxonomy_terms_tenant_id UNIQUE (tenant_id, id);",
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .create_index(
                        Index::create()
                            .name("uq_taxonomy_terms_tenant_id")
                            .table(Alias::new("taxonomy_terms"))
                            .col(Alias::new("tenant_id"))
                            .col(Alias::new("id"))
                            .unique()
                            .to_owned(),
                    )
                    .await?;
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "taxonomy tenant identity migration does not support {backend:?}"
                )));
            }
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // The tenant identity key is required by consumer composite foreign keys.
        Ok(())
    }
}
