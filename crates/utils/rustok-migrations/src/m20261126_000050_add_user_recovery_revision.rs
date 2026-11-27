use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared("ALTER TABLE users ADD COLUMN recovery_revision BIGINT NOT NULL DEFAULT 0 CHECK (recovery_revision >= 0)").await?;
                db.execute_unprepared(
                    r#"
                    CREATE FUNCTION auth_bump_recovery_revision() RETURNS trigger
                    LANGUAGE plpgsql AS $$
                    BEGIN
                        IF NEW.recovery_revision IS DISTINCT FROM OLD.recovery_revision THEN
                            RAISE EXCEPTION 'recovery revision is database owned';
                        END IF;
                        IF NEW.email IS DISTINCT FROM OLD.email
                            OR NEW.password_hash IS DISTINCT FROM OLD.password_hash
                            OR NEW.status IS DISTINCT FROM OLD.status
                            OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                            OR (OLD.email_verified_at IS NOT NULL AND NEW.email_verified_at IS NULL)
                        THEN
                            NEW.recovery_revision := OLD.recovery_revision + 1;
                        END IF;
                        RETURN NEW;
                    END;
                    $$"#,
                )
                .await?;
                db.execute_unprepared("CREATE TRIGGER auth_user_recovery_revision BEFORE UPDATE ON users FOR EACH ROW EXECUTE FUNCTION auth_bump_recovery_revision()").await?;
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared("ALTER TABLE users ADD COLUMN recovery_revision BIGINT NOT NULL DEFAULT 0 CHECK (typeof(recovery_revision) = 'integer' AND recovery_revision >= 0)").await?;
                db.execute_unprepared(r#"
                    CREATE TRIGGER auth_user_recovery_revision
                    AFTER UPDATE OF email, password_hash, status, tenant_id, email_verified_at ON users
                    WHEN NEW.email IS NOT OLD.email OR NEW.password_hash IS NOT OLD.password_hash
                        OR NEW.status IS NOT OLD.status OR NEW.tenant_id IS NOT OLD.tenant_id
                        OR (OLD.email_verified_at IS NOT NULL AND NEW.email_verified_at IS NULL)
                    BEGIN
                        SELECT CASE WHEN OLD.recovery_revision = 9223372036854775807
                            THEN RAISE(ABORT, 'recovery revision exhausted') END;
                        UPDATE users SET recovery_revision = OLD.recovery_revision + 1 WHERE id = NEW.id;
                    END"#).await?;
                db.execute_unprepared(
                    r#"
                    CREATE TRIGGER auth_user_recovery_revision_monotonic
                    BEFORE UPDATE OF recovery_revision ON users
                    WHEN NEW.recovery_revision < OLD.recovery_revision
                    BEGIN
                        SELECT RAISE(ABORT, 'recovery revision cannot decrease');
                    END"#,
                )
                .await?;
            }
            _ => {
                return Err(DbErr::Custom(
                    "unsupported user recovery revision backend".into(),
                ));
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared("DROP TRIGGER auth_user_recovery_revision ON users")
                    .await?;
                db.execute_unprepared("DROP FUNCTION auth_bump_recovery_revision()")
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared("DROP TRIGGER auth_user_recovery_revision")
                    .await?;
                db.execute_unprepared("DROP TRIGGER auth_user_recovery_revision_monotonic")
                    .await?;
            }
            _ => {
                return Err(DbErr::Custom(
                    "unsupported user recovery revision backend".into(),
                ));
            }
        }
        db.execute_unprepared("ALTER TABLE users DROP COLUMN recovery_revision")
            .await?;
        Ok(())
    }
}
