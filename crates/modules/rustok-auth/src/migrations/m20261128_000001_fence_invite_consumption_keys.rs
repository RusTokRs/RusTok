//! Nonwaiting admission to invitation receipt unique keys.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

const POSTGRES: &str = r#"
CREATE FUNCTION auth_fence_invite_consumption_digest(digest text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    IF NOT pg_try_advisory_xact_lock(hashtextextended('rustok:auth-invite-receipt:digest:' || digest, 0)) THEN
        RAISE EXCEPTION 'Auth invite receipt digest busy' USING ERRCODE = '55P03';
    END IF;
END; $$;
CREATE FUNCTION auth_guard_invite_consumption_keys() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE target_id uuid; target_digest text;
BEGIN
    IF TG_OP = 'UPDATE' THEN
        IF ROW(NEW.id, NEW.tenant_id, NEW.token_hash) IS DISTINCT FROM
           ROW(OLD.id, OLD.tenant_id, OLD.token_hash) THEN
            RAISE EXCEPTION 'Auth invite receipt identity is immutable' USING ERRCODE = '23514';
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN target_id := OLD.id; target_digest := OLD.token_hash;
    ELSE target_id := NEW.id; target_digest := NEW.token_hash; END IF;
    -- Raw UPDATE/DELETE may already hold the tuple; never wait for a key.
    PERFORM auth_fence_invite_consumption_digest(target_digest);
    IF NOT pg_try_advisory_xact_lock(hashtextextended('rustok:auth-invite-receipt:id:' || target_id::text, 0)) THEN
        RAISE EXCEPTION 'Auth invite receipt identity busy' USING ERRCODE = '55P03';
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END; $$;
CREATE TRIGGER auth_invite_consumption_keys_guard
BEFORE INSERT OR UPDATE OR DELETE ON auth_invite_consumptions
FOR EACH ROW EXECUTE FUNCTION auth_guard_invite_consumption_keys();
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager.get_connection().execute_unprepared(POSTGRES).await.map(|_| ()),
            DatabaseBackend::Sqlite => Ok(()), // serialized writer
            _ => Err(DbErr::Migration("unsupported invite receipt backend".into())),
        }
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("invite receipt key admission cannot be downgraded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, TransactionTrait};
    use std::time::Duration;

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_receipt_unique_keys_reject_contention_without_waiting() {
        let db = Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap()).await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("auth_invite_receipt_{}", uuid::Uuid::new_v4().simple());
        let id = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let tenant = uuid::Uuid::new_v4();
        let setup = db.begin().await.unwrap();
        setup.execute_unprepared(&format!(
            "CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema};
             CREATE TABLE auth_invite_consumptions (
                id UUID PRIMARY KEY, tenant_id UUID NOT NULL, token_hash TEXT NOT NULL UNIQUE, user_id UUID)"
        )).await.unwrap();
        Migration.up(&SchemaManager::new(&setup)).await.unwrap();
        setup.commit().await.unwrap();

        let owner = db.begin().await.unwrap();
        owner.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        owner.execute_unprepared(&format!(
            "INSERT INTO auth_invite_consumptions VALUES ('{id}', '{tenant}', 'digest', NULL)"
        )).await.unwrap();
        for sql in [
            format!("INSERT INTO auth_invite_consumptions VALUES ('{other}', '{tenant}', 'digest', NULL)"),
            format!("INSERT INTO auth_invite_consumptions VALUES ('{id}', '{tenant}', 'other-digest', NULL)"),
            "SELECT auth_fence_invite_consumption_digest('digest')".to_owned(),
        ] {
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            let error = tokio::time::timeout(Duration::from_secs(3), tx.execute_unprepared(&sql))
                .await.expect("receipt key must not wait").unwrap_err();
            assert_eq!(
                rustok_core::resilience::retry::PostgresTransactionConflict::classify(DatabaseBackend::Postgres, &error),
                Some(rustok_core::resilience::retry::PostgresTransactionConflict::LockUnavailable),
            );
            tx.rollback().await.unwrap();
        }
        owner.rollback().await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx.execute_unprepared("SELECT auth_fence_invite_consumption_digest('digest')").await.unwrap();
        tx.execute_unprepared(&format!("INSERT INTO auth_invite_consumptions VALUES ('{id}', '{tenant}', 'digest', NULL)")).await.unwrap();
        tx.execute_unprepared(&format!("UPDATE auth_invite_consumptions SET user_id = '{other}'")).await.unwrap();
        tx.execute_unprepared("UPDATE auth_invite_consumptions SET user_id = NULL").await.unwrap();
        tx.commit().await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        assert!(tx.execute_unprepared("UPDATE auth_invite_consumptions SET token_hash = 'replacement'").await.is_err());
        tx.rollback().await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
        tx.execute_unprepared("DELETE FROM auth_invite_consumptions").await.unwrap();
        tx.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }
}
