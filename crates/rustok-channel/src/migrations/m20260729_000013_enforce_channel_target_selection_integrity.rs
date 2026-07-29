use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

const HOST_TARGET_TYPE: &str = "web_domain";
const PRIMARY_TARGET_INDEX: &str = "uq_channel_targets_one_primary";
const MYSQL_PRIMARY_GUARD_COLUMN: &str = "primary_target_guard";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        reject_existing_duplicate_host_claims(manager).await?;
        reject_existing_duplicate_primary_targets(manager).await?;
        create_claim_table(manager).await?;
        backfill_claims(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
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
        .query_one(Statement::from_sql_and_values(
            manager.get_database_backend(),
            r#"
            SELECT channel.tenant_id, target.value
            FROM channel_targets target
            JOIN channels channel ON channel.id = target.channel_id
            WHERE target.target_type = $1
            GROUP BY channel.tenant_id, target.value
            HAVING COUNT(*) > 1
            LIMIT 1
            "#
            .replace("$1", placeholder(manager.get_database_backend(), 1)),
            vec![HOST_TARGET_TYPE.into()],
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
    manager
        .create_index(
            Index::create()
                .name("uq_channel_host_target_claims_tenant_value")
                .table(ChannelHostTargetClaims::Table)
                .col(ChannelHostTargetClaims::TenantId)
                .col(ChannelHostTargetClaims::TargetType)
                .col(ChannelHostTargetClaims::Value)
                .unique()
                .to_owned(),
        )
        .await
}

async fn backfill_claims(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    manager
        .get_connection()
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                r#"
                INSERT INTO channel_host_target_claims (target_id, tenant_id, target_type, value)
                SELECT target.id, channel.tenant_id, target.target_type, target.value
                FROM channel_targets target
                JOIN channels channel ON channel.id = target.channel_id
                WHERE target.target_type = {}
                "#,
                placeholder(backend, 1)
            ),
            vec![HOST_TARGET_TYPE.into()],
        ))
        .await?;
    Ok(())
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            CREATE UNIQUE INDEX {PRIMARY_TARGET_INDEX}
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
            CREATE UNIQUE INDEX IF NOT EXISTS {PRIMARY_TARGET_INDEX}
                ON channel_targets (channel_id)
                WHERE is_primary = 1;

            CREATE TRIGGER IF NOT EXISTS channel_targets_host_claim_guard_insert
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

            CREATE TRIGGER IF NOT EXISTS channel_targets_host_claim_guard_update
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

            CREATE TRIGGER IF NOT EXISTS channel_targets_host_claim_delete
            AFTER DELETE ON channel_targets
            BEGIN
                DELETE FROM channel_host_target_claims WHERE target_id = OLD.id;
            END;

            CREATE TRIGGER IF NOT EXISTS channels_host_claim_tenant_move
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

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            ALTER TABLE channel_targets
                ADD COLUMN {MYSQL_PRIMARY_GUARD_COLUMN} TINYINT
                    GENERATED ALWAYS AS (
                        CASE WHEN is_primary THEN 1 ELSE NULL END
                    ) STORED,
                ADD UNIQUE INDEX {PRIMARY_TARGET_INDEX} (
                    channel_id,
                    {MYSQL_PRIMARY_GUARD_COLUMN}
                );

            CREATE TRIGGER channel_targets_host_claim_guard_insert
            AFTER INSERT ON channel_targets
            FOR EACH ROW
            BEGIN
                IF NEW.target_type = '{HOST_TARGET_TYPE}' THEN
                    INSERT INTO channel_host_target_claims
                        (target_id, tenant_id, target_type, value)
                    VALUES (
                        NEW.id,
                        (SELECT tenant_id FROM channels WHERE id = NEW.channel_id),
                        NEW.target_type,
                        NEW.value
                    );
                END IF;
            END;

            CREATE TRIGGER channel_targets_host_claim_guard_update
            AFTER UPDATE ON channel_targets
            FOR EACH ROW
            BEGIN
                DELETE FROM channel_host_target_claims WHERE target_id = NEW.id;
                IF NEW.target_type = '{HOST_TARGET_TYPE}' THEN
                    INSERT INTO channel_host_target_claims
                        (target_id, tenant_id, target_type, value)
                    VALUES (
                        NEW.id,
                        (SELECT tenant_id FROM channels WHERE id = NEW.channel_id),
                        NEW.target_type,
                        NEW.value
                    );
                END IF;
            END;

            CREATE TRIGGER channel_targets_host_claim_delete
            AFTER DELETE ON channel_targets
            FOR EACH ROW
            BEGIN
                DELETE FROM channel_host_target_claims WHERE target_id = OLD.id;
            END;

            CREATE TRIGGER channels_host_claim_tenant_move
            AFTER UPDATE ON channels
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.tenant_id <=> OLD.tenant_id) THEN
                    UPDATE channel_host_target_claims claim
                    JOIN channel_targets target ON target.id = claim.target_id
                       SET claim.tenant_id = NEW.tenant_id
                     WHERE target.channel_id = OLD.id;
                END IF;
            END;
            "#
        ))
        .await?;
    Ok(())
}

fn placeholder(backend: DatabaseBackend, position: usize) -> &'static str {
    match backend {
        DatabaseBackend::Postgres => match position {
            1 => "$1",
            _ => unreachable!("unsupported PostgreSQL placeholder position"),
        },
        DatabaseBackend::MySql => "?",
        DatabaseBackend::Sqlite => match position {
            1 => "?1",
            _ => unreachable!("unsupported SQLite placeholder position"),
        },
    }
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
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
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
                FOREIGN KEY (channel_id) REFERENCES channels(id) ON DELETE CASCADE
            );
            "#,
        )
        .await
        .expect("target schema");
        db
    }

    async fn insert_channel(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
    ) -> Uuid {
        let id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channels (id, tenant_id) VALUES (?1, ?2)",
            vec![id.into(), tenant_id.into()],
        ))
        .await
        .expect("channel");
        id
    }

    async fn insert_target(
        db: &sea_orm::DatabaseConnection,
        channel_id: Uuid,
        target_type: &str,
        value: &str,
        is_primary: bool,
    ) -> Result<(), sea_orm::DbErr> {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_targets (id, channel_id, target_type, value, is_primary) VALUES (?1, ?2, ?3, ?4, ?5)",
            vec![
                Uuid::new_v4().into(),
                channel_id.into(),
                target_type.into(),
                value.into(),
                is_primary.into(),
            ],
        ))
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn sqlite_serializes_host_claims_per_tenant_and_primary_target() {
        let db = sqlite_target_schema().await;
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("target integrity migration");

        let tenant_a = Uuid::new_v4();
        let channel_a1 = insert_channel(&db, tenant_a).await;
        let channel_a2 = insert_channel(&db, tenant_a).await;
        let channel_b = insert_channel(&db, Uuid::new_v4()).await;

        insert_target(&db, channel_a1, HOST_TARGET_TYPE, "shop.example", true)
            .await
            .expect("first host claim");
        assert!(
            insert_target(&db, channel_a2, HOST_TARGET_TYPE, "shop.example", false)
                .await
                .is_err()
        );
        insert_target(&db, channel_b, HOST_TARGET_TYPE, "shop.example", true)
            .await
            .expect("other tenant host claim");
        assert!(
            insert_target(&db, channel_a1, "mobile_app", "ios", true)
                .await
                .is_err()
        );
        insert_target(&db, channel_a2, "mobile_app", "ios", false)
            .await
            .expect("non-host duplicate value");
    }

    #[tokio::test]
    async fn migration_rejects_historical_duplicate_host_claims() {
        let db = sqlite_target_schema().await;
        let tenant_id = Uuid::new_v4();
        let channel_a = insert_channel(&db, tenant_id).await;
        let channel_b = insert_channel(&db, tenant_id).await;
        insert_target(&db, channel_a, HOST_TARGET_TYPE, "duplicate.example", false)
            .await
            .expect("first historical target");
        insert_target(&db, channel_b, HOST_TARGET_TYPE, "duplicate.example", false)
            .await
            .expect("second historical target");

        let error = Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect_err("duplicate claims must block migration");
        assert!(error.to_string().contains(&tenant_id.to_string()));
        assert!(error.to_string().contains("duplicate.example"));
    }
}
