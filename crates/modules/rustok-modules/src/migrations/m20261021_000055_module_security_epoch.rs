use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Durable, platform-wide security epoch (audit remediation D9/H-7).
///
/// The epoch preempts in-flight transitions after quarantine/revocation, so it
/// must be shared by every process and survive restarts. A singleton row
/// (`singleton_id = 1`) holds the current epoch, the reason for the latest
/// advancement, and when it happened. The bootstrap row (epoch 1) is seeded
/// here so readers never have to invent a value.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let (create_table, seed_row) = match manager.get_database_backend() {
            DbBackend::Postgres => (
                "CREATE TABLE module_security_epoch (\
                    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1), \
                    epoch BIGINT NOT NULL CHECK (epoch >= 1), \
                    last_reason TEXT NOT NULL, \
                    updated_at TIMESTAMPTZ NOT NULL)",
                "INSERT INTO module_security_epoch (singleton_id, epoch, last_reason, updated_at) \
                 VALUES (1, 1, 'Initial security epoch bootstrap', now()) \
                 ON CONFLICT (singleton_id) DO NOTHING",
            ),
            DbBackend::Sqlite => (
                "CREATE TABLE module_security_epoch (\
                    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1), \
                    epoch INTEGER NOT NULL CHECK (epoch >= 1), \
                    last_reason TEXT NOT NULL, \
                    updated_at TEXT NOT NULL)",
                "INSERT INTO module_security_epoch (singleton_id, epoch, last_reason, updated_at) \
                 VALUES (1, 1, 'Initial security epoch bootstrap', '1970-01-01T00:00:00.000000+00:00') \
                 ON CONFLICT (singleton_id) DO NOTHING",
            ),
            backend => {
                return Err(DbErr::Migration(format!(
                    "module security epoch migration does not support database backend {backend:?}"
                )));
            }
        };
        for statement in [create_table, seed_row] {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    statement,
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_raw(Statement::from_string(
                manager.get_database_backend(),
                "DROP TABLE module_security_epoch",
            ))
            .await?;
        Ok(())
    }
}
