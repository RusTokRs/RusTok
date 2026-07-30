use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

const UNIQUE_DEFAULT_INDEX: &str = "uq_channels_one_default_per_tenant";
const MYSQL_DEFAULT_GUARD_COLUMN: &str = "default_channel_guard";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        reject_existing_duplicate_defaults(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: removing this invariant would reopen a cross-replica
        // resolution ambiguity and allow multiple defaults for one tenant.
        Ok(())
    }
}

async fn reject_existing_duplicate_defaults(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    let duplicate = manager
        .get_connection()
        .query_one(Statement::from_string(
            backend,
            r#"
            SELECT tenant_id
            FROM channels
            WHERE is_default = TRUE
            GROUP BY tenant_id
            HAVING COUNT(*) > 1
            LIMIT 1
            "#
            .to_string(),
        ))
        .await?;

    if let Some(row) = duplicate {
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        return Err(DbErr::Custom(format!(
            "cannot enforce one default channel per tenant: tenant {tenant_id} has multiple default channels"
        )));
    }

    Ok(())
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            CREATE OR REPLACE FUNCTION channel_promote_single_default()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.is_default THEN
                    UPDATE channels
                       SET is_default = FALSE,
                           updated_at = CURRENT_TIMESTAMP
                     WHERE tenant_id = NEW.tenant_id
                       AND id <> NEW.id
                       AND is_default;
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channels_promote_single_default ON channels;
            CREATE TRIGGER channels_promote_single_default
            BEFORE INSERT OR UPDATE OF is_default, tenant_id ON channels
            FOR EACH ROW
            EXECUTE FUNCTION channel_promote_single_default();

            CREATE UNIQUE INDEX IF NOT EXISTS {UNIQUE_DEFAULT_INDEX}
                ON channels (tenant_id)
                WHERE is_default;
            "#
        ))
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            CREATE TRIGGER IF NOT EXISTS channels_promote_single_default_insert
            BEFORE INSERT ON channels
            WHEN NEW.is_default = 1
            BEGIN
                UPDATE channels
                   SET is_default = 0,
                       updated_at = CURRENT_TIMESTAMP
                 WHERE tenant_id = NEW.tenant_id
                   AND id <> NEW.id
                   AND is_default = 1;
            END;

            CREATE TRIGGER IF NOT EXISTS channels_promote_single_default_update
            BEFORE UPDATE OF is_default, tenant_id ON channels
            WHEN NEW.is_default = 1
            BEGIN
                UPDATE channels
                   SET is_default = 0,
                       updated_at = CURRENT_TIMESTAMP
                 WHERE tenant_id = NEW.tenant_id
                   AND id <> NEW.id
                   AND is_default = 1;
            END;

            CREATE UNIQUE INDEX IF NOT EXISTS {UNIQUE_DEFAULT_INDEX}
                ON channels (tenant_id)
                WHERE is_default = 1;
            "#
        ))
        .await?;
    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            ALTER TABLE channels
                ADD COLUMN {MYSQL_DEFAULT_GUARD_COLUMN} TINYINT
                    GENERATED ALWAYS AS (
                        CASE WHEN is_default THEN 1 ELSE NULL END
                    ) STORED,
                ADD UNIQUE INDEX {UNIQUE_DEFAULT_INDEX} (
                    tenant_id,
                    {MYSQL_DEFAULT_GUARD_COLUMN}
                );
            "#
        ))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    async fn sqlite_channels_schema() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared(
            r#"
            CREATE TABLE channels (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                is_default INTEGER NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .await
        .expect("channels table");
        db
    }

    async fn insert_channel(
        db: &sea_orm::DatabaseConnection,
        id: Uuid,
        tenant_id: Uuid,
        is_default: bool,
    ) -> Result<(), sea_orm::DbErr> {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id, is_default) VALUES (?1, ?2, ?3)",
            vec![id.into(), tenant_id.into(), is_default.into()],
        ))
        .await?;
        Ok(())
    }

    async fn default_count(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) -> i64 {
        db.query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM channels WHERE tenant_id = ?1 AND is_default = 1",
            vec![tenant_id.into()],
        ))
        .await
        .expect("default count query")
        .expect("default count row")
        .try_get("", "count")
        .expect("default count")
    }

    #[tokio::test]
    async fn sqlite_promotion_demotes_the_previous_default() {
        let db = sqlite_channels_schema().await;
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("single-default invariant");

        let tenant_id = Uuid::new_v4();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        insert_channel(&db, first_id, tenant_id, true)
            .await
            .expect("first default");
        insert_channel(&db, second_id, tenant_id, true)
            .await
            .expect("promoted default");

        assert_eq!(default_count(&db, tenant_id).await, 1);
        let promoted: i64 = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT is_default FROM channels WHERE id = ?1",
                vec![second_id.into()],
            ))
            .await
            .expect("promoted query")
            .expect("promoted row")
            .try_get("", "is_default")
            .expect("promoted flag");
        assert_eq!(promoted, 1);

        insert_channel(&db, Uuid::new_v4(), tenant_id, false)
            .await
            .expect("non-default channel");
        insert_channel(&db, Uuid::new_v4(), Uuid::new_v4(), true)
            .await
            .expect("other tenant default");
    }

    #[tokio::test]
    async fn sqlite_replay_preserves_the_single_default_invariant() {
        let db = sqlite_channels_schema().await;
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.expect("first migration pass");
        Migration.up(&manager).await.expect("replayed migration pass");

        let tenant_id = Uuid::new_v4();
        insert_channel(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("first default");
        insert_channel(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("promoted default");
        assert_eq!(default_count(&db, tenant_id).await, 1);
    }

    #[tokio::test]
    async fn migration_rejects_preexisting_duplicate_defaults() {
        let db = sqlite_channels_schema().await;
        let tenant_id = Uuid::new_v4();
        insert_channel(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("first historical default");
        insert_channel(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("second historical default");

        let error = Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect_err("duplicate defaults must block migration");
        assert!(error.to_string().contains(&tenant_id.to_string()));
    }
}
