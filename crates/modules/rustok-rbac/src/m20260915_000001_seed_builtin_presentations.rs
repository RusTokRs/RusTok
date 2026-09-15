use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

use crate::presentation_catalog::seed_builtin_presentations_on;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = connection.get_database_backend();
        let tenants = connection
            .query_all(Statement::from_string(
                backend,
                "SELECT id FROM tenants".to_string(),
            ))
            .await?;

        for tenant in tenants {
            let tenant_id = tenant.try_get::<Uuid>("", "id")?;
            seed_builtin_presentations_on(connection, tenant_id)
                .await
                .map_err(|error| DbErr::Custom(error.to_string()))?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Seed rows become canonical owner data and may be edited after rollout.
        // A downgrade must not guess which rows are still pristine seed copy and
        // which now contain tenant-authored presentation.
        Ok(())
    }
}
