use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

use super::shared::{Tenants, Users};

/// One active, one-time mailbox-change challenge per user and target address.
///
/// The user relation is tenant-composite. The independent user/tenant
/// references are retained for cascade semantics, while the composite
/// constraint (Postgres) or matching triggers (SQLite) prevent cross-tenant
/// challenge rows and parent tenant moves.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AuthEmailChangeRequests::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(AuthEmailChangeRequests::TenantId).uuid().not_null())
                    .col(ColumnDef::new(AuthEmailChangeRequests::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::CurrentEmail)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::NewEmail)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::PasswordChangedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::TokenHash)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AuthEmailChangeRequests::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_email_change_requests_tenant")
                            .from(AuthEmailChangeRequests::Table, AuthEmailChangeRequests::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_auth_email_change_requests_user")
                            .from(AuthEmailChangeRequests::Table, AuthEmailChangeRequests::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_email_change_requests_tenant_user")
                    .table(AuthEmailChangeRequests::Table)
                    .col(AuthEmailChangeRequests::TenantId)
                    .col(AuthEmailChangeRequests::UserId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_email_change_requests_tenant_email")
                    .table(AuthEmailChangeRequests::Table)
                    .col(AuthEmailChangeRequests::TenantId)
                    .col(AuthEmailChangeRequests::NewEmail)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_auth_email_change_requests_expires_at")
                    .table(AuthEmailChangeRequests::Table)
                    .col(AuthEmailChangeRequests::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                // m20260720_000002 creates this referenced composite unique
                // index before the email-change table migration is admitted.
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
ALTER TABLE auth_email_change_requests
    ADD CONSTRAINT fk_auth_email_change_requests_tenant_user
    FOREIGN KEY (tenant_id, user_id)
    REFERENCES users (tenant_id, id)
    ON DELETE CASCADE;
"#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                for trigger in sqlite_triggers() {
                    manager.get_connection().execute_unprepared(trigger).await?;
                }
            }
            backend => {
                return Err(DbErr::Custom(format!(
                    "Auth email-change tenant integrity does not support {backend:?}"
                )));
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            for name in [
                "trg_auth_email_change_requests_tenant_insert",
                "trg_auth_email_change_requests_tenant_update",
                "trg_auth_users_email_change_tenant_update",
            ] {
                manager
                    .get_connection()
                    .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {name}"))
                    .await?;
            }
        }

        manager
            .drop_table(Table::drop().table(AuthEmailChangeRequests::Table).to_owned())
            .await
    }
}

fn sqlite_triggers() -> [&'static str; 3] {
    [
        r#"CREATE TRIGGER trg_auth_email_change_requests_tenant_insert
           BEFORE INSERT ON auth_email_change_requests FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM users subject_user
               WHERE subject_user.id = NEW.user_id
                 AND subject_user.tenant_id = NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'Auth email change request user tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_email_change_requests_tenant_update
           BEFORE UPDATE OF tenant_id, user_id ON auth_email_change_requests FOR EACH ROW
           WHEN NOT EXISTS (
               SELECT 1 FROM users subject_user
               WHERE subject_user.id = NEW.user_id
                 AND subject_user.tenant_id = NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'Auth email change request user tenant mismatch'); END"#,
        r#"CREATE TRIGGER trg_auth_users_email_change_tenant_update
           BEFORE UPDATE OF tenant_id ON users FOR EACH ROW
           WHEN NEW.tenant_id <> OLD.tenant_id AND EXISTS (
               SELECT 1 FROM auth_email_change_requests request
               WHERE request.user_id = NEW.id
                 AND request.tenant_id <> NEW.tenant_id
           )
           BEGIN SELECT RAISE(ABORT, 'Auth user tenant update would invalidate email-change request'); END"#,
    ]
}

#[derive(Iden)]
enum AuthEmailChangeRequests {
    #[iden = "auth_email_change_requests"]
    Table,
    Id,
    TenantId,
    UserId,
    CurrentEmail,
    NewEmail,
    PasswordChangedAt,
    TokenHash,
    ExpiresAt,
    CreatedAt,
}

#[cfg(test)]
mod tests {
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database};

    use super::*;

    #[tokio::test]
    async fn sqlite_enforces_tenant_composite_email_change_ownership() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");

        for statement in [
            "CREATE TABLE tenants (id TEXT PRIMARY KEY)",
            "CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, UNIQUE (tenant_id, id))",
            "INSERT INTO tenants (id) VALUES ('tenant-1'), ('tenant-2')",
            "INSERT INTO users (id, tenant_id) VALUES ('user-1', 'tenant-1'), ('user-2', 'tenant-2')",
        ] {
            db.execute_unprepared(statement)
                .await
                .expect("fixture statement");
        }

        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("email-change migration");

        let valid = "INSERT INTO auth_email_change_requests (id, tenant_id, user_id, current_email, new_email, token_hash, expires_at) VALUES ('request-1', 'tenant-1', 'user-1', 'old@example.test', 'new@example.test', 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', '2030-01-01T00:00:00Z')";
        db.execute_unprepared(valid)
            .await
            .expect("same-tenant request");

        let cross_tenant_insert = "INSERT INTO auth_email_change_requests (id, tenant_id, user_id, current_email, new_email, token_hash, expires_at) VALUES ('request-2', 'tenant-2', 'user-1', 'old@example.test', 'other@example.test', 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb', '2030-01-01T00:00:00Z')";
        assert!(db.execute_unprepared(cross_tenant_insert).await.is_err());

        assert!(
            db.execute_unprepared(
                "UPDATE auth_email_change_requests SET tenant_id = 'tenant-2' WHERE id = 'request-1'"
            )
            .await
            .is_err()
        );
        assert!(
            db.execute_unprepared(
                "UPDATE users SET tenant_id = 'tenant-2' WHERE id = 'user-1'"
            )
            .await
            .is_err()
        );
    }
}
