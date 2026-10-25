use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Renames the stored transition checkpoint state formerly called
/// `FailedClosed` to `InterventionRequired`.
///
/// The old name claimed a serving consequence the watchdog never performed.
/// The state only stops automatic transition action; candidate serving
/// selection remains owned by the installation store.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statement = match backend {
            SupportedBackend::Postgres => {
                "UPDATE module_transition_checkpoints \
                 SET state = jsonb_set(state, '{state}', '\"InterventionRequired\"'::jsonb, false) \
                 WHERE state->>'state' = 'FailedClosed'"
            }
            SupportedBackend::Sqlite => {
                "UPDATE module_transition_checkpoints \
                 SET state = json_replace(state, '$.state', 'InterventionRequired') \
                 WHERE json_extract(state, '$.state') = 'FailedClosed'"
            }
        };
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                statement.to_string(),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statement = match backend {
            SupportedBackend::Postgres => {
                "UPDATE module_transition_checkpoints \
                 SET state = jsonb_set(state, '{state}', '\"FailedClosed\"'::jsonb, false) \
                 WHERE state->>'state' = 'InterventionRequired'"
            }
            SupportedBackend::Sqlite => {
                "UPDATE module_transition_checkpoints \
                 SET state = json_replace(state, '$.state', 'FailedClosed') \
                 WHERE json_extract(state, '$.state') = 'InterventionRequired'"
            }
        };
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                backend.db_backend(),
                statement.to_string(),
            ))
            .await?;
        Ok(())
    }
}
