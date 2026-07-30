use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

const HOST_TARGET_TYPE: &str = "web_domain";
const PRIMARY_TARGET_INDEX: &str = "uq_channel_targets_one_primary";
const HOST_CLAIM_INDEX: &str = "uq_channel_host_target_claims_tenant_value";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        reject_existing_duplicate_host_claims(manager).await?;
        reject_existing_duplicate_primary_targets(manager).await?;
        create_claim_table(manager).await?;
        rebuild_claims(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => {
                return Err(DbErr::Custom(
                    "channel target integrity migration does not support MySQL; channel durable generation already requires PostgreSQL or SQLite"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: removing claim serialization or primary uniqueness would
        // reintroduce nondeterministic channel selection under concurrent writes.
        Ok(())
    }
}

async fn reject_existing_duplicate_host_claims(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let duplicate = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            format!(
                r#"
                SELECT channel.tenant_id, target.value
                FROM channel_targets target
                JOIN channels channel ON channel.id = target.channel_id
                WHERE target.target_type = '{HOST_TARGET_TYPE}'
                GROUP BY channel.tenant_id, target.value
                HAVING COUNT(*) > 1
                LIMIT 1
                "#
            ),
        ))
        .await?;

    if let Some(row) = duplicate {
        let tenant_id: Uuid = row.try_get("", "tenant_id")?;
        let value: String = row.try_get("", "value")?;
        return Err(DbErr::Custom(format!(
            "cannot enforce unique channel host target claims: tenant {tenant_id} has duplicate host `{value}`"
        )));
    }
    Ok(())
}

async fn reject_existing_duplicate_primary_targets(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let duplicate = manager
        .get_connection()
        .query_one(Statement::from_string(
            manager.get_database_backend(),
            r#"
            SELECT channel_id
            FROM channel_targets
            WHERE is_primary = TRUE
            GROUP BY channel_id
            HAVING COUNT(*) > 1
            LIMIT 1
            "#
            .to_string(),
        ))
        .await?;

    if let Some(row) = duplicate {
        let channel_id: Uuid = row.try_get("", "channel_id")?;
        return Err(DbErr::Custom(format!(
            "cannot enforce one primary channel target: channel {channel_id} has multiple primary targets"
        )));
    }
    Ok(())
}

async fn create_claim_table(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .create_table(
            Table::create()
                .table(ChannelHostTargetClaims::Table)
                .if_not_exists()
                .col(
                    ColumnDef::new(ChannelHostTargetClaims::TargetId)
                        .uuid()
                        .not_null()
                        .primary_key(),
                )
                .col(
                    ColumnDef::new(ChannelHostTargetClaims::TenantId)
                        .uuid()
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ChannelHostTargetClaims::TargetType)
                        .string_len(50)
                        .not_null(),
                )
                .col(
                    ColumnDef::new(ChannelHostTargetClaims::Value)
                        .string_len(500)
                        .not_null(),
                )
                .foreign_key(
                    ForeignKey::create()
                        .name("fk_channel_host_target_claims_target")
                        .from(
                            ChannelHostTargetClaims::Table,
                            ChannelHostTargetClaims::TargetId,
                        )
                        .to(ChannelTargets::Table, ChannelTargets::Id)
                        .on_update(ForeignKeyAction::Cascade)
                        .on_delete(ForeignKeyAction::Cascade),
                )
                .to_owned(),
        )
        .await?;

    let sql = match manager.get_database_backend() {
        DatabaseBackend::Postgres => format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS {HOST_CLAIM_INDEX} ON channel_host_target_claims (tenant_id, target_type, value)"
        ),
        DatabaseBackend::Sqlite => format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS {HOST_CLAIM_INDEX} ON channel_host_target_claims (tenant_id, target_type, value)"
        ),
        DatabaseBackend::MySql => {
            return Err(DbErr::Custom(
                "channel target claims do not support MySQL".to_string(),
            ));
        }
    };
    manager.get_connection().execute_unprepared(&sql).await?;
    Ok(())
}

async fn rebuild_claims(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    connection
        .execute_unprepared("DELETE FROM channel_host_target_claims")
        .await?;
    connection
        .execute_unprepared(&format!(
            r#"
            INSERT INTO channel_host_target_claims (target_id, tenant_id, target_type, value)
            SELECT target.id, channel.tenant_id, target.target_type, target.value
            FROM channel_targets target
            JOIN channels channel ON channel.id = target.channel_id
            WHERE target.target_type = '{HOST_TARGET_TYPE}'
            "#
        ))
        .await?;
    Ok(())
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            CREATE OR REPLACE FUNCTION channel_promote_single_primary_target()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.is_primary THEN
                    UPDATE channel_targets
                       SET is_primary = FALSE,
                           updated_at = CURRENT_TIMESTAMP
                     WHERE channel_id = NEW.channel_id
                       AND id <> NEW.id
                       AND is_primary;
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channel_targets_promote_single_primary ON channel_targets;
            CREATE TRIGGER channel_targets_promote_single_primary
            BEFORE INSERT OR UPDATE OF is_primary, channel_id ON channel_targets
            FOR EACH ROW EXECUTE FUNCTION channel_promote_single_primary_target();

            CREATE UNIQUE INDEX IF NOT EXISTS {PRIMARY_TARGET_INDEX}
                ON channel_targets (channel_id)
                WHERE is_primary;

            CREATE OR REPLACE FUNCTION channel_sync_host_target_claim()
            RETURNS trigger AS $$
            DECLARE
                target_tenant UUID;
            BEGIN
                DELETE FROM channel_host_target_claims WHERE target_id = NEW.id;
                IF NEW.target_type = '{HOST_TARGET_TYPE}' THEN
                    SELECT tenant_id INTO target_tenant FROM channels WHERE id = NEW.channel_id;
                    INSERT INTO channel_host_target_claims
                        (target_id, tenant_id, target_type, value)
                    VALUES (NEW.id, target_tenant, NEW.target_type, NEW.value);
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channel_targets_host_claim_guard ON channel_targets;
            CREATE TRIGGER channel_targets_host_claim_guard
            AFTER INSERT OR UPDATE OF channel_id, target_type, value ON channel_targets
            FOR EACH ROW EXECUTE FUNCTION channel_sync_host_target_claim();

            CREATE OR REPLACE FUNCTION channel_delete_host_target_claim()
            RETURNS trigger AS $$
            BEGIN
                DELETE FROM channel_host_target_claims WHERE target_id = OLD.id;
                RETURN OLD;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channel_targets_host_claim_delete ON channel_targets;
            CREATE TRIGGER channel_targets_host_claim_delete
            AFTER DELETE ON channel_targets
            FOR EACH ROW EXECUTE FUNCTION channel_delete_host_target_claim();

            CREATE OR REPLACE FUNCTION channel_move_host_target_claims()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id THEN
                    UPDATE channel_host_target_claims claim
                       SET tenant_id = NEW.tenant_id
                      FROM channel_targets target
                     WHERE target.channel_id = OLD.id
                       AND claim.target_id = target.id;
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channels_host_claim_tenant_move ON channels;
            CREATE TRIGGER channels_host_claim_tenant_move
            AFTER UPDATE OF tenant_id ON channels
            FOR EACH ROW EXECUTE FUNCTION channel_move_host_target_claims();
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
            CREATE TRIGGER IF NOT EXISTS channel_targets_promote_single_primary_insert
            BEFORE INSERT ON channel_targets
            WHEN NEW.is_primary = 1
            BEGIN
                UPDATE channel_targets
                   SET is_primary = 0,
                       updated_at = CURRENT_TIMESTAMP
                 WHERE channel_id = NEW.channel_id
                   AND id <> NEW.id
                   AND is_primary = 1;
            END;

            CREATE TRIGGER IF NOT EXISTS channel_targets_promote_single_primary_update
            BEFORE UPDATE OF is_primary, channel_id ON channel_targets
            WHEN NEW.is_primary = 1
            BEGIN
                UPDATE channel_targets
                   SET is_primary = 0,
                       updated_at = CURRENT_TIMESTAMP
                 WHERE channel_id = NEW.channel_id
                   AND id <> NEW.id
                   AND is_primary = 1;
            END;

            CREATE UNIQUE INDEX IF NOT EXISTS {PRIMARY_TARGET_INDEX}
                ON channel_targets (channel_id)
                WHERE is_primary = 1;

            DROP TRIGGER IF EXISTS channel_targets_host_claim_guard_insert;
            CREATE TRIGGER channel_targets_host_claim_guard_insert
            AFTER INSERT ON channel_targets
            WHEN NEW.target_type = '{HOST_TARGET_TYPE}'
            BEGIN
                INSERT INTO channel_host_target_claims
                    (target_id, tenant_id, target_type, value)
                VALUES (
                    NEW.id,
                    (SELECT tenant_id FROM channels WHERE id = NEW.channel_id),
                    NEW.target_type,
                    NEW.value
                );
            END;

            DROP TRIGGER IF EXISTS channel_targets_host_claim_guard_update;
            CREATE TRIGGER channel_targets_host_claim_guard_update
            AFTER UPDATE OF channel_id, target_type, value ON channel_targets
            BEGIN
                DELETE FROM channel_host_target_claims WHERE target_id = NEW.id;
                INSERT INTO channel_host_target_claims
                    (target_id, tenant_id, target_type, value)
                SELECT
                    NEW.id,
                    channel.tenant_id,
                    NEW.target_type,
                    NEW.value
                FROM channels channel
                WHERE channel.id = NEW.channel_id
                  AND NEW.target_type = '{HOST_TARGET_TYPE}';
            END;

            DROP TRIGGER IF EXISTS channel_targets_host_claim_delete;
            CREATE TRIGGER channel_targets_host_claim_delete
            AFTER DELETE ON channel_targets
            BEGIN
                DELETE FROM channel_host_target_claims WHERE target_id = OLD.id;
            END;

            DROP TRIGGER IF EXISTS channels_host_claim_tenant_move;
            CREATE TRIGGER channels_host_claim_tenant_move
            AFTER UPDATE OF tenant_id ON channels
            WHEN NEW.tenant_id IS NOT OLD.tenant_id
            BEGIN
                UPDATE channel_host_target_claims
                   SET tenant_id = NEW.tenant_id
                 WHERE target_id IN (
                    SELECT id FROM channel_targets WHERE channel_id = OLD.id
                 );
            END;
            "#
        ))
        .await?;
    Ok(())
}

#[derive(Iden)]
enum ChannelTargets {
    Table,
    Id,
}

#[derive(Iden)]
enum ChannelHostTargetClaims {
    Table,
    TargetId,
    TenantId,
    TargetType,
    Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    async fn sqlite_target_schema() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE channels (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
            CREATE TABLE channel_targets (
                id TEXT PRIMARY KEY NOT NULL,
                channel_id TEXT NOT NULL,
                target_type TEXT NOT NULL,
                value TEXT NOT NULL,
                is_primary INTEGER NOT NULL,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
            );
            "#,
        )
        .await
        .unwrap();
        db
    }

    async fn insert_channel(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) -> Uuid {
        let id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id) VALUES (?1, ?2)",
            vec![id.into(), tenant_id.into()],
        ))
        .await
        .unwrap();
        id
    }

    async fn insert_target(
        db: &sea_orm::DatabaseConnection,
        channel_id: Uuid,
        target_type: &str,
        value: &str,
        is_primary: bool,
    ) -> Result<Uuid, sea_orm::DbErr> {
        let id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_targets (id, channel_id, target_type, value, is_primary) VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                id.into(),
                channel_id.into(),
                target_type.into(),
                value.into(),
                is_primary.into(),
            ],
        ))
        .await?;
        Ok(id)
    }

    async fn count(
        db: &sea_orm::DatabaseConnection,
        sql: &str,
        id: Uuid,
    ) -> i64 {
        db.query_one(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            sql,
            vec![id.into()],
        ))
        .await
        .unwrap()
        .unwrap()
        .try_get("", "count")
        .unwrap()
    }

    #[tokio::test]
    async fn sqlite_serializes_host_claims_and_primary_promotion() {
        let db = sqlite_target_schema().await;
        Migration.up(&SchemaManager::new(&db)).await.unwrap();

        let tenant_a = Uuid::new_v4();
        let channel_a1 = insert_channel(&db, tenant_a).await;
        let channel_a2 = insert_channel(&db, tenant_a).await;
        let channel_b = insert_channel(&db, Uuid::new_v4()).await;

        insert_target(&db, channel_a1, HOST_TARGET_TYPE, "shop.example", true)
            .await
            .unwrap();
        assert!(
            insert_target(&db, channel_a2, HOST_TARGET_TYPE, "shop.example", false)
                .await
                .is_err()
        );
        insert_target(&db, channel_b, HOST_TARGET_TYPE, "shop.example", true)
            .await
            .unwrap();

        insert_target(&db, channel_a1, "mobile_app", "ios", true)
            .await
            .unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) AS count FROM channel_targets WHERE channel_id = ?1 AND is_primary = 1",
                channel_a1,
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn sqlite_replay_rebuilds_claims_without_duplicates() {
        let db = sqlite_target_schema().await;
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        let tenant_id = Uuid::new_v4();
        let channel_id = insert_channel(&db, tenant_id).await;
        insert_target(&db, channel_id, HOST_TARGET_TYPE, "replay.example", false)
            .await
            .unwrap();

        Migration.up(&manager).await.unwrap();
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) AS count FROM channel_host_target_claims WHERE tenant_id = ?1",
                tenant_id,
            )
            .await,
            1
        );
    }

    #[tokio::test]
    async fn migration_rejects_historical_duplicate_host_claims() {
        let db = sqlite_target_schema().await;
        let tenant_id = Uuid::new_v4();
        let channel_a = insert_channel(&db, tenant_id).await;
        let channel_b = insert_channel(&db, tenant_id).await;
        insert_target(&db, channel_a, HOST_TARGET_TYPE, "duplicate.example", false)
            .await
            .unwrap();
        insert_target(&db, channel_b, HOST_TARGET_TYPE, "duplicate.example", false)
            .await
            .unwrap();

        let error = Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect_err("duplicate claims must block migration");
        assert!(error.to_string().contains(&tenant_id.to_string()));
        assert!(error.to_string().contains("duplicate.example"));
    }
}
