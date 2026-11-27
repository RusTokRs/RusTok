//! Protect the platform-owned session and refresh history FK lock order.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES_GUARDS: &str = r#"
CREATE FUNCTION rustok_fence_session_parents() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.tenant_id, NEW.user_id) IS NOT DISTINCT FROM
           ROW(OLD.tenant_id, OLD.user_id) THEN RETURN NEW; END IF;
    END IF;
    LOCK TABLE tenants IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM tenants WHERE id = NEW.tenant_id FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Session tenant parent missing or invisible' USING ERRCODE = '23503';
    END IF;
    LOCK TABLE users IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM users WHERE id = NEW.user_id FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Session user parent missing or invisible' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Session parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER trg_session_parent_fence BEFORE INSERT OR UPDATE OF tenant_id, user_id ON sessions
FOR EACH ROW EXECUTE FUNCTION rustok_fence_session_parents();

CREATE FUNCTION rustok_fence_session_history_parent() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    LOCK TABLE sessions IN ROW SHARE MODE NOWAIT;
    -- Existing history is owned by its session. Even metadata updates and
    -- direct deletes must not hold a child while waiting on its parent.
    IF TG_OP <> 'INSERT' THEN
        PERFORM 1 FROM sessions WHERE id = OLD.session_id FOR KEY SHARE NOWAIT;
        IF TG_OP = 'DELETE' THEN
            -- A session's FK cascade runs after its parent is deleted in
            -- this transaction; absence here is expected in that case.
            RETURN OLD;
        END IF;
    END IF;
    IF TG_OP = 'UPDATE' THEN
        IF NEW.session_id IS NOT DISTINCT FROM OLD.session_id THEN RETURN NEW; END IF;
    END IF;
    PERFORM 1 FROM sessions WHERE id = NEW.session_id FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Session history parent missing or invisible' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Session history parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER trg_session_history_parent_fence
BEFORE INSERT OR UPDATE OR DELETE ON session_refresh_history
FOR EACH ROW EXECUTE FUNCTION rustok_fence_session_history_parent();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager.get_connection().execute_unprepared(POSTGRES_GUARDS).await?;
            }
            DatabaseBackend::Sqlite => {}
            _ => return Err(DbErr::Custom("Unsupported session parent fence database".into())),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => Err(DbErr::Migration(
                "Session parent lock order cannot be downgraded safely".into(),
            )),
            DatabaseBackend::Sqlite => Ok(()),
            _ => Err(DbErr::Custom("Unsupported session parent fence database".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DatabaseTransaction, TransactionTrait};
    use std::time::Duration;

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_session_history_parent_guard_rejects_late_fk_waits() {
        let db = Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("session_parent_fence_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        setup.execute_unprepared(
            "CREATE TABLE tenants (id UUID PRIMARY KEY);
             CREATE TABLE users (id UUID PRIMARY KEY);
             CREATE TABLE sessions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id), user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE);
             CREATE TABLE session_refresh_history (token_hash TEXT PRIMARY KEY, session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE, rotated_at TIMESTAMPTZ);
             INSERT INTO tenants VALUES ('10000000-0000-0000-0000-000000000001');
             INSERT INTO users VALUES ('20000000-0000-0000-0000-000000000001');
             INSERT INTO sessions VALUES ('30000000-0000-0000-0000-000000000001', '10000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000001');
             INSERT INTO session_refresh_history VALUES ('existing', '30000000-0000-0000-0000-000000000001', CURRENT_TIMESTAMP);"
        ).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        for (lock, writes, message) in [
            ("SELECT id FROM tenants FOR UPDATE", vec!["INSERT INTO sessions VALUES ('30000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000001')"], "Session parent fence busy"),
            ("SELECT id FROM users FOR UPDATE", vec!["INSERT INTO sessions VALUES ('30000000-0000-0000-0000-000000000002', '10000000-0000-0000-0000-000000000001', '20000000-0000-0000-0000-000000000001')"], "Session parent fence busy"),
            ("SELECT id FROM sessions FOR UPDATE", vec![
                "INSERT INTO session_refresh_history VALUES ('new', '30000000-0000-0000-0000-000000000001', CURRENT_TIMESTAMP)",
                "UPDATE session_refresh_history SET rotated_at = CURRENT_TIMESTAMP WHERE token_hash = 'existing'",
                "DELETE FROM session_refresh_history WHERE token_hash = 'existing'",
            ], "Session history parent fence busy"),
        ] {
            let owner = tx(&db, &schema).await;
            owner.execute_unprepared(lock).await.unwrap();
            for sql in writes {
                let writer = tx(&db, &schema).await;
                let error = tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(sql))
                    .await.expect("late parent fence must not wait")
                    .expect_err("busy parent must reject");
                assert!(error.to_string().contains(message), "{error}");
                writer.rollback().await.unwrap();
            }
            owner.rollback().await.unwrap();
        }

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(
            "SELECT id FROM sessions FOR UPDATE;
             INSERT INTO session_refresh_history VALUES ('new', '30000000-0000-0000-0000-000000000001', CURRENT_TIMESTAMP)"
        ).await.unwrap();
        owner.commit().await.unwrap();
        let owner = tx(&db, &schema).await;
        owner.execute_unprepared("DELETE FROM users").await.unwrap();
        owner.commit().await.unwrap();
        let check = tx(&db, &schema).await;
        let row = check.query_one_raw(sea_orm_migration::sea_orm::Statement::from_string(
            DatabaseBackend::Postgres, "SELECT COUNT(*) AS count FROM session_refresh_history"
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
        check.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
