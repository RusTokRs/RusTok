//! Enforces tenant-composite integrity for OAuth applications and credentials.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "OAuth tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "OAuth tenant-integrity migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE UNIQUE INDEX IF NOT EXISTS uq_auth_users_tenant_id_id
    ON users (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_auth_oauth_apps_tenant_id_id
    ON oauth_apps (tenant_id, id);

ALTER TABLE oauth_tokens
    ADD CONSTRAINT fk_oauth_tokens_tenant_app
    FOREIGN KEY (tenant_id, app_id)
    REFERENCES oauth_apps (tenant_id, id)
    ON DELETE CASCADE;
ALTER TABLE oauth_tokens
    ADD CONSTRAINT fk_oauth_tokens_tenant_user
    FOREIGN KEY (tenant_id, user_id)
    REFERENCES users (tenant_id, id)
    ON DELETE CASCADE;

ALTER TABLE oauth_authorization_codes
    ADD CONSTRAINT fk_oauth_codes_tenant_app
    FOREIGN KEY (tenant_id, app_id)
    REFERENCES oauth_apps (tenant_id, id)
    ON DELETE CASCADE;
ALTER TABLE oauth_authorization_codes
    ADD CONSTRAINT fk_oauth_codes_tenant_user
    FOREIGN KEY (tenant_id, user_id)
    REFERENCES users (tenant_id, id)
    ON DELETE CASCADE;

ALTER TABLE oauth_consents
    ADD CONSTRAINT fk_oauth_consents_tenant_app
    FOREIGN KEY (tenant_id, app_id)
    REFERENCES oauth_apps (tenant_id, id)
    ON DELETE CASCADE;
ALTER TABLE oauth_consents
    ADD CONSTRAINT fk_oauth_consents_tenant_user
    FOREIGN KEY (tenant_id, user_id)
    REFERENCES users (tenant_id, id)
    ON DELETE CASCADE;

CREATE OR REPLACE FUNCTION rustok_enforce_auth_invite_user_tenant()
RETURNS trigger AS $$
BEGIN
    IF NEW.user_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM users subject_user
        WHERE subject_user.id = NEW.user_id
          AND subject_user.tenant_id = NEW.tenant_id
    ) THEN
        RAISE EXCEPTION 'Auth invite consumption user tenant mismatch'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_auth_invite_consumptions_tenant ON auth_invite_consumptions;
CREATE TRIGGER trg_auth_invite_consumptions_tenant
BEFORE INSERT OR UPDATE OF tenant_id, user_id ON auth_invite_consumptions
FOR EACH ROW EXECUTE FUNCTION rustok_enforce_auth_invite_user_tenant();
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE oauth_consents DROP CONSTRAINT IF EXISTS fk_oauth_consents_tenant_user;
ALTER TABLE oauth_consents DROP CONSTRAINT IF EXISTS fk_oauth_consents_tenant_app;
ALTER TABLE oauth_authorization_codes DROP CONSTRAINT IF EXISTS fk_oauth_codes_tenant_user;
ALTER TABLE oauth_authorization_codes DROP CONSTRAINT IF EXISTS fk_oauth_codes_tenant_app;
ALTER TABLE oauth_tokens DROP CONSTRAINT IF EXISTS fk_oauth_tokens_tenant_user;
ALTER TABLE oauth_tokens DROP CONSTRAINT IF EXISTS fk_oauth_tokens_tenant_app;
DROP TRIGGER IF EXISTS trg_auth_invite_consumptions_tenant ON auth_invite_consumptions;
DROP FUNCTION IF EXISTS rustok_enforce_auth_invite_user_tenant();
DROP INDEX IF EXISTS uq_auth_oauth_apps_tenant_id_id;
DROP INDEX IF EXISTS uq_auth_users_tenant_id_id;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    reject_existing_mismatches(manager).await?;
    for trigger in sqlite_triggers() {
        manager.get_connection().execute_unprepared(trigger).await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for name in [
        "trg_auth_oauth_tokens_tenant_insert",
        "trg_auth_oauth_tokens_tenant_update",
        "trg_auth_oauth_codes_tenant_insert",
        "trg_auth_oauth_codes_tenant_update",
        "trg_auth_oauth_consents_tenant_insert",
        "trg_auth_oauth_consents_tenant_update",
        "trg_auth_invite_consumptions_tenant_insert",
        "trg_auth_invite_consumptions_tenant_update",
        "trg_auth_oauth_apps_tenant_update",
        "trg_auth_users_oauth_tenant_update",
    ] {
        manager
            .get_connection()
            .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {name}"))
            .await?;
    }
    Ok(())
}

async fn reject_existing_mismatches(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let mismatch = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            r#"
SELECT 1
WHERE EXISTS (
    SELECT 1 FROM oauth_tokens token
    WHERE NOT EXISTS (
        SELECT 1 FROM oauth_apps app
        WHERE app.id = token.app_id AND app.tenant_id = token.tenant_id
    ) OR (token.user_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM users user
        WHERE user.id = token.user_id AND user.tenant_id = token.tenant_id
    ))
) OR EXISTS (
    SELECT 1 FROM oauth_authorization_codes code
    WHERE NOT EXISTS (
        SELECT 1 FROM oauth_apps app
        WHERE app.id = code.app_id AND app.tenant_id = code.tenant_id
    ) OR NOT EXISTS (
        SELECT 1 FROM users user
        WHERE user.id = code.user_id AND user.tenant_id = code.tenant_id
    )
) OR EXISTS (
    SELECT 1 FROM oauth_consents consent
    WHERE NOT EXISTS (
        SELECT 1 FROM oauth_apps app
        WHERE app.id = consent.app_id AND app.tenant_id = consent.tenant_id
    ) OR NOT EXISTS (
        SELECT 1 FROM users user
        WHERE user.id = consent.user_id AND user.tenant_id = consent.tenant_id
    )
) OR EXISTS (
    SELECT 1 FROM auth_invite_consumptions consumption
    WHERE consumption.user_id IS NOT NULL AND NOT EXISTS (
        SELECT 1 FROM users subject_user
        WHERE subject_user.id = consumption.user_id
          AND subject_user.tenant_id = consumption.tenant_id
    )
)
LIMIT 1
"#
            .to_string(),
        ))
        .await?;
    if mismatch.is_some() {
        return Err(DbErr::Custom(
            "OAuth tenant-integrity migration found cross-tenant relations".to_string(),
        ));
    }
    Ok(())
}

fn sqlite_triggers() -> [&'static str; 10] {
    [
        r#"CREATE TRIGGER trg_auth_oauth_tokens_tenant_insert
           BEFORE INSERT ON oauth_tokens FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM oauth_apps app
               WHERE app.id = NEW.app_id AND app.tenant_id = NEW.tenant_id
           ) OR (NEW.user_id IS NOT NULL AND NOT EXISTS (
               SELECT 1 FROM users user
               WHERE user.id = NEW.user_id AND user.tenant_id = NEW.tenant_id
           ))
           BEGIN SELECT RAISE(ABORT, 'OAuth token tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_oauth_tokens_tenant_update
           BEFORE UPDATE OF tenant_id, app_id, user_id ON oauth_tokens FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM oauth_apps app
               WHERE app.id = NEW.app_id AND app.tenant_id = NEW.tenant_id
           ) OR (NEW.user_id IS NOT NULL AND NOT EXISTS (
               SELECT 1 FROM users user
               WHERE user.id = NEW.user_id AND user.tenant_id = NEW.tenant_id
           ))
           BEGIN SELECT RAISE(ABORT, 'OAuth token tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_oauth_codes_tenant_insert
           BEFORE INSERT ON oauth_authorization_codes FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM oauth_apps app
               WHERE app.id = NEW.app_id AND app.tenant_id = NEW.tenant_id
           ) OR NOT EXISTS (
               SELECT 1 FROM users user
               WHERE user.id = NEW.user_id AND user.tenant_id = NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'OAuth authorization code tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_oauth_codes_tenant_update
           BEFORE UPDATE OF tenant_id, app_id, user_id ON oauth_authorization_codes FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM oauth_apps app
               WHERE app.id = NEW.app_id AND app.tenant_id = NEW.tenant_id
           ) OR NOT EXISTS (
               SELECT 1 FROM users user
               WHERE user.id = NEW.user_id AND user.tenant_id = NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'OAuth authorization code tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_oauth_consents_tenant_insert
           BEFORE INSERT ON oauth_consents FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM oauth_apps app
               WHERE app.id = NEW.app_id AND app.tenant_id = NEW.tenant_id
           ) OR NOT EXISTS (
               SELECT 1 FROM users user
               WHERE user.id = NEW.user_id AND user.tenant_id = NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'OAuth consent tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_oauth_consents_tenant_update
           BEFORE UPDATE OF tenant_id, app_id, user_id ON oauth_consents FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM oauth_apps app
               WHERE app.id = NEW.app_id AND app.tenant_id = NEW.tenant_id
           ) OR NOT EXISTS (
               SELECT 1 FROM users user
               WHERE user.id = NEW.user_id AND user.tenant_id = NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'OAuth consent tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_invite_consumptions_tenant_insert
           BEFORE INSERT ON auth_invite_consumptions FOR EACH ROW
           WHEN NEW.user_id IS NOT NULL AND NOT EXISTS (
               SELECT 1 FROM users subject_user
               WHERE subject_user.id = NEW.user_id AND subject_user.tenant_id = NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'Auth invite consumption user tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_invite_consumptions_tenant_update
           BEFORE UPDATE OF tenant_id, user_id ON auth_invite_consumptions FOR EACH ROW
           WHEN NEW.user_id IS NOT NULL AND NOT EXISTS (
               SELECT 1 FROM users subject_user
               WHERE subject_user.id = NEW.user_id AND subject_user.tenant_id = NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'Auth invite consumption user tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_oauth_apps_tenant_update
           BEFORE UPDATE OF tenant_id ON oauth_apps FOR EACH ROW
           WHEN NEW.tenant_id <> OLD.tenant_id AND (
               EXISTS (SELECT 1 FROM oauth_tokens WHERE app_id = NEW.id AND tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM oauth_authorization_codes WHERE app_id = NEW.id AND tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM oauth_consents WHERE app_id = NEW.id AND tenant_id <> NEW.tenant_id)
           )
           BEGIN SELECT RAISE(ABORT, 'OAuth app tenant update would invalidate relations'); END"#,
        r#"CREATE TRIGGER trg_auth_users_oauth_tenant_update
           BEFORE UPDATE OF tenant_id ON users FOR EACH ROW
           WHEN NEW.tenant_id <> OLD.tenant_id AND (
               EXISTS (SELECT 1 FROM oauth_tokens WHERE user_id = NEW.id AND tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM oauth_authorization_codes WHERE user_id = NEW.id AND tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM oauth_consents WHERE user_id = NEW.id AND tenant_id <> NEW.tenant_id)
               OR EXISTS (SELECT 1 FROM auth_invite_consumptions WHERE user_id = NEW.id AND tenant_id <> NEW.tenant_id)
           )
           BEGIN SELECT RAISE(ABORT, 'OAuth user tenant update would invalidate relations'); END"#,
    ]
}

#[cfg(test)]
mod tests {
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database};

    use super::*;

    #[tokio::test]
    async fn sqlite_rejects_cross_tenant_oauth_relations_and_parent_moves() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        for statement in [
            "CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
            "CREATE TABLE oauth_apps (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
            "CREATE TABLE oauth_tokens (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, app_id TEXT NOT NULL, user_id TEXT NULL)",
            "CREATE TABLE oauth_authorization_codes (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, app_id TEXT NOT NULL, user_id TEXT NOT NULL)",
            "CREATE TABLE oauth_consents (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, app_id TEXT NOT NULL, user_id TEXT NOT NULL)",
            "CREATE TABLE auth_invite_consumptions (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, user_id TEXT NULL)",
            "INSERT INTO users (id, tenant_id) VALUES ('user-1', 'tenant-1')",
            "INSERT INTO oauth_apps (id, tenant_id) VALUES ('app-1', 'tenant-1')",
        ] {
            db.execute_unprepared(statement)
                .await
                .expect("fixture statement");
        }

        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("tenant-integrity migration");
        db.execute_unprepared(
            "INSERT INTO oauth_tokens (id, tenant_id, app_id, user_id) VALUES ('token-1', 'tenant-1', 'app-1', 'user-1')",
        )
        .await
        .expect("same-tenant token");
        assert!(db
            .execute_unprepared(
                "INSERT INTO oauth_consents (id, tenant_id, app_id, user_id) VALUES ('consent-1', 'tenant-2', 'app-1', 'user-1')",
            )
            .await
            .is_err());
        assert!(db
            .execute_unprepared(
                "INSERT INTO auth_invite_consumptions (id, tenant_id, user_id) VALUES ('invite-1', 'tenant-2', 'user-1')",
            )
            .await
            .is_err());
        assert!(
            db.execute_unprepared(
                "UPDATE oauth_apps SET tenant_id = 'tenant-2' WHERE id = 'app-1'"
            )
            .await
            .is_err()
        );
        assert!(
            db.execute_unprepared("UPDATE users SET tenant_id = 'tenant-2' WHERE id = 'user-1'")
                .await
                .is_err()
        );
    }
}
