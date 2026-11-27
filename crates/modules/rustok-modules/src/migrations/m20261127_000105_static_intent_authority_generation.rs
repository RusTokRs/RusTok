//! Durable physical enablement-intent continuity, independent of event cursors.
use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;
use crate::backend::SupportedBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE TABLE module_static_intent_generations (
    tenant_id UUID PRIMARY KEY,
    generation BIGINT NOT NULL CHECK (generation >= 0)
);
ALTER TABLE module_static_intent_generations ENABLE ROW LEVEL SECURITY;
CREATE POLICY module_static_intent_generation_scope ON module_static_intent_generations
    USING (tenant_id::text = current_setting('rustok.tenant_id', true))
    WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true));
CREATE FUNCTION module_static_intent_generation_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP IN ('DELETE', 'TRUNCATE') THEN
        RAISE EXCEPTION 'Static intent history cannot be removed' USING ERRCODE = '23514';
    ELSIF TG_OP = 'INSERT' THEN
        IF NEW.generation IS DISTINCT FROM 0 OR NEW.tenant_id = '00000000-0000-0000-0000-000000000000'::uuid THEN
            RAISE EXCEPTION 'Static intent history must start at zero' USING ERRCODE = '23514';
        END IF;
        RETURN NEW;
    END IF;
    IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id OR OLD.generation = 9223372036854775807 THEN
        RAISE EXCEPTION 'Static intent history identity or sequence exhausted' USING ERRCODE = '23514';
    END IF;
    IF NEW.generation IS DISTINCT FROM OLD.generation + 1 THEN
        RAISE EXCEPTION 'Static intent generation must advance by one' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER module_static_intent_generation_guard BEFORE INSERT OR UPDATE OR DELETE
    ON module_static_intent_generations FOR EACH ROW EXECUTE FUNCTION module_static_intent_generation_guard();
CREATE TRIGGER module_static_intent_generation_no_truncate BEFORE TRUNCATE
    ON module_static_intent_generations FOR EACH STATEMENT EXECUTE FUNCTION module_static_intent_generation_guard();

CREATE FUNCTION module_static_intent_observe_write() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE tenant UUID;
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        RAISE EXCEPTION 'Static module intent cannot be truncated' USING ERRCODE = '23514';
    ELSIF TG_OP = 'UPDATE' THEN
        IF NEW.tenant_id IS DISTINCT FROM OLD.tenant_id OR NEW.module_slug IS DISTINCT FROM OLD.module_slug THEN
            RAISE EXCEPTION 'Static module intent identity is immutable' USING ERRCODE = '23514';
        END IF;
        IF NEW.enabled IS NOT DISTINCT FROM OLD.enabled THEN RETURN NEW; END IF;
        tenant := NEW.tenant_id;
    ELSIF TG_OP = 'INSERT' THEN
        tenant := NEW.tenant_id;
    ELSE
        tenant := OLD.tenant_id;
    END IF;
    INSERT INTO module_static_intent_generations (tenant_id, generation) VALUES (tenant, 0)
        ON CONFLICT (tenant_id) DO NOTHING;
    UPDATE module_static_intent_generations SET generation = generation + 1 WHERE tenant_id = tenant;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Static intent generation unavailable' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER module_static_intent_observe_write AFTER INSERT OR UPDATE OR DELETE ON tenant_modules
    FOR EACH ROW EXECUTE FUNCTION module_static_intent_observe_write();
CREATE TRIGGER module_static_intent_no_truncate BEFORE TRUNCATE ON tenant_modules
    FOR EACH STATEMENT EXECUTE FUNCTION module_static_intent_observe_write();
"#;

const SQLITE: &str = r#"
CREATE TABLE module_static_intent_generations (
    tenant_key TEXT NOT NULL PRIMARY KEY,
    generation INTEGER NOT NULL CHECK (typeof(generation) = 'integer' AND generation >= 0)
) WITHOUT ROWID;
CREATE TRIGGER module_static_intent_generation_insert BEFORE INSERT ON module_static_intent_generations
WHEN typeof(NEW.tenant_key) <> 'text' OR length(NEW.tenant_key) <> 32
    OR NEW.tenant_key GLOB '*[^0-9a-f]*' OR NEW.tenant_key = '00000000000000000000000000000000'
    OR typeof(NEW.generation) <> 'integer' OR NEW.generation <> 0
    OR EXISTS (SELECT 1 FROM module_static_intent_generations WHERE tenant_key = NEW.tenant_key)
BEGIN SELECT RAISE(ABORT, 'Static intent history must start at zero and cannot be replaced'); END;
CREATE TRIGGER module_static_intent_generation_delete BEFORE DELETE ON module_static_intent_generations
BEGIN SELECT RAISE(ABORT, 'Static intent history cannot be removed'); END;
CREATE TRIGGER module_static_intent_generation_update BEFORE UPDATE ON module_static_intent_generations
WHEN NEW.tenant_key IS NOT OLD.tenant_key OR typeof(NEW.generation) <> 'integer'
    OR OLD.generation = 9223372036854775807 OR NEW.generation <> OLD.generation + 1
BEGIN SELECT RAISE(ABORT, 'Static intent generation must advance by one'); END;

-- REPLACE can implicitly delete a conflicting row without firing DELETE
-- triggers when recursive_triggers is OFF. Reject cross-tenant conflicts;
-- same-tenant replacements still advance that tenant's generation.
CREATE TRIGGER module_static_intent_insert_identity BEFORE INSERT ON tenant_modules
WHEN EXISTS (SELECT 1 FROM tenant_modules t
    WHERE (t.id = NEW.id OR t.rowid = NEW.rowid) AND replace(replace(replace(replace(lower(CASE WHEN typeof(t.tenant_id) = 'blob' THEN hex(t.tenant_id) ELSE CAST(t.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') <> replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN hex(NEW.tenant_id) ELSE CAST(NEW.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''))
BEGIN SELECT RAISE(ABORT, 'Cross-tenant static intent replacement'); END;
CREATE TRIGGER module_static_intent_update_identity BEFORE UPDATE ON tenant_modules
WHEN NEW.tenant_id IS NOT OLD.tenant_id OR NEW.module_slug IS NOT OLD.module_slug
    OR NEW.rowid IS NOT OLD.rowid
    OR EXISTS (SELECT 1 FROM tenant_modules t WHERE t.id = NEW.id AND replace(replace(replace(replace(lower(CASE WHEN typeof(t.tenant_id) = 'blob' THEN hex(t.tenant_id) ELSE CAST(t.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '') <> replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN hex(NEW.tenant_id) ELSE CAST(NEW.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''))
BEGIN SELECT RAISE(ABORT, 'Static intent identity cannot be rebound'); END;
CREATE TRIGGER module_static_intent_insert AFTER INSERT ON tenant_modules
BEGIN
    INSERT INTO module_static_intent_generations (tenant_key, generation)
    SELECT replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN hex(NEW.tenant_id) ELSE CAST(NEW.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''), 0 WHERE NOT EXISTS (SELECT 1 FROM module_static_intent_generations WHERE tenant_key = replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN hex(NEW.tenant_id) ELSE CAST(NEW.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''));
    UPDATE module_static_intent_generations SET generation = generation + 1 WHERE tenant_key = replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN hex(NEW.tenant_id) ELSE CAST(NEW.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '');
END;
CREATE TRIGGER module_static_intent_delete AFTER DELETE ON tenant_modules
BEGIN
    INSERT INTO module_static_intent_generations (tenant_key, generation)
    SELECT replace(replace(replace(replace(lower(CASE WHEN typeof(OLD.tenant_id) = 'blob' THEN hex(OLD.tenant_id) ELSE CAST(OLD.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''), 0 WHERE NOT EXISTS (SELECT 1 FROM module_static_intent_generations WHERE tenant_key = replace(replace(replace(replace(lower(CASE WHEN typeof(OLD.tenant_id) = 'blob' THEN hex(OLD.tenant_id) ELSE CAST(OLD.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''));
    UPDATE module_static_intent_generations SET generation = generation + 1 WHERE tenant_key = replace(replace(replace(replace(lower(CASE WHEN typeof(OLD.tenant_id) = 'blob' THEN hex(OLD.tenant_id) ELSE CAST(OLD.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '');
END;
CREATE TRIGGER module_static_intent_update AFTER UPDATE ON tenant_modules
WHEN NEW.enabled IS NOT OLD.enabled OR NEW.id IS NOT OLD.id
BEGIN
    INSERT INTO module_static_intent_generations (tenant_key, generation)
    SELECT replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN hex(NEW.tenant_id) ELSE CAST(NEW.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''), 0 WHERE NOT EXISTS (SELECT 1 FROM module_static_intent_generations WHERE tenant_key = replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN hex(NEW.tenant_id) ELSE CAST(NEW.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', ''));
    UPDATE module_static_intent_generations SET generation = generation + 1 WHERE tenant_key = replace(replace(replace(replace(lower(CASE WHEN typeof(NEW.tenant_id) = 'blob' THEN hex(NEW.tenant_id) ELSE CAST(NEW.tenant_id AS TEXT) END), 'urn:uuid:', ''), '{', ''), '}', ''), '-', '');
END;
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match SupportedBackend::new(manager.get_database_backend())? {
            SupportedBackend::Postgres => {
                db.execute_unprepared("LOCK TABLE tenant_modules IN SHARE ROW EXCLUSIVE MODE").await?;
                db.execute_unprepared(POSTGRES).await?;
            }
            SupportedBackend::Sqlite => {
                db.execute_unprepared("UPDATE tenant_modules SET enabled = enabled WHERE 1 = 0").await?;
                db.execute_unprepared(SQLITE).await?;
            }
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("static intent continuity cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SeaOrmModulePolicyRevisionConsumer;
    use sea_orm::{Database, TransactionTrait};
    use uuid::Uuid;

    #[tokio::test]
    async fn sqlite_intent_generation_tracks_aba_and_rejects_replacement() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("CREATE TABLE tenant_modules (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL, module_slug TEXT NOT NULL, enabled BOOLEAN, settings TEXT, UNIQUE(tenant_id, module_slug))").await.unwrap();
        Migration.up(&SchemaManager::new(&db)).await.unwrap();
        db.execute_unprepared("PRAGMA recursive_triggers = OFF").await.unwrap();
        let owner = SeaOrmModulePolicyRevisionConsumer::new(db.clone());
        let tenant = Uuid::new_v4();
        let other = Uuid::new_v4();
        let id = Uuid::new_v4();
        assert_eq!(owner.observe_static_intent_generation(tenant).await.unwrap(), 0);
        db.execute_unprepared(&format!("INSERT INTO tenant_modules VALUES ('{id}', '{tenant}', 'blog', 1, '{{}}')")).await.unwrap();
        let admitted = owner.observe_static_intent_generation(tenant).await.unwrap();
        db.execute_unprepared("UPDATE tenant_modules SET settings = 'changed'").await.unwrap();
        assert_eq!(owner.observe_static_intent_generation(tenant).await.unwrap(), admitted);
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("UPDATE tenant_modules SET enabled = 0").await.unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(owner.observe_static_intent_generation(tenant).await.unwrap(), admitted);
        db.execute_unprepared("UPDATE tenant_modules SET enabled = 0; UPDATE tenant_modules SET enabled = 1").await.unwrap();
        assert_eq!(owner.observe_static_intent_generation(tenant).await.unwrap(), admitted + 2);
        db.execute_unprepared(&format!("INSERT OR REPLACE INTO tenant_modules VALUES ('{id}', '{tenant}', 'blog', NULL, '{{}}')")).await.unwrap();
        let restored = owner.observe_static_intent_generation(tenant).await.unwrap();
        assert!(restored > admitted + 2);
        for sql in [
            "DELETE FROM module_static_intent_generations".to_string(),
            "UPDATE module_static_intent_generations SET generation = 0".to_string(),
            "UPDATE module_static_intent_generations SET generation = generation + 2".to_string(),
            format!("INSERT OR REPLACE INTO module_static_intent_generations VALUES ('{}', 0)", tenant.simple()),
            format!("INSERT OR REPLACE INTO tenant_modules VALUES ('{id}', '{other}', 'blog', 1, '{{}}')"),
            format!("INSERT OR REPLACE INTO tenant_modules (rowid, id, tenant_id, module_slug, enabled) SELECT rowid, '{}', '{other}', 'blog', 1 FROM tenant_modules", Uuid::new_v4()),
        ] {
            assert!(db.execute_unprepared(&sql).await.is_err(), "{sql}");
        }
        assert_eq!(owner.observe_static_intent_generation(tenant).await.unwrap(), restored);
        assert_eq!(owner.observe_static_intent_generation(other).await.unwrap(), 0);
        db.execute_unprepared("DELETE FROM tenant_modules").await.unwrap();
        assert_eq!(owner.observe_static_intent_generation(tenant).await.unwrap(), restored + 1);
    }
}
