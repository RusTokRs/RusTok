use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

const CHANNEL_RESOLUTION_SCOPE: &str = "resolution";
const CHANNEL_RESOLUTION_TABLES: &[&str] = &[
    "channels",
    "channel_targets",
    "channel_module_bindings",
    "channel_oauth_apps",
    "channel_resolution_policy_sets",
    "channel_resolution_policy_rules",
];

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ChannelResolutionInvalidationState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ChannelResolutionInvalidationState::Scope)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionInvalidationState::Generation)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ChannelResolutionInvalidationState::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(&format!(
                "INSERT INTO channel_resolution_invalidation_state (scope, generation, updated_at) \
                 SELECT '{CHANNEL_RESOLUTION_SCOPE}', 0, CURRENT_TIMESTAMP \
                 WHERE NOT EXISTS ( \
                     SELECT 1 FROM channel_resolution_invalidation_state \
                     WHERE scope = '{CHANNEL_RESOLUTION_SCOPE}' \
                 )"
            ))
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_triggers(manager).await,
            DatabaseBackend::Sqlite => install_sqlite_triggers(manager).await,
            backend => Err(DbErr::Custom(format!(
                "channel resolution invalidation migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => remove_postgres_triggers(manager).await?,
            DatabaseBackend::Sqlite => remove_sqlite_triggers(manager).await?,
            backend => {
                return Err(DbErr::Custom(format!(
                    "channel resolution invalidation migration does not support {backend:?}"
                )));
            }
        }

        manager
            .drop_table(
                Table::drop()
                    .table(ChannelResolutionInvalidationState::Table)
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION channel_bump_resolution_invalidation_generation()
RETURNS trigger AS $$
BEGIN
    UPDATE channel_resolution_invalidation_state
       SET generation = generation + 1,
           updated_at = CURRENT_TIMESTAMP
     WHERE scope = 'resolution';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;
"#,
        )
        .await?;

    for table in CHANNEL_RESOLUTION_TABLES {
        let trigger = postgres_trigger_name(table);
        connection
            .execute_unprepared(&format!(
                "DROP TRIGGER IF EXISTS {trigger} ON {table}; \
                 CREATE TRIGGER {trigger} \
                 AFTER INSERT OR UPDATE OR DELETE ON {table} \
                 FOR EACH STATEMENT \
                 EXECUTE FUNCTION channel_bump_resolution_invalidation_generation();"
            ))
            .await?;
    }
    Ok(())
}

async fn remove_postgres_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for table in CHANNEL_RESOLUTION_TABLES {
        connection
            .execute_unprepared(&format!(
                "DROP TRIGGER IF EXISTS {} ON {table}",
                postgres_trigger_name(table)
            ))
            .await?;
    }
    connection
        .execute_unprepared(
            "DROP FUNCTION IF EXISTS channel_bump_resolution_invalidation_generation()",
        )
        .await?;
    Ok(())
}

async fn install_sqlite_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for table in CHANNEL_RESOLUTION_TABLES {
        for (event_name, event_sql) in [
            ("insert", "INSERT"),
            ("update", "UPDATE"),
            ("delete", "DELETE"),
        ] {
            let trigger = sqlite_trigger_name(table, event_name);
            connection
                .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {trigger}"))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {trigger} \
                     AFTER {event_sql} ON {table} \
                     BEGIN \
                         UPDATE channel_resolution_invalidation_state \
                            SET generation = generation + 1, \
                                updated_at = CURRENT_TIMESTAMP \
                          WHERE scope = 'resolution'; \
                     END"
                ))
                .await?;
        }
    }
    Ok(())
}

async fn remove_sqlite_triggers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for table in CHANNEL_RESOLUTION_TABLES {
        for event_name in ["insert", "update", "delete"] {
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {}",
                    sqlite_trigger_name(table, event_name)
                ))
                .await?;
        }
    }
    Ok(())
}

fn postgres_trigger_name(table: &str) -> String {
    format!("{table}_resolution_invalidation_generation")
}

fn sqlite_trigger_name(table: &str, event: &str) -> String {
    format!("{table}_resolution_invalidation_generation_{event}")
}

#[derive(Iden)]
enum ChannelResolutionInvalidationState {
    Table,
    Scope,
    Generation,
    UpdatedAt,
}

#[cfg(test)]
mod tests {
    use super::{CHANNEL_RESOLUTION_TABLES, Migration};
    use sea_orm_migration::MigrationTrait;
    use sea_orm_migration::prelude::SchemaManager;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    async fn generation(db: &sea_orm_migration::sea_orm::DatabaseConnection) -> i64 {
        db.query_one(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT generation FROM channel_resolution_invalidation_state WHERE scope = 'resolution'"
                .to_string(),
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "generation")
        .unwrap()
    }

    #[tokio::test]
    async fn sqlite_triggers_advance_generation_and_replay_preserves_it() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for table in CHANNEL_RESOLUTION_TABLES {
            db.execute_unprepared(&format!(
                "CREATE TABLE {table} (id INTEGER PRIMARY KEY, value TEXT)"
            ))
            .await
            .unwrap();
        }

        let manager = SchemaManager::new(&db);
        let migration = Migration;
        migration.up(&manager).await.unwrap();
        assert_eq!(generation(&db).await, 0);

        db.execute_unprepared("INSERT INTO channels (id, value) VALUES (1, 'a')")
            .await
            .unwrap();
        db.execute_unprepared("UPDATE channels SET value = 'b' WHERE id = 1")
            .await
            .unwrap();
        db.execute_unprepared("DELETE FROM channels WHERE id = 1")
            .await
            .unwrap();
        db.execute_unprepared(
            "INSERT INTO channel_resolution_policy_rules (id, value) VALUES (1, 'rule')",
        )
        .await
        .unwrap();
        assert_eq!(generation(&db).await, 4);

        migration.up(&manager).await.unwrap();
        assert_eq!(generation(&db).await, 4);
        db.execute_unprepared("INSERT INTO channel_targets (id, value) VALUES (1, 'target')")
            .await
            .unwrap();
        assert_eq!(generation(&db).await, 5);
    }
}
