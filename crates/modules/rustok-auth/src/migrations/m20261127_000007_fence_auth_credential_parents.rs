//! Fail rather than wait for earlier FK parents from a late credential write.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES_GUARDS: &str = r#"
CREATE FUNCTION rustok_auth_fence_tenant_parent(target uuid) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    LOCK TABLE tenants IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM tenants WHERE id = target FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Auth tenant parent missing or invisible' USING ERRCODE = '23503';
    END IF;
END; $$;

CREATE FUNCTION rustok_auth_fence_app_parent(tenant uuid, app uuid) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    LOCK TABLE oauth_apps IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM oauth_apps WHERE tenant_id = tenant AND id = app FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'OAuth application parent missing or invisible' USING ERRCODE = '23503';
    END IF;
END; $$;

CREATE FUNCTION rustok_auth_fence_user_parent(tenant uuid, subject uuid) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    LOCK TABLE users IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM users WHERE tenant_id = tenant AND id = subject FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'Auth user parent missing or invisible' USING ERRCODE = '23503';
    END IF;
END; $$;

CREATE FUNCTION rustok_auth_fence_oauth_app_tenant() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF NEW.tenant_id IS NOT DISTINCT FROM OLD.tenant_id THEN RETURN NEW; END IF;
    END IF;
    PERFORM rustok_auth_fence_tenant_parent(NEW.tenant_id);
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'OAuth application tenant fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER trg_auth_oauth_app_tenant_fence
BEFORE INSERT OR UPDATE OF tenant_id ON oauth_apps
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_oauth_app_tenant();

CREATE FUNCTION rustok_auth_fence_oauth_credential_parents() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.tenant_id, NEW.app_id, NEW.user_id) IS NOT DISTINCT FROM
           ROW(OLD.tenant_id, OLD.app_id, OLD.user_id) THEN RETURN NEW; END IF;
    END IF;
    -- Canonical credential writers already serialize on the application
    -- and, for delegated credentials, the user. Never wait for a parent
    -- here: a direct child UPDATE may already own the credential row.
    PERFORM rustok_auth_fence_tenant_parent(NEW.tenant_id);
    PERFORM rustok_auth_fence_app_parent(NEW.tenant_id, NEW.app_id);
    IF NEW.user_id IS NOT NULL THEN
        PERFORM rustok_auth_fence_user_parent(NEW.tenant_id, NEW.user_id);
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'OAuth credential parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER trg_auth_oauth_credential_parent_fence
BEFORE INSERT OR UPDATE OF tenant_id, app_id, user_id ON oauth_tokens
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_oauth_credential_parents();
CREATE TRIGGER trg_auth_oauth_credential_parent_fence
BEFORE INSERT OR UPDATE OF tenant_id, app_id, user_id ON oauth_authorization_codes
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_oauth_credential_parents();
CREATE TRIGGER trg_auth_oauth_credential_parent_fence
BEFORE INSERT OR UPDATE OF tenant_id, app_id, user_id ON oauth_consents
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_oauth_credential_parents();

CREATE FUNCTION rustok_auth_fence_oauth_family_root() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- The existing lineage guard owns immutable identity and validates the
    -- complete root shape. Retain its exact FK parent before inserting a
    -- descendant, rather than first waiting at the deferred COMMIT check.
    -- A self-root has no pre-existing parent to lock.
    IF NEW.family_id = NEW.id THEN RETURN NEW; END IF;
    LOCK TABLE oauth_tokens IN ROW SHARE MODE NOWAIT;
    PERFORM 1 FROM oauth_tokens WHERE id = NEW.family_id FOR KEY SHARE NOWAIT;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'OAuth family root missing or invisible' USING ERRCODE = '23503';
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'OAuth family root fence busy' USING ERRCODE = '55P03';
END; $$;
-- Lexical trigger order: credential parents, family root, lineage validation.
CREATE TRIGGER trg_auth_oauth_family_root_fence
BEFORE INSERT ON oauth_tokens
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_oauth_family_root();

CREATE FUNCTION rustok_auth_fence_oauth_translation_parent() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.tenant_id, NEW.app_id) IS NOT DISTINCT FROM
           ROW(OLD.tenant_id, OLD.app_id) THEN RETURN NEW; END IF;
    END IF;
    PERFORM rustok_auth_fence_app_parent(NEW.tenant_id, NEW.app_id);
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'OAuth translation parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER trg_auth_oauth_translation_parent_fence
BEFORE INSERT OR UPDATE OF tenant_id, app_id ON oauth_app_translations
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_oauth_translation_parent();

CREATE FUNCTION rustok_auth_fence_account_child_parents() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    subject uuid;
    previous_subject uuid;
    tenant_changed boolean := true;
BEGIN
    IF TG_TABLE_NAME = 'auth_invites' THEN
        subject := NEW.issued_by;
        IF TG_OP = 'UPDATE' THEN previous_subject := OLD.issued_by; END IF;
    ELSE
        subject := NEW.user_id;
        IF TG_OP = 'UPDATE' THEN previous_subject := OLD.user_id; END IF;
    END IF;
    IF TG_OP = 'UPDATE' THEN
        tenant_changed := NEW.tenant_id IS DISTINCT FROM OLD.tenant_id;
        IF NOT tenant_changed AND subject IS NOT DISTINCT FROM previous_subject THEN RETURN NEW; END IF;
    END IF;
    -- A user DELETE SET NULL cascade has no new subject FK. In particular,
    -- it must not discover a new tenant lock under the deleted user.
    IF tenant_changed THEN
        PERFORM rustok_auth_fence_tenant_parent(NEW.tenant_id);
    END IF;
    IF subject IS NOT NULL THEN
        PERFORM rustok_auth_fence_user_parent(NEW.tenant_id, subject);
    END IF;
    RETURN NEW;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Auth account child parent fence busy' USING ERRCODE = '55P03';
END; $$;
CREATE TRIGGER trg_auth_account_child_parent_fence
BEFORE INSERT OR UPDATE OF tenant_id, issued_by ON auth_invites
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_account_child_parents();
CREATE TRIGGER trg_auth_account_child_parent_fence
BEFORE INSERT OR UPDATE OF tenant_id, user_id ON auth_invite_consumptions
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_account_child_parents();
CREATE TRIGGER trg_auth_account_child_parent_fence
BEFORE INSERT OR UPDATE OF tenant_id, user_id ON auth_email_change_requests
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_account_child_parents();
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
            DatabaseBackend::Sqlite => {
                // Existing tenant and lineage guards remain in force;
                // SQLite serializes writers.
            }
            _ => return Err(DbErr::Custom("Unsupported Auth parent fence database".into())),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => Err(DbErr::Migration(
                "Auth credential parent lock order cannot be downgraded safely".into(),
            )),
            DatabaseBackend::Sqlite => Ok(()),
            _ => Err(DbErr::Custom("Unsupported Auth parent fence database".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DatabaseTransaction, IsolationLevel, TransactionTrait};
    use std::time::Duration;
    use uuid::Uuid;

    const T: &str = "10000000-0000-0000-0000-000000000001";
    const A: &str = "20000000-0000-0000-0000-000000000001";
    const U: &str = "30000000-0000-0000-0000-000000000001";
    const R: &str = "40000000-0000-0000-0000-000000000001";
    const C: &str = "40000000-0000-0000-0000-000000000002";

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin_with_config(Some(IsolationLevel::ReadCommitted), None).await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    async fn reject_busy(db: &DatabaseConnection, schema: &str, sql: &str, message: &str) {
        let writer = tx(db, schema).await;
        let error = tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(sql))
            .await.expect("late Auth parent acquisition must not wait")
            .expect_err("contended parent must reject the write");
        assert!(error.to_string().contains(message), "{error}");
        writer.rollback().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_auth_credential_parent_guards_reject_late_fk_waits() {
        let db = Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("auth_parent_fence_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        setup.execute_unprepared(&format!(
            "CREATE TABLE tenants (id UUID PRIMARY KEY);
             CREATE TABLE users (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, UNIQUE (tenant_id, id));
             CREATE TABLE oauth_apps (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, UNIQUE (tenant_id, id));
             CREATE TABLE oauth_tokens (
                id UUID PRIMARY KEY, family_id UUID, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                app_id UUID NOT NULL, user_id UUID, authorization_code_id UUID, consent_granted_at TIMESTAMPTZ,
                grant_type TEXT NOT NULL DEFAULT 'authorization_code', token_kind TEXT NOT NULL DEFAULT 'refresh',
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, revoked_at TIMESTAMPTZ,
                FOREIGN KEY (tenant_id, app_id) REFERENCES oauth_apps(tenant_id, id) ON DELETE CASCADE,
                FOREIGN KEY (tenant_id, user_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE);
             CREATE TABLE oauth_authorization_codes (
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                app_id UUID NOT NULL, user_id UUID NOT NULL,
                FOREIGN KEY (tenant_id, app_id) REFERENCES oauth_apps(tenant_id, id) ON DELETE CASCADE,
                FOREIGN KEY (tenant_id, user_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE);
             CREATE TABLE oauth_consents (LIKE oauth_authorization_codes INCLUDING ALL);
             ALTER TABLE oauth_consents ADD FOREIGN KEY (tenant_id, app_id) REFERENCES oauth_apps(tenant_id, id) ON DELETE CASCADE;
             ALTER TABLE oauth_consents ADD FOREIGN KEY (tenant_id, user_id) REFERENCES users(tenant_id, id) ON DELETE CASCADE;
             CREATE TABLE oauth_app_translations (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, app_id UUID NOT NULL,
                FOREIGN KEY (tenant_id, app_id) REFERENCES oauth_apps(tenant_id, id) ON UPDATE CASCADE ON DELETE CASCADE);
             CREATE TABLE auth_invites (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, issued_by UUID REFERENCES users(id) ON DELETE SET NULL);
             CREATE TABLE auth_invite_consumptions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, user_id UUID REFERENCES users(id) ON DELETE SET NULL);
             CREATE TABLE auth_email_change_requests (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE);
             INSERT INTO tenants VALUES ('{T}');
             INSERT INTO users VALUES ('{U}', '{T}');
             INSERT INTO oauth_apps VALUES ('{A}', '{T}');
             INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id, user_id, revoked_at)
                VALUES ('{R}', '{R}', '{T}', '{A}', '{U}', CURRENT_TIMESTAMP);"
        )).await.unwrap();
        super::super::m20261126_000060_enforce_oauth_family_identity::Migration
            .up(&SchemaManager::new(&setup)).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        let child = format!("INSERT INTO oauth_tokens (id, family_id, tenant_id, app_id, user_id) VALUES ('{C}', '{R}', '{T}', '{A}', '{U}')");
        let code = format!("INSERT INTO oauth_authorization_codes VALUES ('{C}', '{T}', '{A}', '{U}')");
        let consent = format!("INSERT INTO oauth_consents VALUES ('{C}', '{T}', '{A}', '{U}')");
        let translation = format!("INSERT INTO oauth_app_translations VALUES ('{C}', '{T}', '{A}')");
        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT id FROM tenants WHERE id = '{T}' FOR UPDATE")).await.unwrap();
        for sql in [&child, &code, &consent] {
            reject_busy(&db, &schema, sql, "OAuth credential parent fence busy").await;
        }
        reject_busy(&db, &schema, &format!("INSERT INTO oauth_apps VALUES ('{C}', '{T}')"), "OAuth application tenant fence busy").await;
        owner.rollback().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT id FROM oauth_apps WHERE id = '{A}' FOR UPDATE")).await.unwrap();
        for sql in [&child, &code, &consent] {
            reject_busy(&db, &schema, sql, "OAuth credential parent fence busy").await;
        }
        reject_busy(&db, &schema, &translation, "OAuth translation parent fence busy").await;
        owner.rollback().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT id FROM users WHERE id = '{U}' FOR UPDATE")).await.unwrap();
        for sql in [&child, &code, &consent] {
            reject_busy(&db, &schema, sql, "OAuth credential parent fence busy").await;
        }
        for table in ["auth_invites", "auth_invite_consumptions", "auth_email_change_requests"] {
            reject_busy(&db, &schema, &format!("INSERT INTO {table} VALUES ('{C}', '{T}', '{U}')"), "Auth account child parent fence busy").await;
        }
        owner.rollback().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT id FROM oauth_tokens WHERE id = '{R}' FOR UPDATE")).await.unwrap();
        reject_busy(&db, &schema, &child, "OAuth family root fence busy").await;
        owner.rollback().await.unwrap();

        // Canonical lock reuse, a successful retry, and metadata-only writes.
        let writer = tx(&db, &schema).await;
        writer.execute_unprepared(&format!(
            "SELECT id FROM tenants WHERE id = '{T}' FOR SHARE;
             SELECT id FROM oauth_apps WHERE id = '{A}' FOR UPDATE;
             SELECT id FROM users WHERE id = '{U}' FOR UPDATE;"
        )).await.unwrap();
        for sql in [&child, &code, &consent, &translation] {
            writer.execute_unprepared(sql).await.unwrap();
        }
        for table in ["auth_invites", "auth_invite_consumptions", "auth_email_change_requests"] {
            writer.execute_unprepared(&format!("INSERT INTO {table} VALUES ('{C}', '{T}', '{U}')")).await.unwrap();
        }
        writer.commit().await.unwrap();
        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT id FROM oauth_apps WHERE id = '{A}' FOR UPDATE")).await.unwrap();
        let writer = tx(&db, &schema).await;
        tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(&format!(
            "UPDATE oauth_tokens SET revoked_at = CURRENT_TIMESTAMP WHERE id = '{C}'"
        ))).await.unwrap().unwrap();
        writer.commit().await.unwrap();
        owner.rollback().await.unwrap();

        // Existing owner cascades must not acquire a new parent for SET NULL.
        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("DELETE FROM users WHERE id = '{U}'")).await.unwrap();
        owner.commit().await.unwrap();
        let row = db.query_one_raw(sea_orm_migration::sea_orm::Statement::from_string(
            DatabaseBackend::Postgres, format!("SELECT (SELECT COUNT(*) FROM {schema}.oauth_tokens) AS tokens, (SELECT COUNT(*) FROM {schema}.auth_invites WHERE issued_by IS NULL) AS invites, (SELECT COUNT(*) FROM {schema}.auth_invite_consumptions WHERE user_id IS NULL) AS receipts")
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "tokens").unwrap(), 0);
        assert_eq!(row.try_get::<i64>("", "invites").unwrap(), 1);
        assert_eq!(row.try_get::<i64>("", "receipts").unwrap(), 1);
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
