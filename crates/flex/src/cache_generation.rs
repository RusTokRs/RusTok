use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DbBackend};

pub const FIELD_DEFINITION_CACHE_GENERATION_TABLE: &str =
    "flex_field_definition_cache_generation";
const FIELD_DEFINITION_CACHE_GENERATION_ID: i32 = 1;
const POSTGRES_BUMP_FUNCTION: &str = "rustok_bump_flex_field_definition_cache_generation";

/// Create the singleton durable generation used by every attached field-definition owner.
pub async fn create_field_definition_cache_generation_table(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(Alias::new(FIELD_DEFINITION_CACHE_GENERATION_TABLE))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(Alias::new("generation"))
                        .big_integer()
                        .not_null()
                        .default(0_i64),
                )
                .col(
                    ColumnDef::new(Alias::new("updated_at"))
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                )
                .to_owned(),
        )
        .await?;

    let connection = manager.get_connection();
    let seed = match connection.get_database_backend() {
        DbBackend::Postgres | DbBackend::Sqlite => format!(
            "INSERT INTO {FIELD_DEFINITION_CACHE_GENERATION_TABLE} (id, generation) \
             VALUES ({FIELD_DEFINITION_CACHE_GENERATION_ID}, 0) \
             ON CONFLICT (id) DO NOTHING"
        ),
        DbBackend::MySql => format!(
            "INSERT IGNORE INTO {FIELD_DEFINITION_CACHE_GENERATION_TABLE} (id, generation) \
             VALUES ({FIELD_DEFINITION_CACHE_GENERATION_ID}, 0)"
        ),
    };
    connection.execute_unprepared(&seed).await?;

    if connection.get_database_backend() == DbBackend::Postgres {
        connection
            .execute_unprepared(&format!(
                "CREATE OR REPLACE FUNCTION {POSTGRES_BUMP_FUNCTION}() RETURNS trigger AS $$ \
                 BEGIN \
                   UPDATE {FIELD_DEFINITION_CACHE_GENERATION_TABLE} \
                   SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP \
                   WHERE id = {FIELD_DEFINITION_CACHE_GENERATION_ID}; \
                   RETURN NULL; \
                 END; \
                 $$ LANGUAGE plpgsql"
            ))
            .await?;
    }

    Ok(())
}

/// Install an atomic generation bump for every mutation of one owner table.
pub async fn create_field_definition_cache_generation_trigger(
    manager: &SchemaManager<'_>,
    table_name: &str,
    trigger_name: &str,
) -> Result<(), DbErr> {
    validate_identifier(table_name)?;
    validate_identifier(trigger_name)?;

    let connection = manager.get_connection();
    match connection.get_database_backend() {
        DbBackend::Postgres => {
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {trigger_name} ON {table_name}"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {trigger_name} \
                     AFTER INSERT OR UPDATE OR DELETE ON {table_name} \
                     FOR EACH STATEMENT EXECUTE FUNCTION {POSTGRES_BUMP_FUNCTION}()"
                ))
                .await?;
        }
        DbBackend::Sqlite => {
            for operation in ["insert", "update", "delete"] {
                let sqlite_trigger = format!("{trigger_name}_{operation}");
                connection
                    .execute_unprepared(&format!(
                        "CREATE TRIGGER IF NOT EXISTS {sqlite_trigger} \
                         AFTER {} ON {table_name} \
                         BEGIN \
                           UPDATE {FIELD_DEFINITION_CACHE_GENERATION_TABLE} \
                           SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP \
                           WHERE id = {FIELD_DEFINITION_CACHE_GENERATION_ID}; \
                         END",
                        operation.to_ascii_uppercase()
                    ))
                    .await?;
            }
        }
        DbBackend::MySql => {
            for operation in ["insert", "update", "delete"] {
                let mysql_trigger = format!("{trigger_name}_{operation}");
                connection
                    .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {mysql_trigger}"))
                    .await?;
                connection
                    .execute_unprepared(&format!(
                        "CREATE TRIGGER {mysql_trigger} AFTER {} ON {table_name} \
                         FOR EACH ROW \
                         UPDATE {FIELD_DEFINITION_CACHE_GENERATION_TABLE} \
                         SET generation = generation + 1, updated_at = CURRENT_TIMESTAMP \
                         WHERE id = {FIELD_DEFINITION_CACHE_GENERATION_ID}",
                        operation.to_ascii_uppercase()
                    ))
                    .await?;
            }
        }
    }

    Ok(())
}

pub async fn drop_field_definition_cache_generation_trigger(
    manager: &SchemaManager<'_>,
    table_name: &str,
    trigger_name: &str,
) -> Result<(), DbErr> {
    validate_identifier(table_name)?;
    validate_identifier(trigger_name)?;

    let connection = manager.get_connection();
    match connection.get_database_backend() {
        DbBackend::Postgres => {
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {trigger_name} ON {table_name}"
                ))
                .await?;
        }
        DbBackend::Sqlite | DbBackend::MySql => {
            for operation in ["insert", "update", "delete"] {
                connection
                    .execute_unprepared(&format!(
                        "DROP TRIGGER IF EXISTS {trigger_name}_{operation}"
                    ))
                    .await?;
            }
        }
    }

    Ok(())
}

pub async fn drop_field_definition_cache_generation_table(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    if connection.get_database_backend() == DbBackend::Postgres {
        connection
            .execute_unprepared(&format!(
                "DROP FUNCTION IF EXISTS {POSTGRES_BUMP_FUNCTION}()"
            ))
            .await?;
    }

    manager
        .drop_table(
            Table::drop()
                .table(Alias::new(FIELD_DEFINITION_CACHE_GENERATION_TABLE))
                .if_exists()
                .to_owned(),
        )
        .await
}

fn validate_identifier(value: &str) -> Result<(), DbErr> {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Ok(());
    }

    Err(DbErr::Custom(format!(
        "unsafe field-definition cache generation identifier: {value}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{
        Database, DatabaseConnection, Statement, TransactionTrait,
    };

    const OWNERS: [(&str, &str); 4] = [
        ("user_field_definitions", "flex_user_fd_cache_generation"),
        (
            "product_field_definitions",
            "flex_product_fd_cache_generation",
        ),
        ("order_field_definitions", "flex_order_fd_cache_generation"),
        ("topic_field_definitions", "flex_topic_fd_cache_generation"),
    ];

    async fn read_generation(db: &DatabaseConnection) -> u64 {
        let row = db
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                format!(
                    "SELECT generation FROM {FIELD_DEFINITION_CACHE_GENERATION_TABLE} WHERE id = 1"
                ),
            ))
            .await
            .expect("generation query should succeed")
            .expect("generation singleton should exist");
        let generation: i64 = row
            .try_get("", "generation")
            .expect("generation should decode");
        std::convert::TryFrom::try_from(generation).expect("generation should remain non-negative")
    }

    async fn create_owner_tables(db: &DatabaseConnection) {
        for (table, _) in OWNERS {
            db.execute_unprepared(&format!(
                "CREATE TABLE {table} (id INTEGER PRIMARY KEY, position INTEGER NOT NULL DEFAULT 0, is_active INTEGER NOT NULL DEFAULT 1)"
            ))
            .await
            .expect("owner table should create");
        }
    }

    #[test]
    fn generation_trigger_identifiers_are_strictly_bounded_to_sql_names() {
        assert!(validate_identifier("user_field_definitions").is_ok());
        assert!(validate_identifier("flex_user_fd_generation").is_ok());
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("user; DROP TABLE users").is_err());
    }

    #[tokio::test]
    async fn sqlite_all_owner_mutations_are_transactional_and_replay_safe() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite Flex fixture should connect");
        create_owner_tables(&db).await;
        let manager = SchemaManager::new(&db);
        create_field_definition_cache_generation_table(&manager)
            .await
            .expect("generation table should create");
        for (table, trigger) in OWNERS {
            create_field_definition_cache_generation_trigger(&manager, table, trigger)
                .await
                .expect("owner trigger should create");
        }
        assert_eq!(read_generation(&db).await, 0);

        for (index, (table, _)) in OWNERS.into_iter().enumerate() {
            db.execute_unprepared(&format!(
                "INSERT INTO {table} (id, position, is_active) VALUES ({}, 0, 1)",
                index + 1
            ))
            .await
            .expect("owner insert should commit");
        }
        assert_eq!(read_generation(&db).await, 4);

        for (table, _) in OWNERS {
            db.execute_unprepared(&format!(
                "UPDATE {table} SET position = position + 1"
            ))
            .await
            .expect("owner reorder should commit");
        }
        assert_eq!(read_generation(&db).await, 8);

        for (table, _) in OWNERS {
            db.execute_unprepared(&format!(
                "UPDATE {table} SET is_active = 0"
            ))
            .await
            .expect("owner soft delete should commit");
        }
        assert_eq!(read_generation(&db).await, 12);

        let transaction = db.begin().await.expect("rollback transaction should begin");
        transaction
            .execute_unprepared(
                "INSERT INTO user_field_definitions (id, position, is_active) VALUES (99, 0, 1)",
            )
            .await
            .expect("rolled-back owner mutation should execute");
        transaction
            .rollback()
            .await
            .expect("owner mutation should roll back");
        assert_eq!(read_generation(&db).await, 12);

        for (table, _) in OWNERS {
            db.execute_unprepared(&format!("DELETE FROM {table}"))
                .await
                .expect("owner delete should commit");
        }
        assert_eq!(read_generation(&db).await, 16);

        create_field_definition_cache_generation_table(&manager)
            .await
            .expect("generation table replay should succeed");
        for (table, trigger) in OWNERS {
            create_field_definition_cache_generation_trigger(&manager, table, trigger)
                .await
                .expect("owner trigger replay should succeed");
        }
        assert_eq!(read_generation(&db).await, 16);
    }
}
