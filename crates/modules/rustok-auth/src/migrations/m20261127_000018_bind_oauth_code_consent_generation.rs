//! Bind authorization-code issuance to the exact consent UUID. Never infer a
//! historical generation from timestamps: unbound legacy codes fail closed
//! at exchange for consent-required clients.
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = manager.get_database_backend();
        if !matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::Sqlite) {
            return Err(DbErr::Migration("unsupported OAuth consent generation backend".into()));
        }
        manager.alter_table(Table::alter()
            .table(Alias::new("oauth_authorization_codes"))
            .add_column(ColumnDef::new(Alias::new("consent_id")).uuid().null())
            .to_owned()).await?;
        // Keep the identifier as evidence even if consent history is pruned.
        // A nullable FK with SET NULL would destroy the generation binding.
        manager.get_connection().execute_unprepared(match backend {
            DatabaseBackend::Postgres => r#"
CREATE FUNCTION auth_guard_code_consent_generation() RETURNS trigger LANGUAGE plpgsql AS $guard$
BEGIN
    IF NEW.consent_id IS DISTINCT FROM OLD.consent_id THEN
        RAISE EXCEPTION 'OAuth code consent generation is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END; $guard$;
CREATE TRIGGER auth_code_consent_generation_guard BEFORE UPDATE ON oauth_authorization_codes
FOR EACH ROW EXECUTE FUNCTION auth_guard_code_consent_generation();
"#,
            DatabaseBackend::Sqlite => r#"
CREATE TRIGGER auth_code_consent_generation_guard BEFORE UPDATE ON oauth_authorization_codes
WHEN NEW.consent_id IS NOT OLD.consent_id
BEGIN SELECT RAISE(ABORT, 'OAuth code consent generation is immutable'); END;
"#,
            _ => unreachable!("backend checked"),
        }).await?;
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration("OAuth code consent generation cannot be discarded safely".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{Database, TransactionTrait};

    const ID: &str = "10000000-0000-0000-0000-000000000001";
    const CONSENT: &str = "10000000-0000-0000-0000-000000000002";

    #[tokio::test]
    async fn sqlite_code_generation_is_write_once_including_legacy_null() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared(&format!(
            "CREATE TABLE oauth_authorization_codes (id UUID PRIMARY KEY NOT NULL, used_at TIMESTAMP);
             INSERT INTO oauth_authorization_codes (id) VALUES ('{ID}')"
        )).await.unwrap();
        let tx = db.begin().await.unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.commit().await.unwrap();
        assert!(db.execute_unprepared(&format!(
            "UPDATE oauth_authorization_codes SET consent_id = '{CONSENT}'"
        )).await.is_err());
        db.execute_unprepared(&format!(
            "INSERT INTO oauth_authorization_codes (id, consent_id) VALUES ('{CONSENT}', '{CONSENT}')"
        )).await.unwrap();
        assert!(db.execute_unprepared(&format!(
            "UPDATE oauth_authorization_codes SET consent_id = NULL WHERE id = '{CONSENT}'"
        )).await.is_err());
        assert!(db.execute_unprepared(&format!(
            "UPDATE oauth_authorization_codes SET consent_id = '{ID}' WHERE id = '{CONSENT}'"
        )).await.is_err());
        db.execute_unprepared("UPDATE oauth_authorization_codes SET consent_id = consent_id, used_at = '2026-11-27'")
            .await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_AUTH_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_code_generation_is_write_once_including_legacy_null() {
        let db = Database::connect(std::env::var("RUSTOK_AUTH_TEST_DATABASE_URL").unwrap())
            .await.unwrap();
        assert_eq!(db.get_database_backend(), DatabaseBackend::Postgres);
        let schema = format!("oauth_code_generation_{}", uuid::Uuid::new_v4().simple());
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!(
            "CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema};
             CREATE TABLE oauth_authorization_codes (id UUID PRIMARY KEY, used_at TIMESTAMPTZ);
             INSERT INTO oauth_authorization_codes (id) VALUES ('{ID}')"
        )).await.unwrap();
        Migration.up(&SchemaManager::new(&tx)).await.unwrap();
        tx.execute_unprepared(&format!(
            "INSERT INTO oauth_authorization_codes (id, consent_id) VALUES ('{CONSENT}', '{CONSENT}')"
        )).await.unwrap();
        tx.commit().await.unwrap();
        for sql in [
            format!("UPDATE oauth_authorization_codes SET consent_id = '{CONSENT}' WHERE id = '{ID}'"),
            format!("UPDATE oauth_authorization_codes SET consent_id = NULL WHERE id = '{CONSENT}'"),
            format!("UPDATE oauth_authorization_codes SET consent_id = '{ID}' WHERE id = '{CONSENT}'"),
        ] {
            let tx = db.begin().await.unwrap();
            tx.execute_unprepared(&format!("SET LOCAL search_path TO {schema}")).await.unwrap();
            assert!(tx.execute_unprepared(&sql).await.is_err(), "{sql}");
            tx.rollback().await.unwrap();
        }
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared(&format!(
            "SET LOCAL search_path TO {schema};
             UPDATE oauth_authorization_codes SET consent_id = consent_id, used_at = '2026-11-27T00:00:00Z'"
        )).await.unwrap();
        tx.commit().await.unwrap();
        db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE")).await.unwrap();
    }

}
