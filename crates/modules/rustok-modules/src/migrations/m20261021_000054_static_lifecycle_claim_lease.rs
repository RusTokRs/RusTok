use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Records when the active static-lifecycle aggregate claim was taken so an
/// expired claim (crashed or never-released owner) can be CAS-reclaimed
/// instead of deadlocking the `(tenant, module)` aggregate forever. Existing
/// rows keep `NULL`, which the owner treats as an already-expired legacy
/// claim; every fresh claim writes the timestamp.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statement = match manager.get_database_backend() {
            DbBackend::Postgres => {
                "ALTER TABLE module_static_tenant_lifecycle \
                 ADD COLUMN active_claimed_at TIMESTAMPTZ NULL"
            }
            DbBackend::Sqlite => {
                "ALTER TABLE module_static_tenant_lifecycle \
                 ADD COLUMN active_claimed_at TEXT NULL"
            }
            backend => {
                return Err(DbErr::Migration(format!(
                    "static lifecycle claim lease migration does not support database backend {backend:?}"
                )));
            }
        };
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                manager.get_database_backend(),
                statement,
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                manager.get_database_backend(),
                "ALTER TABLE module_static_tenant_lifecycle DROP COLUMN active_claimed_at",
            ))
            .await?;
        Ok(())
    }
}
