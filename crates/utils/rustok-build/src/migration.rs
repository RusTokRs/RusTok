//! Owner migration for durable, globally ordered build-progress snapshots.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

const BUILD_PROGRESS_INDEX: &str = "idx_builds_event_sequence";

#[derive(Iden)]
enum BuildEventClock {
    Table,
    Id,
    Sequence,
}

#[derive(Iden)]
enum Builds {
    Table,
    EventSequence,
}

/// Add the singleton sequence clock and per-build latest-state watermark.
pub async fn create_build_progress_sequence(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let create_clock = match manager.get_database_backend() {
        DatabaseBackend::Postgres => {
            "CREATE TABLE build_event_clock (\
             id SMALLINT PRIMARY KEY CHECK (id = 1), \
             sequence BIGINT NOT NULL CHECK (sequence >= 0))"
        }
        DatabaseBackend::Sqlite => {
            "CREATE TABLE build_event_clock (\
             id INTEGER PRIMARY KEY CHECK (id = 1), \
             sequence INTEGER NOT NULL CHECK (typeof(sequence) = 'integer' AND sequence >= 0))"
        }
        _ => {
            return Err(DbErr::Migration(
                "durable build progress requires PostgreSQL or SQLite".into(),
            ));
        }
    };

    manager
        .get_connection()
        .execute_unprepared(create_clock)
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            "INSERT INTO build_event_clock (id, sequence) VALUES (1, 0)",
        )
        .await?;

    manager
        .alter_table(
            Table::alter()
                .table(Builds::Table)
                .add_column(
                    ColumnDef::new(Builds::EventSequence)
                        .big_integer()
                        .not_null()
                        .default(0),
                )
                .to_owned(),
        )
        .await?;

    manager
        .create_index(
            Index::create()
                .name(BUILD_PROGRESS_INDEX)
                .table(Builds::Table)
                .col(Builds::EventSequence)
                .to_owned(),
        )
        .await?;

    Ok(())
}

/// Remove only the progress watermark; build history and build rows remain.
pub async fn drop_build_progress_sequence(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .drop_index(
            Index::drop()
                .name(BUILD_PROGRESS_INDEX)
                .table(Builds::Table)
                .to_owned(),
        )
        .await?;
    manager
        .alter_table(
            Table::alter()
                .table(Builds::Table)
                .drop_column(Builds::EventSequence)
                .to_owned(),
        )
        .await?;
    manager
        .drop_table(Table::drop().table(BuildEventClock::Table).to_owned())
        .await
}
