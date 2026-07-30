use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

const UNIQUE_ACTIVE_INDEX: &str = "uq_channel_policy_sets_one_active_per_tenant";
const MYSQL_ACTIVE_GUARD_COLUMN: &str = "active_policy_set_guard";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        reject_existing_duplicate_active_sets(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: multiple active policy sets make channel resolution
        // nondeterministic and must not be re-enabled by rollback.
        Ok(())
    }
}

async fn reject_existing_duplicate_active_sets(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let duplicate = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            r#"
            SELECT tenant_id
            FROM channel_resolution_policy_sets
            WHERE is_active = TRUE
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
            "cannot enforce one active channel policy set per tenant: tenant {tenant_id} has multiple active policy sets"
        )));
    }
    Ok(())
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            CREATE OR REPLACE FUNCTION channel_promote_single_active_policy_set()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.is_active THEN
                    UPDATE channel_resolution_policy_sets
                       SET is_active = FALSE,
                           updated_at = CURRENT_TIMESTAMP
                     WHERE tenant_id = NEW.tenant_id
                       AND id <> NEW.id
                       AND is_active;
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channel_policy_sets_promote_single_active
                ON channel_resolution_policy_sets;
            CREATE TRIGGER channel_policy_sets_promote_single_active
            BEFORE INSERT OR UPDATE OF is_active, tenant_id
                ON channel_resolution_policy_sets
            FOR EACH ROW
            EXECUTE FUNCTION channel_promote_single_active_policy_set();

            CREATE UNIQUE INDEX IF NOT EXISTS {UNIQUE_ACTIVE_INDEX}
                ON channel_resolution_policy_sets (tenant_id)
                WHERE is_active;
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
            CREATE TRIGGER IF NOT EXISTS channel_policy_sets_promote_single_active_insert
            BEFORE INSERT ON channel_resolution_policy_sets
            WHEN NEW.is_active = 1
            BEGIN
                UPDATE channel_resolution_policy_sets
                   SET is_active = 0,
                       updated_at = CURRENT_TIMESTAMP
                 WHERE tenant_id = NEW.tenant_id
                   AND id <> NEW.id
                   AND is_active = 1;
            END;

            CREATE TRIGGER IF NOT EXISTS channel_policy_sets_promote_single_active_update
            BEFORE UPDATE OF is_active, tenant_id ON channel_resolution_policy_sets
            WHEN NEW.is_active = 1
            BEGIN
                UPDATE channel_resolution_policy_sets
                   SET is_active = 0,
                       updated_at = CURRENT_TIMESTAMP
                 WHERE tenant_id = NEW.tenant_id
                   AND id <> NEW.id
                   AND is_active = 1;
            END;

            CREATE UNIQUE INDEX IF NOT EXISTS {UNIQUE_ACTIVE_INDEX}
                ON channel_resolution_policy_sets (tenant_id)
                WHERE is_active = 1;
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
            ALTER TABLE channel_resolution_policy_sets
                ADD COLUMN {MYSQL_ACTIVE_GUARD_COLUMN} TINYINT
                    GENERATED ALWAYS AS (
                        CASE WHEN is_active THEN 1 ELSE NULL END
                    ) STORED,
                ADD UNIQUE INDEX {UNIQUE_ACTIVE_INDEX} (
                    tenant_id,
                    {MYSQL_ACTIVE_GUARD_COLUMN}
                )
            "#
        ))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    async fn sqlite_policy_schema() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared(
            r#"
            CREATE TABLE channel_resolution_policy_sets (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL,
                is_active INTEGER NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )
        .await
        .expect("policy-set table");
        db
    }

    async fn insert_policy_set(
        db: &sea_orm::DatabaseConnection,
        id: Uuid,
        tenant_id: Uuid,
        is_active: bool,
    ) -> Result<(), sea_orm::DbErr> {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_resolution_policy_sets (id, tenant_id, is_active) VALUES (?1, ?2, ?3)",
            vec![id.into(), tenant_id.into(), is_active.into()],
        ))
        .await?;
        Ok(())
    }

    async fn active_count(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) -> i64 {
        db.query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM channel_resolution_policy_sets WHERE tenant_id = ?1 AND is_active = 1",
            vec![tenant_id.into()],
        ))
        .await
        .expect("active count query")
        .expect("active count row")
        .try_get("", "count")
        .expect("active count")
    }

    #[tokio::test]
    async fn sqlite_promotion_demotes_the_previous_active_policy_set() {
        let db = sqlite_policy_schema().await;
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("single-active invariant");

        let tenant_id = Uuid::new_v4();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        insert_policy_set(&db, first_id, tenant_id, true)
            .await
            .expect("first active set");
        insert_policy_set(&db, second_id, tenant_id, true)
            .await
            .expect("promoted active set");

        assert_eq!(active_count(&db, tenant_id).await, 1);
        let promoted: i64 = db
            .query_one(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT is_active FROM channel_resolution_policy_sets WHERE id = ?1",
                vec![second_id.into()],
            ))
            .await
            .expect("promoted query")
            .expect("promoted row")
            .try_get("", "is_active")
            .expect("promoted flag");
        assert_eq!(promoted, 1);

        insert_policy_set(&db, Uuid::new_v4(), tenant_id, false)
            .await
            .expect("inactive set");
        insert_policy_set(&db, Uuid::new_v4(), Uuid::new_v4(), true)
            .await
            .expect("other tenant active set");
    }

    #[tokio::test]
    async fn sqlite_replay_preserves_the_single_active_invariant() {
        let db = sqlite_policy_schema().await;
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.expect("first migration pass");
        Migration.up(&manager).await.expect("replayed migration pass");

        let tenant_id = Uuid::new_v4();
        insert_policy_set(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("first active set");
        insert_policy_set(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("promoted active set");
        assert_eq!(active_count(&db, tenant_id).await, 1);
    }

    #[tokio::test]
    async fn migration_rejects_preexisting_duplicate_active_sets() {
        let db = sqlite_policy_schema().await;
        let tenant_id = Uuid::new_v4();
        insert_policy_set(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("first historical active set");
        insert_policy_set(&db, Uuid::new_v4(), tenant_id, true)
            .await
            .expect("second historical active set");

        let error = Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect_err("duplicate active sets must block migration");
        assert!(error.to_string().contains(&tenant_id.to_string()));
    }
}
