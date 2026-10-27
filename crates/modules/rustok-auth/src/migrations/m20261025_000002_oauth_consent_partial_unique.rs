use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

/// Replaces the full unique index on `oauth_consents`
/// `(app_id, user_id, tenant_id)` with a partial unique index over
/// active rows only.
///
/// Consent revocation is a soft delete (`revoked_at`), so the full index
/// made it impossible to ever consent again for the same
/// (app, user, tenant) after a revoke: the insert collided with the
/// revoked row and re-authorization failed forever. The partial index
/// keeps at most one *active* consent per triple while allowing revoked
/// history to remain. Compatible with PostgreSQL and SQLite.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP INDEX IF EXISTS idx_oauth_consents_unique;
CREATE UNIQUE INDEX idx_oauth_consents_unique
    ON oauth_consents (app_id, user_id, tenant_id)
    WHERE revoked_at IS NULL;
"#,
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Fails loudly if revoked duplicates accumulated while the partial
        // index was in effect; deduplicate `oauth_consents` before
        // downgrading rather than silently discarding consent history.
        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP INDEX IF EXISTS idx_oauth_consents_unique;
CREATE UNIQUE INDEX idx_oauth_consents_unique
    ON oauth_consents (app_id, user_id, tenant_id);
"#,
            )
            .await
            .map(|_| ())
    }
}
