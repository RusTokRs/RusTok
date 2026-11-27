use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

// Do not backfill an unproved issuance epoch. Preserve accepted and
// previously revoked ledger records, and mark all remaining unconsumed
// invites revoked. The timestamp is migration metadata, not an admission
// clock sample. The same cutover rule applies on disposable schema down.
const REVOKE_UNCONSUMED: &str = "UPDATE auth_invites SET revoked_at = CURRENT_TIMESTAMP \
    WHERE revoked_at IS NULL AND NOT EXISTS (\
        SELECT 1 FROM auth_invite_consumptions c \
        WHERE c.tenant_id = auth_invites.tenant_id AND c.token_hash = auth_invites.token_hash\
    )";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let statement = match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                "ALTER TABLE auth_invites ADD COLUMN issuer_membership_revision BIGINT CHECK (issuer_membership_revision >= 0)"
            }
            DatabaseBackend::Sqlite => {
                "ALTER TABLE auth_invites ADD COLUMN issuer_membership_revision BIGINT CHECK (issuer_membership_revision IS NULL OR (typeof(issuer_membership_revision) = 'integer' AND issuer_membership_revision >= 0))"
            }
            _ => {
                return Err(DbErr::Custom(
                    "unsupported invitation membership revision backend".into(),
                ));
            }
        };
        db.execute_unprepared(statement).await?;
        db.execute_unprepared(REVOKE_UNCONSUMED).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(REVOKE_UNCONSUMED).await?;
        db.execute_unprepared("ALTER TABLE auth_invites DROP COLUMN issuer_membership_revision")
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::Database;

    #[tokio::test]
    async fn cutover_revokes_unconsumed_invites_without_erasing_history() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        for sql in [
            "CREATE TABLE auth_invites (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, token_hash TEXT NOT NULL, revoked_at TEXT)",
            "CREATE TABLE auth_invite_consumptions (tenant_id TEXT NOT NULL, token_hash TEXT NOT NULL)",
            "INSERT INTO auth_invites VALUES ('pending', 't', 'p', NULL), ('accepted', 't', 'a', NULL), ('revoked', 't', 'r', 'original')",
            "INSERT INTO auth_invite_consumptions VALUES ('t', 'a'), ('other', 'p')",
        ] {
            db.execute_unprepared(sql).await.unwrap();
        }
        let manager = SchemaManager::new(&db);
        Migration.up(&manager).await.unwrap();
        let row = db.query_one_raw(sea_orm_migration::sea_orm::Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM auth_invites WHERE (id = 'pending' AND revoked_at IS NOT NULL) OR (id = 'accepted' AND revoked_at IS NULL) OR (id = 'revoked' AND revoked_at = 'original')",
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 3);
        assert!(db.execute_unprepared("INSERT INTO auth_invites (id, tenant_id, token_hash, issuer_membership_revision) VALUES ('invalid', 't', 'i', -1)").await.is_err());
        db.execute_unprepared("INSERT INTO auth_invites (id, tenant_id, token_hash, issuer_membership_revision) VALUES ('new', 't', 'n', 0)").await.unwrap();
        Migration.down(&manager).await.unwrap();
        let row = db.query_one_raw(sea_orm_migration::sea_orm::Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT COUNT(*) AS count FROM auth_invites WHERE id = 'new' AND revoked_at IS NOT NULL",
        )).await.unwrap().unwrap();
        assert_eq!(row.try_get::<i64>("", "count").unwrap(), 1);
    }
}
