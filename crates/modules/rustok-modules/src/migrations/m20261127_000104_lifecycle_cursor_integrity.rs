//! Preserve lifecycle history used by admitted Auth and RBAC credentials.
use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION module_lifecycle_cursor_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'Policy cursor table cannot be truncated' USING ERRCODE = '23514';
    ELSIF TG_OP = 'DELETE' THEN
        IF OLD.consumer_key = 'module.lifecycle' THEN
            RAISE EXCEPTION 'Lifecycle cursor history cannot be removed' USING ERRCODE = '23514';
        END IF;
        RETURN OLD;
    ELSIF TG_OP = 'INSERT' THEN
        IF NEW.consumer_key = 'module.lifecycle' AND
           (NEW.tenant_id = '00000000-0000-0000-0000-000000000000'::uuid OR
            NEW.current_sequence IS DISTINCT FROM 0 OR NEW.current_revision IS NOT NULL) THEN
            RAISE EXCEPTION 'Lifecycle cursor must start at zero' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF OLD.consumer_key <> 'module.lifecycle' AND NEW.consumer_key <> 'module.lifecycle' THEN
        RETURN NEW;
    END IF;
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id OR NEW.consumer_key IS DISTINCT FROM OLD.consumer_key THEN
        RAISE EXCEPTION 'Lifecycle cursor identity is immutable' USING ERRCODE = '23514';
    END IF;
    IF NEW.current_sequence IS NOT DISTINCT FROM OLD.current_sequence AND
       NEW.current_revision IS NOT DISTINCT FROM OLD.current_revision AND
       NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at THEN
        RETURN NEW;
    END IF;
    IF OLD.current_sequence = 9223372036854775807 THEN
        RAISE EXCEPTION 'Lifecycle cursor sequence exhausted' USING ERRCODE = '23514';
    END IF;
    IF NEW.current_sequence IS DISTINCT FROM OLD.current_sequence + 1 OR
       NEW.current_revision IS NULL OR NEW.current_revision IS NOT DISTINCT FROM OLD.current_revision OR
       NEW.current_revision !~ '^sha256:[0-9a-fA-F]{64}$' THEN
        RAISE EXCEPTION 'Lifecycle cursor must advance by one to a different revision' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER module_lifecycle_cursor_guard
BEFORE INSERT OR UPDATE OR DELETE ON module_policy_revision_cursors
FOR EACH ROW EXECUTE FUNCTION module_lifecycle_cursor_guard();
CREATE TRIGGER module_lifecycle_cursor_no_truncate
BEFORE TRUNCATE ON module_policy_revision_cursors
FOR EACH STATEMENT EXECUTE FUNCTION module_lifecycle_cursor_guard();
"#;

const SQLITE: &str = r#"
CREATE TRIGGER module_lifecycle_cursor_insert
BEFORE INSERT ON module_policy_revision_cursors
WHEN (NEW.consumer_key = 'module.lifecycle' AND (
      NOT ((length((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END)) = 32 AND (CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END) NOT GLOB '*[^0-9a-f]*' AND (CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END) <> '00000000000000000000000000000000' AND ((typeof(NEW.tenant_id) = 'blob' AND length(NEW.tenant_id) = 16) OR (typeof(NEW.tenant_id) = 'text' AND lower(NEW.tenant_id) IN ((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END), (substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),1,8) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),9,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),13,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),17,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),21,12)), 'urn:uuid:' || (substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),1,8) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),9,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),13,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),17,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),21,12)), '{' || (substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),1,8) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),9,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),13,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),17,4) || '-' || substr((CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),21,12)) || '}'))))) OR
      typeof(NEW.current_sequence) <> 'integer' OR NEW.current_sequence <> 0 OR NEW.current_revision IS NOT NULL OR
      EXISTS (SELECT 1 FROM module_policy_revision_cursors c WHERE c.consumer_key = 'module.lifecycle'
        AND (CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END) = (CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN lower(hex(NEW.tenant_id)) ELSE replace(replace(replace(replace(lower(NEW.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END))
    )) OR EXISTS (SELECT 1 FROM module_policy_revision_cursors c WHERE c.consumer_key = 'module.lifecycle' AND c.rowid = NEW.rowid)
BEGIN SELECT RAISE(ABORT, 'Lifecycle cursor must start at zero and cannot be replaced'); END;

CREATE TRIGGER module_lifecycle_cursor_delete
BEFORE DELETE ON module_policy_revision_cursors
WHEN OLD.consumer_key = 'module.lifecycle'
BEGIN SELECT RAISE(ABORT, 'Lifecycle cursor history cannot be removed'); END;

CREATE TRIGGER module_lifecycle_cursor_update
BEFORE UPDATE ON module_policy_revision_cursors
WHEN (
    (OLD.consumer_key = 'module.lifecycle' OR NEW.consumer_key = 'module.lifecycle') AND (
        NEW.tenant_id IS NOT OLD.tenant_id OR NEW.consumer_key IS NOT OLD.consumer_key OR NEW.rowid IS NOT OLD.rowid OR
        typeof(OLD.current_sequence) <> 'integer' OR typeof(NEW.current_sequence) <> 'integer' OR
        NOT (
            (NEW.current_sequence = OLD.current_sequence AND NEW.current_revision IS OLD.current_revision AND NEW.updated_at IS OLD.updated_at)
            OR (OLD.current_sequence < 9223372036854775807 AND NEW.current_sequence = OLD.current_sequence + 1
                AND NEW.current_revision IS NOT OLD.current_revision AND (typeof(NEW.current_revision) = 'text' AND length(NEW.current_revision) = 71 AND substr(NEW.current_revision,1,7) = 'sha256:' AND substr(NEW.current_revision,8) NOT GLOB '*[^0-9a-fA-F]*'))
        )
    )
) OR (NEW.rowid IS NOT OLD.rowid AND EXISTS (
    SELECT 1 FROM module_policy_revision_cursors c WHERE c.consumer_key = 'module.lifecycle' AND c.rowid = NEW.rowid
))
BEGIN SELECT RAISE(ABORT, 'Lifecycle cursor must advance by one without changing identity'); END;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let (invalid_sql, guards) = match backend {
            SupportedBackend::Postgres => {
                db.execute_unprepared("LOCK TABLE module_policy_revision_cursors IN SHARE ROW EXCLUSIVE MODE").await?;
                // A hidden lifecycle row is not a valid cutover baseline.
                let rls = db.query_one_raw(Statement::from_string(
                    backend.db_backend(), "SELECT row_security_active('module_policy_revision_cursors'::regclass) AS enforced",
                )).await?.ok_or_else(|| DbErr::Migration("lifecycle cursor visibility unavailable".into()))?;
                if rls.try_get::<bool>("", "enforced")? {
                    return Err(DbErr::Migration("lifecycle cursor migration requires complete row visibility".into()));
                }
                ("SELECT COUNT(*) AS invalid FROM module_policy_revision_cursors WHERE consumer_key = 'module.lifecycle' AND (
                    tenant_id = '00000000-0000-0000-0000-000000000000'::uuid OR current_sequence < 0 OR
                    (current_revision IS NULL AND current_sequence <> 0) OR
                    (current_revision IS NOT NULL AND current_revision !~ '^sha256:[0-9a-fA-F]{64}$')
                )", POSTGRES)
            }
            SupportedBackend::Sqlite => {
                db.execute_unprepared("UPDATE module_policy_revision_cursors SET current_sequence = current_sequence WHERE 1 = 0").await?;
                ("SELECT COUNT(*) AS invalid FROM module_policy_revision_cursors c WHERE c.consumer_key = 'module.lifecycle' AND (
                    NOT ((length((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END)) = 32 AND (CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END) NOT GLOB '*[^0-9a-f]*' AND (CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END) <> '00000000000000000000000000000000' AND ((typeof(c.tenant_id) = 'blob' AND length(c.tenant_id) = 16) OR (typeof(c.tenant_id) = 'text' AND lower(c.tenant_id) IN ((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END), (substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),1,8) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),9,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),13,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),17,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),21,12)), 'urn:uuid:' || (substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),1,8) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),9,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),13,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),17,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),21,12)), '{' || (substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),1,8) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),9,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),13,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),17,4) || '-' || substr((CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END),21,12)) || '}'))))) OR typeof(c.current_sequence) <> 'integer' OR c.current_sequence < 0 OR
                    (c.current_revision IS NULL AND c.current_sequence <> 0) OR
                    (c.current_revision IS NOT NULL AND NOT ((typeof(c.current_revision) = 'text' AND length(c.current_revision) = 71 AND substr(c.current_revision,1,7) = 'sha256:' AND substr(c.current_revision,8) NOT GLOB '*[^0-9a-fA-F]*'))) OR
                    EXISTS (SELECT 1 FROM module_policy_revision_cursors other WHERE other.consumer_key = 'module.lifecycle'
                        AND other.rowid <> c.rowid AND (CASE WHEN typeof(other.tenant_id) = 'blob' THEN lower(hex(other.tenant_id)) ELSE replace(replace(replace(replace(lower(other.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END) = (CASE WHEN typeof(c.tenant_id) = 'blob' THEN lower(hex(c.tenant_id)) ELSE replace(replace(replace(replace(lower(c.tenant_id), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') END))
                )", SQLITE)
            }
        };
        let invalid = db.query_one_raw(Statement::from_string(backend.db_backend(), invalid_sql))
            .await?.ok_or_else(|| DbErr::Migration("lifecycle cursor validation unavailable".into()))?
            .try_get::<i64>("", "invalid")?;
        if invalid != 0 {
            return Err(DbErr::Migration("invalid lifecycle cursor history; offline reconciliation required".into()));
        }
        db.execute_unprepared(guards).await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("lifecycle cursor integrity cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SeaOrmModulePolicyRevisionConsumer;
    use sea_orm::{Database, TransactionTrait};
    use uuid::Uuid;

    #[tokio::test]
    async fn sqlite_lifecycle_history_survives_replacement_and_rollback() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let manager = SchemaManager::new(&db);
        super::super::m20260722_000037_policy_revision_cursors::Migration.up(&manager).await.unwrap();
        super::super::m20261027_000080_policy_revision_cursor_sequences::Migration.up(&manager).await.unwrap();
        let tenant = Uuid::new_v4();
        let owner = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
        let initial = owner.cursor_state(tenant, "module.lifecycle").await.unwrap();
        let a = format!("sha256:{}", "a".repeat(64));
        let b = format!("sha256:{}", "b".repeat(64));
        let issued = owner.resync_if_current(tenant, "module.lifecycle", &initial, &a).await.unwrap();
        Migration.up(&manager).await.unwrap();
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        for sql in [
            "DELETE FROM module_policy_revision_cursors".to_owned(),
            "UPDATE module_policy_revision_cursors SET current_sequence = 0".to_owned(),
            "UPDATE module_policy_revision_cursors SET current_sequence = current_sequence + 2".to_owned(),
            "UPDATE module_policy_revision_cursors SET current_sequence = current_sequence + 1".to_owned(),
            "UPDATE module_policy_revision_cursors SET current_sequence = 1.5".to_owned(),
            "UPDATE module_policy_revision_cursors SET consumer_key = 'other'".to_owned(),
            "UPDATE module_policy_revision_cursors SET rowid = rowid + 10".to_owned(),
            format!("INSERT OR REPLACE INTO module_policy_revision_cursors (tenant_id, consumer_key) VALUES ('{tenant}', 'module.lifecycle')"),
            format!("INSERT INTO module_policy_revision_cursors (tenant_id, consumer_key) VALUES ('{}', 'module.lifecycle')", tenant.simple()),
            format!("INSERT INTO module_policy_revision_cursors (tenant_id, consumer_key) VALUES (X'{}', 'module.lifecycle')", tenant.simple()),
            format!("INSERT OR REPLACE INTO module_policy_revision_cursors (rowid, tenant_id, consumer_key) SELECT rowid, '{}', 'other' FROM module_policy_revision_cursors", Uuid::new_v4()),
        ] {
            assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
            assert_eq!(owner.observe_lifecycle_cursor(tenant).await.unwrap(), Some(issued.clone()));
        }
        let tx = db.begin().await.unwrap();
        owner.resync_if_current_in_transaction(&tx, tenant, "module.lifecycle", &issued, &b).await.unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(owner.observe_lifecycle_cursor(tenant).await.unwrap(), Some(issued.clone()));
        let changed = owner.resync_if_current(tenant, "module.lifecycle", &issued, &b).await.unwrap();
        let restored = owner.resync_if_current(tenant, "module.lifecycle", &changed, &a).await.unwrap();
        assert_eq!(issued.current_revision, restored.current_revision);
        assert_eq!(restored.current_sequence, issued.current_sequence + 2);
        let fresh = Uuid::new_v4();
        assert_eq!(owner.observe_lifecycle_cursor(fresh).await.unwrap(), None);
        assert_eq!(owner.cursor_state(fresh, "module.lifecycle").await.unwrap().current_sequence, 0);
        let other = owner.cursor_state(tenant, "other").await.unwrap();
        assert_eq!(other.current_sequence, 0);
        db.execute_unprepared("DELETE FROM module_policy_revision_cursors WHERE consumer_key = 'other'").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL schema privileges"]
    async fn postgres_lifecycle_cursor_rejects_ordinary_sql_reset() {
        let db = Database::connect(std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").unwrap()).await.unwrap();
        let schema = format!("lifecycle_cursor_integrity_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        let manager = SchemaManager::new(&tx);
        super::super::m20260722_000037_policy_revision_cursors::Migration.up(&manager).await.unwrap();
        super::super::m20261027_000080_policy_revision_cursor_sequences::Migration.up(&manager).await.unwrap();
        let tenant = Uuid::new_v4();
        tx.execute_unprepared(&format!("INSERT INTO module_policy_revision_cursors (tenant_id, consumer_key, current_revision, current_sequence) VALUES ('{tenant}', 'module.lifecycle', 'sha256:{}', 5)", "a".repeat(64))).await.unwrap();
        Migration.up(&manager).await.unwrap();
        tx.commit().await.unwrap();
        for sql in [
            "DELETE FROM module_policy_revision_cursors",
            "TRUNCATE module_policy_revision_cursors",
            "UPDATE module_policy_revision_cursors SET current_sequence = 0",
            "UPDATE module_policy_revision_cursors SET current_sequence = 7",
            "UPDATE module_policy_revision_cursors SET consumer_key = 'other'",
            "UPDATE module_policy_revision_cursors SET current_revision = NULL, current_sequence = 6",
        ] {
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            assert!(tx.execute_unprepared(sql).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx.execute_unprepared(&format!("UPDATE module_policy_revision_cursors SET current_sequence = 6, current_revision = 'sha256:{}'", "b".repeat(64))).await.unwrap();
        tx.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
