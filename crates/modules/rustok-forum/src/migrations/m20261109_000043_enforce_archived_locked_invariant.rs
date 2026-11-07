use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

/// P1 3.3 ideal: `archived` topics must be locked. Currently `delete` does
/// `status='archived' + is_locked=true` but `archive` leaves `is_locked` as-was,
/// so an `archived` unlocked row is possible. Add CHECK to make the invariant
/// explicit at DB level (alternative to documenting orthogonality).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "archived locked invariant does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "archived locked invariant rollback does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            -- Make existing archived unlocked rows compliant before adding CHECK
            UPDATE forum_topics SET is_locked = TRUE WHERE status = 'archived' AND is_locked = FALSE;

            ALTER TABLE forum_topics
                ADD CONSTRAINT forum_topics_archived_implies_locked
                CHECK (status != 'archived' OR is_locked = TRUE);
            "#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let conn = manager.get_connection();
    // SQLite CHECK on ALTER TABLE is limited — recreate via validation trigger + backfill
    conn.execute_unprepared(
        "UPDATE forum_topics SET is_locked = 1 WHERE status = 'archived' AND is_locked = 0",
    )
    .await?;
    conn.execute_unprepared(
        r#"
        CREATE TRIGGER forum_topics_archived_locked_check
        BEFORE INSERT ON forum_topics
        FOR EACH ROW WHEN NEW.status = 'archived' AND NEW.is_locked = 0
        BEGIN
            SELECT RAISE(ABORT, 'archived forum topic must be locked');
        END
        "#,
    )
    .await?;
    conn.execute_unprepared(
        r#"
        CREATE TRIGGER forum_topics_archived_locked_update_check
        BEFORE UPDATE ON forum_topics
        FOR EACH ROW WHEN NEW.status = 'archived' AND NEW.is_locked = 0
        BEGIN
            SELECT RAISE(ABORT, 'archived forum topic must be locked');
        END
        "#,
    )
    .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            "ALTER TABLE forum_topics DROP CONSTRAINT IF EXISTS forum_topics_archived_implies_locked;",
        )
        .await?;
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let conn = manager.get_connection();
    conn.execute_unprepared("DROP TRIGGER IF EXISTS forum_topics_archived_locked_check")
        .await?;
    conn.execute_unprepared(
        "DROP TRIGGER IF EXISTS forum_topics_archived_locked_update_check",
    )
    .await?;
    Ok(())
}
