//! Nonwaiting Auth-child admission before native PostgreSQL FK cascades.
//! Parent deletion itself and other owners' children retain their contracts.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION rustok_auth_require_cascade_visibility(relations regclass[]) RETURNS void
LANGUAGE plpgsql AS $$
DECLARE relation_id regclass;
BEGIN
    FOREACH relation_id IN ARRAY relations LOOP
        IF row_security_active(relation_id) THEN
            RAISE EXCEPTION 'Auth cascade requires complete child visibility' USING ERRCODE = '42501';
        END IF;
    END LOOP;
END; $$;

CREATE FUNCTION rustok_auth_fence_parent_cascade() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    -- DELETE already owns the parent row. New FK references must acquire
    -- that row's key share; the existing child admission guards also use
    -- NOWAIT. Do not wait for a child that may own an earlier owner fence.
    IF TG_TABLE_NAME = 'users' THEN
        LOCK TABLE sessions, auth_invites, auth_invite_consumptions,
            auth_email_change_requests, oauth_tokens, oauth_authorization_codes,
            oauth_consents IN ROW EXCLUSIVE MODE NOWAIT;
        PERFORM rustok_auth_require_cascade_visibility(ARRAY[
            'sessions'::regclass, 'auth_invites'::regclass,
            'auth_invite_consumptions'::regclass, 'auth_email_change_requests'::regclass,
            'oauth_tokens'::regclass, 'oauth_authorization_codes'::regclass,
            'oauth_consents'::regclass
        ]);
        -- Match even the single-column legacy FK actions. UUID identity
        -- is global; tenant-composite constraints remain independently enforced.
        PERFORM 1 FROM sessions WHERE user_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM auth_invites WHERE issued_by = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM auth_invite_consumptions WHERE user_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM auth_email_change_requests WHERE user_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM oauth_tokens WHERE user_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM oauth_authorization_codes WHERE user_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM oauth_consents WHERE user_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
    ELSIF TG_TABLE_NAME = 'oauth_apps' THEN
        LOCK TABLE oauth_tokens, oauth_authorization_codes, oauth_consents,
            oauth_app_translations IN ROW EXCLUSIVE MODE NOWAIT;
        PERFORM rustok_auth_require_cascade_visibility(ARRAY[
            'oauth_tokens'::regclass, 'oauth_authorization_codes'::regclass,
            'oauth_consents'::regclass, 'oauth_app_translations'::regclass
        ]);
        PERFORM 1 FROM oauth_tokens WHERE app_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM oauth_authorization_codes WHERE app_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM oauth_consents WHERE app_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
        PERFORM 1 FROM oauth_app_translations WHERE app_id = OLD.id ORDER BY id FOR UPDATE NOWAIT;
    ELSIF TG_TABLE_NAME = 'sessions' THEN
        LOCK TABLE session_refresh_history IN ROW EXCLUSIVE MODE NOWAIT;
        PERFORM rustok_auth_require_cascade_visibility(ARRAY['session_refresh_history'::regclass]);
        PERFORM 1 FROM session_refresh_history WHERE session_id = OLD.id
            ORDER BY token_hash FOR UPDATE NOWAIT;
    ELSE
        RAISE EXCEPTION 'Unknown Auth cascade parent' USING ERRCODE = '23514';
    END IF;
    -- Native FK actions own all deletes and SET NULL writes.
    RETURN OLD;
EXCEPTION WHEN lock_not_available THEN
    RAISE EXCEPTION 'Auth parent cascade child fence busy' USING ERRCODE = '55P03';
END; $$;

CREATE TRIGGER auth_user_cascade_fence BEFORE DELETE ON users
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_parent_cascade();
CREATE TRIGGER auth_oauth_app_cascade_fence BEFORE DELETE ON oauth_apps
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_parent_cascade();
CREATE TRIGGER auth_session_cascade_fence BEFORE DELETE ON sessions
FOR EACH ROW EXECUTE FUNCTION rustok_auth_fence_parent_cascade();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager.get_connection().execute_unprepared(POSTGRES).await?;
                Ok(())
            }
            DatabaseBackend::Sqlite => Ok(()), // one writer; existing FK actions
            _ => Err(DbErr::Migration("unsupported Auth cascade fence database".into())),
        }
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("Auth parent cascade fence cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait};
    use std::time::Duration;
    use uuid::Uuid;

    const T: &str = "10000000-0000-0000-0000-000000000001";
    const U: &str = "20000000-0000-0000-0000-000000000001";
    const OTHER: &str = "20000000-0000-0000-0000-000000000002";
    const A: &str = "30000000-0000-0000-0000-000000000001";
    const B: &str = "30000000-0000-0000-0000-000000000002";
    const R: &str = "40000000-0000-0000-0000-000000000001";
    const C: &str = "40000000-0000-0000-0000-000000000002";
    const SERVICE: &str = "40000000-0000-0000-0000-000000000003";
    const X: &str = "40000000-0000-0000-0000-000000000004";

    async fn tx(db: &DatabaseConnection, schema: &str) -> DatabaseTransaction {
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx
    }

    async fn count(tx: &DatabaseTransaction, table: &str, condition: &str) -> i64 {
        tx.query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            format!("SELECT COUNT(*) AS count FROM {table} WHERE {condition}"),
        )).await.unwrap().unwrap().try_get("", "count").unwrap()
    }

    async fn reject_busy(db: &DatabaseConnection, schema: &str, lock: &str, deletion: &str) {
        let owner = tx(db, schema).await;
        owner.execute_unprepared(lock).await.unwrap();
        let writer = tx(db, schema).await;
        let error = tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(deletion))
            .await.expect("cascade child fence must not wait")
            .expect_err("busy exact child must reject deletion");
        assert_eq!(
            rustok_core::resilience::retry::PostgresTransactionConflict::classify(DatabaseBackend::Postgres, &error),
            Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable),
        );
        writer.rollback().await.unwrap();
        owner.rollback().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_auth_parent_cascades_reject_busy_exact_children() {
        let db = Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("auth_cascade_{}", Uuid::new_v4().simple());
        db.execute_unprepared(&format!("CREATE SCHEMA {schema}")).await.unwrap();
        let setup = tx(&db, &schema).await;
        setup.execute_unprepared(&format!(
            "CREATE TABLE tenants (id UUID PRIMARY KEY);
             CREATE TABLE users (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, UNIQUE (tenant_id,id));
             CREATE TABLE oauth_apps (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, UNIQUE (tenant_id,id));
             CREATE TABLE sessions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                 user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE);
             CREATE TABLE session_refresh_history (token_hash TEXT PRIMARY KEY, session_id UUID NOT NULL REFERENCES sessions(id) ON DELETE CASCADE);
             CREATE TABLE auth_invites (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                 issued_by UUID REFERENCES users(id) ON DELETE SET NULL);
             CREATE TABLE auth_invite_consumptions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                 user_id UUID REFERENCES users(id) ON DELETE SET NULL);
             CREATE TABLE auth_email_change_requests (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                 user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE);
             CREATE TABLE oauth_tokens (id UUID PRIMARY KEY, family_id UUID NOT NULL REFERENCES oauth_tokens(id) DEFERRABLE INITIALLY DEFERRED,
                 tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE, app_id UUID NOT NULL, user_id UUID,
                 FOREIGN KEY (tenant_id,app_id) REFERENCES oauth_apps(tenant_id,id) ON DELETE CASCADE,
                 FOREIGN KEY (tenant_id,user_id) REFERENCES users(tenant_id,id) ON DELETE CASCADE);
             CREATE TABLE oauth_authorization_codes (id UUID PRIMARY KEY, tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
                 app_id UUID NOT NULL, user_id UUID NOT NULL,
                 FOREIGN KEY (tenant_id,app_id) REFERENCES oauth_apps(tenant_id,id) ON DELETE CASCADE,
                 FOREIGN KEY (tenant_id,user_id) REFERENCES users(tenant_id,id) ON DELETE CASCADE);
             CREATE TABLE oauth_consents (LIKE oauth_authorization_codes INCLUDING ALL);
             ALTER TABLE oauth_consents ADD FOREIGN KEY (tenant_id,app_id) REFERENCES oauth_apps(tenant_id,id) ON DELETE CASCADE;
             ALTER TABLE oauth_consents ADD FOREIGN KEY (tenant_id,user_id) REFERENCES users(tenant_id,id) ON DELETE CASCADE;
             CREATE TABLE oauth_app_translations (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, app_id UUID NOT NULL,
                 FOREIGN KEY (tenant_id,app_id) REFERENCES oauth_apps(tenant_id,id) ON DELETE CASCADE);
             INSERT INTO tenants VALUES ('{T}');
             INSERT INTO users VALUES ('{U}','{T}'), ('{OTHER}','{T}');
             INSERT INTO oauth_apps VALUES ('{A}','{T}'), ('{B}','{T}');
             INSERT INTO sessions VALUES ('{R}','{T}','{U}'), ('{X}','{T}','{OTHER}');
             INSERT INTO session_refresh_history VALUES ('target','{R}'), ('other','{X}');
             INSERT INTO auth_invites VALUES ('{R}','{T}','{U}');
             INSERT INTO auth_invite_consumptions VALUES ('{R}','{T}','{U}');
             INSERT INTO auth_email_change_requests VALUES ('{R}','{T}','{U}');
             INSERT INTO oauth_tokens VALUES ('{R}','{R}','{T}','{A}','{U}'),
                 ('{C}','{R}','{T}','{A}','{U}'), ('{SERVICE}','{SERVICE}','{T}','{A}',NULL),
                 ('{X}','{X}','{T}','{B}','{OTHER}');
             INSERT INTO oauth_authorization_codes VALUES ('{R}','{T}','{A}','{U}');
             INSERT INTO oauth_consents VALUES ('{R}','{T}','{A}','{U}');
             INSERT INTO oauth_app_translations VALUES ('{R}','{T}','{A}'), ('{X}','{T}','{B}');"
        )).await.unwrap();
        super::super::m20261127_000007_fence_auth_credential_parents::Migration
            .up(&SchemaManager::new(&setup)).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        let delete_user = format!("DELETE FROM users WHERE id = '{U}'");
        let delete_app = format!("DELETE FROM oauth_apps WHERE id = '{A}'");
        let delete_session = format!("DELETE FROM sessions WHERE id = '{R}'");
        for table in ["sessions", "auth_invites", "auth_invite_consumptions",
            "auth_email_change_requests", "oauth_tokens", "oauth_authorization_codes", "oauth_consents"] {
            reject_busy(&db, &schema, &format!("SELECT id FROM {table} WHERE id = '{R}' FOR UPDATE"), &delete_user).await;
        }
        for table in ["oauth_tokens", "oauth_authorization_codes", "oauth_consents", "oauth_app_translations"] {
            reject_busy(&db, &schema, &format!("SELECT id FROM {table} WHERE id = '{R}' FOR UPDATE"), &delete_app).await;
            reject_busy(&db, &schema, &format!("LOCK TABLE {table} IN SHARE MODE"), &delete_app).await;
        }
        for deletion in [&delete_user, &delete_session] {
            reject_busy(&db, &schema, "SELECT token_hash FROM session_refresh_history WHERE token_hash = 'target' FOR UPDATE", deletion).await;
            reject_busy(&db, &schema, "LOCK TABLE session_refresh_history IN SHARE MODE", deletion).await;
        }

        // Failed attempts and an explicit rollback keep the complete graph.
        let writer = tx(&db, &schema).await;
        assert_eq!(count(&writer, "users", "TRUE").await, 2);
        assert_eq!(count(&writer, "oauth_tokens", "TRUE").await, 4);
        writer.execute_unprepared(&delete_user).await.unwrap();
        assert_eq!(count(&writer, "oauth_tokens", "TRUE").await, 2);
        assert_eq!(count(&writer, "session_refresh_history", "TRUE").await, 1);
        writer.rollback().await.unwrap();

        // An unrelated subject and the same app's service credential must
        // neither block nor be deleted by the user cascade.
        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT id FROM oauth_tokens WHERE id IN ('{X}','{SERVICE}') FOR UPDATE")).await.unwrap();
        let writer = tx(&db, &schema).await;
        tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(&delete_user))
            .await.expect("unrelated rows must not block user deletion").unwrap();
        assert_eq!(count(&writer, "auth_invites", "issued_by IS NULL").await, 1);
        assert_eq!(count(&writer, "auth_invite_consumptions", "user_id IS NULL").await, 1);
        assert_eq!(count(&writer, "auth_email_change_requests", "TRUE").await, 0);
        writer.commit().await.unwrap();
        owner.rollback().await.unwrap();

        let owner = tx(&db, &schema).await;
        owner.execute_unprepared(&format!("SELECT id FROM oauth_tokens WHERE id = '{X}' FOR UPDATE")).await.unwrap();
        let writer = tx(&db, &schema).await;
        tokio::time::timeout(Duration::from_secs(3), writer.execute_unprepared(&delete_app))
            .await.expect("unrelated app must not block deletion").unwrap();
        assert_eq!(count(&writer, "oauth_tokens", "TRUE").await, 1);
        assert_eq!(count(&writer, "oauth_app_translations", "TRUE").await, 1);
        assert_eq!(count(&writer, "sessions", "TRUE").await, 1);
        assert_eq!(count(&writer, "session_refresh_history", "token_hash = 'other'").await, 1);
        writer.commit().await.unwrap();
        owner.rollback().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
