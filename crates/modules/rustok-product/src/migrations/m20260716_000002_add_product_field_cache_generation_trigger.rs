use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DbBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE: &str = "product_field_definitions";
const TRIGGER: &str = "flex_product_fd_cache_generation";
const GENERATION_TABLE: &str = "flex_field_definition_cache_generation";
const OPERATIONS: [&str; 3] = ["insert", "update", "delete"];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // This migration still supports installations with the shared cache generation.
        if !manager.has_table(GENERATION_TABLE).await? {
            return Ok(());
        }
        match manager.get_database_backend() {
            DbBackend::Postgres => {
                execute(
                    manager,
                    format!("DROP TRIGGER IF EXISTS {TRIGGER} ON {TABLE}"),
                )
                .await?;
                execute(
                    manager,
                    format!(
                        "CREATE TRIGGER {TRIGGER} AFTER INSERT OR UPDATE OR DELETE ON {TABLE} \
                         FOR EACH STATEMENT EXECUTE FUNCTION rustok_bump_flex_field_definition_cache_generation()"
                    ),
                )
                .await?;
            }
            backend => {
                for operation in OPERATIONS {
                    let trigger = format!("{TRIGGER}_{operation}");
                    let event = operation.to_ascii_uppercase();
                    let bump = format!(
                        "UPDATE {GENERATION_TABLE} SET generation = generation + 1, \
                         updated_at = CURRENT_TIMESTAMP WHERE id = 1"
                    );
                    let sql = if backend == DbBackend::Sqlite {
                        format!(
                            "CREATE TRIGGER IF NOT EXISTS {trigger} AFTER {event} ON {TABLE} BEGIN {bump}; END"
                        )
                    } else {
                        execute(manager, format!("DROP TRIGGER IF EXISTS {trigger}")).await?;
                        format!(
                            "CREATE TRIGGER {trigger} AFTER {event} ON {TABLE} FOR EACH ROW {bump}"
                        )
                    };
                    execute(manager, sql).await?;
                }
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table(GENERATION_TABLE).await? {
            return Ok(());
        }
        if manager.get_database_backend() == DbBackend::Postgres {
            execute(
                manager,
                format!("DROP TRIGGER IF EXISTS {TRIGGER} ON {TABLE}"),
            )
            .await?;
        } else {
            for operation in OPERATIONS {
                execute(
                    manager,
                    format!("DROP TRIGGER IF EXISTS {TRIGGER}_{operation}"),
                )
                .await?;
            }
        }
        Ok(())
    }
}

async fn execute(manager: &SchemaManager<'_>, sql: String) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_raw(Statement::from_string(manager.get_database_backend(), sql))
        .await?;
    Ok(())
}
