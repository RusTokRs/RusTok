use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Records when the active static-lifecycle aggregate claim was taken so an
/// expired claim (crashed or never-released owner) can be CAS-reclaimed
/// instead of deadlocking the `(tenant, module)` aggregate forever. Existing
/// rows keep `NULL`, which the owner treats as an already-expired legacy
/// claim; every fresh claim writes the timestamp.
///
/// `module_static_tenant_lifecycle` itself is owned by `rustok-migrations`
/// (`m20250101_000003_create_tenant_modules`). In the production migration
/// stack all migrations are sorted together by name, so the table exists by
/// the time this column is added. Isolated crate-only test compositions may
/// legitimately lack the table; the alteration is then skipped instead of
/// failing, and the column is added whenever the full stack runs. The guard
/// also makes the migration idempotent for reruns and partial applies.
#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE: &str = "module_static_tenant_lifecycle";
const COLUMN: &str = "active_claimed_at";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        match manager.get_database_backend() {
            DbBackend::Postgres => {
                connection
                    .execute_raw(Statement::from_string(
                        DbBackend::Postgres,
                        format!(
                            "ALTER TABLE IF EXISTS {TABLE} \
                             ADD COLUMN IF NOT EXISTS {COLUMN} TIMESTAMPTZ NULL"
                        ),
                    ))
                    .await?;
            }
            DbBackend::Sqlite => {
                if !sqlite_column_exists(connection, TABLE, COLUMN).await?
                    && sqlite_table_exists(connection, TABLE).await?
                {
                    connection
                        .execute_raw(Statement::from_string(
                            DbBackend::Sqlite,
                            format!("ALTER TABLE {TABLE} ADD COLUMN {COLUMN} TEXT NULL"),
                        ))
                        .await?;
                }
            }
            backend => {
                return Err(DbErr::Migration(format!(
                    "static lifecycle claim lease migration does not support database backend {backend:?}"
                )));
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        match manager.get_database_backend() {
            DbBackend::Postgres => {
                connection
                    .execute_raw(Statement::from_string(
                        DbBackend::Postgres,
                        format!("ALTER TABLE IF EXISTS {TABLE} DROP COLUMN IF EXISTS {COLUMN}"),
                    ))
                    .await?;
            }
            DbBackend::Sqlite => {
                if sqlite_column_exists(connection, TABLE, COLUMN).await? {
                    connection
                        .execute_raw(Statement::from_string(
                            DbBackend::Sqlite,
                            format!("ALTER TABLE {TABLE} DROP COLUMN {COLUMN}"),
                        ))
                        .await?;
                }
            }
            backend => {
                return Err(DbErr::Migration(format!(
                    "static lifecycle claim lease migration does not support database backend {backend:?}"
                )));
            }
        }
        Ok(())
    }
}

async fn sqlite_table_exists<C: ConnectionTrait>(connection: &C, table: &str) -> Result<bool, DbErr> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table.into()],
        ))
        .await?;
    Ok(row.is_some())
}

async fn sqlite_column_exists<C: ConnectionTrait>(
    connection: &C,
    table: &str,
    column: &str,
) -> Result<bool, DbErr> {
    // `table` is a fixed migration-owned constant, never caller input:
    // PRAGMA does not accept bound parameters for the table name.
    let rows = connection
        .query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA table_info({table})"),
        ))
        .await?;
    rows.iter()
        .map(|row| row.try_get::<String>("", "name"))
        .collect::<Result<Vec<_>, _>>()
        .map(|names| names.iter().any(|name| name == column))
}
