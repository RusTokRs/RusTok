use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        reject_existing_cross_tenant_relations(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: these guards are tenant-isolation invariants. Removing
        // them would permit cross-tenant OAuth bindings and policy actions.
        Ok(())
    }
}

async fn reject_existing_cross_tenant_relations(
    manager: &SchemaManager<'_>,
) -> Result<(), DbErr> {
    let backend = manager.get_database_backend();
    let mismatch = manager
        .get_connection()
        .query_one(Statement::from_string(
            backend,
            r#"
            SELECT relation_name, relation_id, owner_tenant_id, related_tenant_id
            FROM (
                SELECT
                    'channel_oauth_apps' AS relation_name,
                    coa.id AS relation_id,
                    c.tenant_id AS owner_tenant_id,
                    oa.tenant_id AS related_tenant_id
                FROM channel_oauth_apps coa
                JOIN channels c ON c.id = coa.channel_id
                JOIN oauth_apps oa ON oa.id = coa.oauth_app_id
                WHERE c.tenant_id <> oa.tenant_id

                UNION ALL

                SELECT
                    'channel_resolution_policy_rules' AS relation_name,
                    rule.id AS relation_id,
                    policy_set.tenant_id AS owner_tenant_id,
                    channel.tenant_id AS related_tenant_id
                FROM channel_resolution_policy_rules rule
                JOIN channel_resolution_policy_sets policy_set
                  ON policy_set.id = rule.policy_set_id
                JOIN channels channel ON channel.id = rule.action_channel_id
                WHERE policy_set.tenant_id <> channel.tenant_id
            ) mismatches
            LIMIT 1
            "#
            .to_string(),
        ))
        .await?;

    if let Some(row) = mismatch {
        let relation_name: String = row.try_get("", "relation_name")?;
        let relation_id: Uuid = row.try_get("", "relation_id")?;
        let owner_tenant_id: Uuid = row.try_get("", "owner_tenant_id")?;
        let related_tenant_id: Uuid = row.try_get("", "related_tenant_id")?;
        return Err(DbErr::Custom(format!(
            "cannot enforce channel tenant integrity: {relation_name} relation {relation_id} connects tenant {owner_tenant_id} to tenant {related_tenant_id}"
        )));
    }

    Ok(())
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION channel_validate_oauth_binding_tenant()
            RETURNS trigger AS $$
            DECLARE
                channel_tenant UUID;
                oauth_tenant UUID;
            BEGIN
                SELECT tenant_id INTO channel_tenant FROM channels WHERE id = NEW.channel_id;
                SELECT tenant_id INTO oauth_tenant FROM oauth_apps WHERE id = NEW.oauth_app_id;
                IF channel_tenant IS DISTINCT FROM oauth_tenant THEN
                    RAISE EXCEPTION 'channel OAuth binding crosses tenant boundary'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channel_oauth_apps_tenant_guard ON channel_oauth_apps;
            CREATE TRIGGER channel_oauth_apps_tenant_guard
            BEFORE INSERT OR UPDATE OF channel_id, oauth_app_id ON channel_oauth_apps
            FOR EACH ROW EXECUTE FUNCTION channel_validate_oauth_binding_tenant();

            CREATE OR REPLACE FUNCTION channel_validate_policy_rule_tenant()
            RETURNS trigger AS $$
            DECLARE
                policy_tenant UUID;
                channel_tenant UUID;
            BEGIN
                SELECT tenant_id INTO policy_tenant
                  FROM channel_resolution_policy_sets
                 WHERE id = NEW.policy_set_id;
                SELECT tenant_id INTO channel_tenant
                  FROM channels
                 WHERE id = NEW.action_channel_id;
                IF policy_tenant IS DISTINCT FROM channel_tenant THEN
                    RAISE EXCEPTION 'channel policy action crosses tenant boundary'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channel_policy_rules_tenant_guard
                ON channel_resolution_policy_rules;
            CREATE TRIGGER channel_policy_rules_tenant_guard
            BEFORE INSERT OR UPDATE OF policy_set_id, action_channel_id
                ON channel_resolution_policy_rules
            FOR EACH ROW EXECUTE FUNCTION channel_validate_policy_rule_tenant();

            CREATE OR REPLACE FUNCTION channel_guard_channel_tenant_move()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND (
                    EXISTS (
                        SELECT 1
                        FROM channel_oauth_apps binding
                        JOIN oauth_apps app ON app.id = binding.oauth_app_id
                        WHERE binding.channel_id = OLD.id
                          AND app.tenant_id IS DISTINCT FROM NEW.tenant_id
                    )
                    OR EXISTS (
                        SELECT 1
                        FROM channel_resolution_policy_rules rule
                        JOIN channel_resolution_policy_sets policy_set
                          ON policy_set.id = rule.policy_set_id
                        WHERE rule.action_channel_id = OLD.id
                          AND policy_set.tenant_id IS DISTINCT FROM NEW.tenant_id
                    )
                ) THEN
                    RAISE EXCEPTION 'channel tenant move would break tenant-safe relations'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channels_relation_tenant_move_guard ON channels;
            CREATE TRIGGER channels_relation_tenant_move_guard
            BEFORE UPDATE OF tenant_id ON channels
            FOR EACH ROW EXECUTE FUNCTION channel_guard_channel_tenant_move();

            CREATE OR REPLACE FUNCTION channel_guard_oauth_app_tenant_move()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND EXISTS (
                    SELECT 1
                    FROM channel_oauth_apps binding
                    JOIN channels channel ON channel.id = binding.channel_id
                    WHERE binding.oauth_app_id = OLD.id
                      AND channel.tenant_id IS DISTINCT FROM NEW.tenant_id
                ) THEN
                    RAISE EXCEPTION 'OAuth app tenant move would break channel tenant integrity'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS oauth_apps_channel_tenant_move_guard ON oauth_apps;
            CREATE TRIGGER oauth_apps_channel_tenant_move_guard
            BEFORE UPDATE OF tenant_id ON oauth_apps
            FOR EACH ROW EXECUTE FUNCTION channel_guard_oauth_app_tenant_move();

            CREATE OR REPLACE FUNCTION channel_guard_policy_set_tenant_move()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id AND EXISTS (
                    SELECT 1
                    FROM channel_resolution_policy_rules rule
                    JOIN channels channel ON channel.id = rule.action_channel_id
                    WHERE rule.policy_set_id = OLD.id
                      AND channel.tenant_id IS DISTINCT FROM NEW.tenant_id
                ) THEN
                    RAISE EXCEPTION 'channel policy-set tenant move would break tenant integrity'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS channel_policy_sets_tenant_move_guard
                ON channel_resolution_policy_sets;
            CREATE TRIGGER channel_policy_sets_tenant_move_guard
            BEFORE UPDATE OF tenant_id ON channel_resolution_policy_sets
            FOR EACH ROW EXECUTE FUNCTION channel_guard_policy_set_tenant_move();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER IF NOT EXISTS channel_oauth_apps_tenant_guard_insert
            BEFORE INSERT ON channel_oauth_apps
            WHEN (SELECT tenant_id FROM channels WHERE id = NEW.channel_id)
                 IS NOT
                 (SELECT tenant_id FROM oauth_apps WHERE id = NEW.oauth_app_id)
            BEGIN
                SELECT RAISE(ABORT, 'channel OAuth binding crosses tenant boundary');
            END;

            CREATE TRIGGER IF NOT EXISTS channel_oauth_apps_tenant_guard_update
            BEFORE UPDATE OF channel_id, oauth_app_id ON channel_oauth_apps
            WHEN (SELECT tenant_id FROM channels WHERE id = NEW.channel_id)
                 IS NOT
                 (SELECT tenant_id FROM oauth_apps WHERE id = NEW.oauth_app_id)
            BEGIN
                SELECT RAISE(ABORT, 'channel OAuth binding crosses tenant boundary');
            END;

            CREATE TRIGGER IF NOT EXISTS channel_policy_rules_tenant_guard_insert
            BEFORE INSERT ON channel_resolution_policy_rules
            WHEN (SELECT tenant_id FROM channel_resolution_policy_sets WHERE id = NEW.policy_set_id)
                 IS NOT
                 (SELECT tenant_id FROM channels WHERE id = NEW.action_channel_id)
            BEGIN
                SELECT RAISE(ABORT, 'channel policy action crosses tenant boundary');
            END;

            CREATE TRIGGER IF NOT EXISTS channel_policy_rules_tenant_guard_update
            BEFORE UPDATE OF policy_set_id, action_channel_id ON channel_resolution_policy_rules
            WHEN (SELECT tenant_id FROM channel_resolution_policy_sets WHERE id = NEW.policy_set_id)
                 IS NOT
                 (SELECT tenant_id FROM channels WHERE id = NEW.action_channel_id)
            BEGIN
                SELECT RAISE(ABORT, 'channel policy action crosses tenant boundary');
            END;

            CREATE TRIGGER IF NOT EXISTS channels_relation_tenant_move_guard
            BEFORE UPDATE OF tenant_id ON channels
            WHEN EXISTS (
                SELECT 1
                FROM channel_oauth_apps binding
                JOIN oauth_apps app ON app.id = binding.oauth_app_id
                WHERE binding.channel_id = OLD.id
                  AND app.tenant_id IS NOT NEW.tenant_id
            ) OR EXISTS (
                SELECT 1
                FROM channel_resolution_policy_rules rule
                JOIN channel_resolution_policy_sets policy_set
                  ON policy_set.id = rule.policy_set_id
                WHERE rule.action_channel_id = OLD.id
                  AND policy_set.tenant_id IS NOT NEW.tenant_id
            )
            BEGIN
                SELECT RAISE(ABORT, 'channel tenant move would break tenant-safe relations');
            END;

            CREATE TRIGGER IF NOT EXISTS oauth_apps_channel_tenant_move_guard
            BEFORE UPDATE OF tenant_id ON oauth_apps
            WHEN EXISTS (
                SELECT 1
                FROM channel_oauth_apps binding
                JOIN channels channel ON channel.id = binding.channel_id
                WHERE binding.oauth_app_id = OLD.id
                  AND channel.tenant_id IS NOT NEW.tenant_id
            )
            BEGIN
                SELECT RAISE(ABORT, 'OAuth app tenant move would break channel tenant integrity');
            END;

            CREATE TRIGGER IF NOT EXISTS channel_policy_sets_tenant_move_guard
            BEFORE UPDATE OF tenant_id ON channel_resolution_policy_sets
            WHEN EXISTS (
                SELECT 1
                FROM channel_resolution_policy_rules rule
                JOIN channels channel ON channel.id = rule.action_channel_id
                WHERE rule.policy_set_id = OLD.id
                  AND channel.tenant_id IS NOT NEW.tenant_id
            )
            BEGIN
                SELECT RAISE(ABORT, 'channel policy-set tenant move would break tenant integrity');
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER channel_oauth_apps_tenant_guard_insert
            BEFORE INSERT ON channel_oauth_apps
            FOR EACH ROW
            BEGIN
                IF (SELECT tenant_id FROM channels WHERE id = NEW.channel_id)
                   <> (SELECT tenant_id FROM oauth_apps WHERE id = NEW.oauth_app_id) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'channel OAuth binding crosses tenant boundary';
                END IF;
            END;

            CREATE TRIGGER channel_oauth_apps_tenant_guard_update
            BEFORE UPDATE ON channel_oauth_apps
            FOR EACH ROW
            BEGIN
                IF (SELECT tenant_id FROM channels WHERE id = NEW.channel_id)
                   <> (SELECT tenant_id FROM oauth_apps WHERE id = NEW.oauth_app_id) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'channel OAuth binding crosses tenant boundary';
                END IF;
            END;

            CREATE TRIGGER channel_policy_rules_tenant_guard_insert
            BEFORE INSERT ON channel_resolution_policy_rules
            FOR EACH ROW
            BEGIN
                IF (SELECT tenant_id FROM channel_resolution_policy_sets WHERE id = NEW.policy_set_id)
                   <> (SELECT tenant_id FROM channels WHERE id = NEW.action_channel_id) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'channel policy action crosses tenant boundary';
                END IF;
            END;

            CREATE TRIGGER channel_policy_rules_tenant_guard_update
            BEFORE UPDATE ON channel_resolution_policy_rules
            FOR EACH ROW
            BEGIN
                IF (SELECT tenant_id FROM channel_resolution_policy_sets WHERE id = NEW.policy_set_id)
                   <> (SELECT tenant_id FROM channels WHERE id = NEW.action_channel_id) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'channel policy action crosses tenant boundary';
                END IF;
            END;

            CREATE TRIGGER channels_relation_tenant_move_guard
            BEFORE UPDATE ON channels
            FOR EACH ROW
            BEGIN
                IF NOT (OLD.tenant_id <=> NEW.tenant_id) AND (
                    EXISTS (
                        SELECT 1
                        FROM channel_oauth_apps binding
                        JOIN oauth_apps app ON app.id = binding.oauth_app_id
                        WHERE binding.channel_id = OLD.id
                          AND NOT (app.tenant_id <=> NEW.tenant_id)
                    )
                    OR EXISTS (
                        SELECT 1
                        FROM channel_resolution_policy_rules rule
                        JOIN channel_resolution_policy_sets policy_set
                          ON policy_set.id = rule.policy_set_id
                        WHERE rule.action_channel_id = OLD.id
                          AND NOT (policy_set.tenant_id <=> NEW.tenant_id)
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'channel tenant move would break tenant-safe relations';
                END IF;
            END;

            CREATE TRIGGER oauth_apps_channel_tenant_move_guard
            BEFORE UPDATE ON oauth_apps
            FOR EACH ROW
            BEGIN
                IF NOT (OLD.tenant_id <=> NEW.tenant_id) AND EXISTS (
                    SELECT 1
                    FROM channel_oauth_apps binding
                    JOIN channels channel ON channel.id = binding.channel_id
                    WHERE binding.oauth_app_id = OLD.id
                      AND NOT (channel.tenant_id <=> NEW.tenant_id)
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'OAuth app tenant move would break channel tenant integrity';
                END IF;
            END;

            CREATE TRIGGER channel_policy_sets_tenant_move_guard
            BEFORE UPDATE ON channel_resolution_policy_sets
            FOR EACH ROW
            BEGIN
                IF NOT (OLD.tenant_id <=> NEW.tenant_id) AND EXISTS (
                    SELECT 1
                    FROM channel_resolution_policy_rules rule
                    JOIN channels channel ON channel.id = rule.action_channel_id
                    WHERE rule.policy_set_id = OLD.id
                      AND NOT (channel.tenant_id <=> NEW.tenant_id)
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'channel policy-set tenant move would break tenant integrity';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    async fn sqlite_relation_schema() -> sea_orm::DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared(
            r#"
            CREATE TABLE channels (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
            CREATE TABLE oauth_apps (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL);
            CREATE TABLE channel_oauth_apps (
                id TEXT PRIMARY KEY NOT NULL,
                channel_id TEXT NOT NULL,
                oauth_app_id TEXT NOT NULL
            );
            CREATE TABLE channel_resolution_policy_sets (
                id TEXT PRIMARY KEY NOT NULL,
                tenant_id TEXT NOT NULL
            );
            CREATE TABLE channel_resolution_policy_rules (
                id TEXT PRIMARY KEY NOT NULL,
                policy_set_id TEXT NOT NULL,
                action_channel_id TEXT NOT NULL
            );
            "#,
        )
        .await
        .expect("channel relation schema");
        db
    }

    async fn insert_identity(
        db: &sea_orm::DatabaseConnection,
        table: &str,
        id: Uuid,
        tenant_id: Uuid,
    ) {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            format!("INSERT INTO {table} (id, tenant_id) VALUES (?1, ?2)"),
            vec![id.into(), tenant_id.into()],
        ))
        .await
        .expect("identity row");
    }

    #[tokio::test]
    async fn sqlite_rejects_cross_tenant_oauth_bindings_and_policy_actions() {
        let db = sqlite_relation_schema().await;
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("tenant integrity guards");

        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let channel_a = Uuid::new_v4();
        let oauth_a = Uuid::new_v4();
        let oauth_b = Uuid::new_v4();
        let policy_a = Uuid::new_v4();
        insert_identity(&db, "channels", channel_a, tenant_a).await;
        insert_identity(&db, "oauth_apps", oauth_a, tenant_a).await;
        insert_identity(&db, "oauth_apps", oauth_b, tenant_b).await;
        insert_identity(&db, "channel_resolution_policy_sets", policy_a, tenant_a).await;

        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_oauth_apps (id, channel_id, oauth_app_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), channel_a.into(), oauth_a.into()],
        ))
        .await
        .expect("same-tenant OAuth binding");
        assert!(
            db.execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO channel_oauth_apps (id, channel_id, oauth_app_id) VALUES (?1, ?2, ?3)",
                vec![Uuid::new_v4().into(), channel_a.into(), oauth_b.into()],
            ))
            .await
            .is_err()
        );

        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_resolution_policy_rules (id, policy_set_id, action_channel_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), policy_a.into(), channel_a.into()],
        ))
        .await
        .expect("same-tenant policy action");
        let channel_b = Uuid::new_v4();
        insert_identity(&db, "channels", channel_b, tenant_b).await;
        assert!(
            db.execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO channel_resolution_policy_rules (id, policy_set_id, action_channel_id) VALUES (?1, ?2, ?3)",
                vec![Uuid::new_v4().into(), policy_a.into(), channel_b.into()],
            ))
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn sqlite_blocks_parent_tenant_moves_that_break_relations() {
        let db = sqlite_relation_schema().await;
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("tenant integrity guards");

        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let channel_a = Uuid::new_v4();
        let oauth_a = Uuid::new_v4();
        let policy_a = Uuid::new_v4();
        insert_identity(&db, "channels", channel_a, tenant_a).await;
        insert_identity(&db, "oauth_apps", oauth_a, tenant_a).await;
        insert_identity(&db, "channel_resolution_policy_sets", policy_a, tenant_a).await;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_oauth_apps (id, channel_id, oauth_app_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), channel_a.into(), oauth_a.into()],
        ))
        .await
        .expect("OAuth binding");
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_resolution_policy_rules (id, policy_set_id, action_channel_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), policy_a.into(), channel_a.into()],
        ))
        .await
        .expect("policy action");

        for (table, id) in [
            ("channels", channel_a),
            ("oauth_apps", oauth_a),
            ("channel_resolution_policy_sets", policy_a),
        ] {
            assert!(
                db.execute(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    format!("UPDATE {table} SET tenant_id = ?1 WHERE id = ?2"),
                    vec![tenant_b.into(), id.into()],
                ))
                .await
                .is_err()
            );
        }
    }

    #[tokio::test]
    async fn migration_rejects_historical_cross_tenant_relations() {
        let db = sqlite_relation_schema().await;
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let channel_a = Uuid::new_v4();
        let oauth_b = Uuid::new_v4();
        insert_identity(&db, "channels", channel_a, tenant_a).await;
        insert_identity(&db, "oauth_apps", oauth_b, tenant_b).await;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO channel_oauth_apps (id, channel_id, oauth_app_id) VALUES (?1, ?2, ?3)",
            vec![Uuid::new_v4().into(), channel_a.into(), oauth_b.into()],
        ))
        .await
        .expect("historical invalid binding");

        let error = Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect_err("historical mismatch must block migration");
        assert!(error.to_string().contains("channel_oauth_apps"));
        assert!(error.to_string().contains(&tenant_a.to_string()));
        assert!(error.to_string().contains(&tenant_b.to_string()));
    }
}
