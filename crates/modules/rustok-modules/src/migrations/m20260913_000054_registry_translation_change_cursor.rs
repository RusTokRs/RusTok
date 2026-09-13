use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Append-only, content-free repair evidence for registry publish-request copy.
///
/// The journal deliberately has no foreign key back to the mutable request.
/// Deleting a request must not erase the latest cursor row or move the durable
/// highwater backwards. Published release translations are immutable snapshots
/// and are intentionally outside this journal.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                r#"CREATE TABLE registry_publish_request_translation_changes (
                    change_seq BIGSERIAL PRIMARY KEY,
                    request_id TEXT NOT NULL CHECK (length(request_id) BETWEEN 1 AND 128),
                    change_kind TEXT NOT NULL CHECK (change_kind IN ('copy', 'lifecycle')),
                    locale TEXT NULL CHECK (locale IS NULL OR length(locale) BETWEEN 1 AND 32),
                    lifecycle TEXT NULL CHECK (lifecycle IS NULL OR length(lifecycle) BETWEEN 1 AND 64),
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    CHECK (
                        (change_kind = 'copy' AND locale IS NOT NULL AND lifecycle IS NULL)
                        OR
                        (change_kind = 'lifecycle' AND locale IS NULL AND lifecycle IS NOT NULL)
                    )
                )"#,
                r#"CREATE INDEX idx_registry_publish_request_translation_changes_request
                    ON registry_publish_request_translation_changes (request_id, change_seq)"#,
                r#"INSERT INTO registry_publish_request_translation_changes
                    (request_id, change_kind, locale, lifecycle, created_at)
                    SELECT id, 'lifecycle', NULL, status, NOW()
                    FROM registry_publish_requests
                    ORDER BY id"#,
                r#"CREATE FUNCTION rustok_log_registry_publish_request_translation_copy() RETURNS trigger
                    LANGUAGE plpgsql SECURITY DEFINER SET search_path = public AS $$
                    BEGIN
                        IF TG_OP = 'DELETE' THEN
                            INSERT INTO registry_publish_request_translation_changes
                                (request_id, change_kind, locale, lifecycle, created_at)
                            VALUES (OLD.request_id, 'copy', OLD.locale, NULL, NOW());
                            RETURN OLD;
                        END IF;
                        INSERT INTO registry_publish_request_translation_changes
                            (request_id, change_kind, locale, lifecycle, created_at)
                        VALUES (NEW.request_id, 'copy', NEW.locale, NULL, NOW());
                        RETURN NEW;
                    END;
                    $$"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_insert
                    AFTER INSERT ON registry_publish_request_translations
                    FOR EACH ROW EXECUTE FUNCTION rustok_log_registry_publish_request_translation_copy()"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_update
                    AFTER UPDATE OF name, description ON registry_publish_request_translations
                    FOR EACH ROW
                    WHEN (OLD.name IS DISTINCT FROM NEW.name OR OLD.description IS DISTINCT FROM NEW.description)
                    EXECUTE FUNCTION rustok_log_registry_publish_request_translation_copy()"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_delete
                    AFTER DELETE ON registry_publish_request_translations
                    FOR EACH ROW EXECUTE FUNCTION rustok_log_registry_publish_request_translation_copy()"#,
                r#"CREATE FUNCTION rustok_log_registry_publish_request_translation_lifecycle() RETURNS trigger
                    LANGUAGE plpgsql SECURITY DEFINER SET search_path = public AS $$
                    BEGIN
                        IF TG_OP = 'DELETE' THEN
                            INSERT INTO registry_publish_request_translation_changes
                                (request_id, change_kind, locale, lifecycle, created_at)
                            VALUES (OLD.id, 'lifecycle', NULL, 'deleted', NOW());
                            RETURN OLD;
                        END IF;
                        IF NEW.status IS DISTINCT FROM OLD.status THEN
                            INSERT INTO registry_publish_request_translation_changes
                                (request_id, change_kind, locale, lifecycle, created_at)
                            VALUES (NEW.id, 'lifecycle', NULL, NEW.status, NOW());
                        END IF;
                        RETURN NEW;
                    END;
                    $$"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_lifecycle_update
                    AFTER UPDATE OF status ON registry_publish_requests
                    FOR EACH ROW EXECUTE FUNCTION rustok_log_registry_publish_request_translation_lifecycle()"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_lifecycle_delete
                    AFTER DELETE ON registry_publish_requests
                    FOR EACH ROW EXECUTE FUNCTION rustok_log_registry_publish_request_translation_lifecycle()"#,
            ],
            DbBackend::Sqlite => &[
                r#"CREATE TABLE registry_publish_request_translation_changes (
                    change_seq INTEGER PRIMARY KEY AUTOINCREMENT,
                    request_id TEXT NOT NULL CHECK (length(request_id) BETWEEN 1 AND 128),
                    change_kind TEXT NOT NULL CHECK (change_kind IN ('copy', 'lifecycle')),
                    locale TEXT NULL CHECK (locale IS NULL OR length(locale) BETWEEN 1 AND 32),
                    lifecycle TEXT NULL CHECK (lifecycle IS NULL OR length(lifecycle) BETWEEN 1 AND 64),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    CHECK (
                        (change_kind = 'copy' AND locale IS NOT NULL AND lifecycle IS NULL)
                        OR
                        (change_kind = 'lifecycle' AND locale IS NULL AND lifecycle IS NOT NULL)
                    )
                )"#,
                r#"CREATE INDEX idx_registry_publish_request_translation_changes_request
                    ON registry_publish_request_translation_changes (request_id, change_seq)"#,
                r#"INSERT INTO registry_publish_request_translation_changes
                    (request_id, change_kind, locale, lifecycle, created_at)
                    SELECT id, 'lifecycle', NULL, status, CURRENT_TIMESTAMP
                    FROM registry_publish_requests
                    ORDER BY id"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_insert
                    AFTER INSERT ON registry_publish_request_translations
                    FOR EACH ROW
                    BEGIN
                        INSERT INTO registry_publish_request_translation_changes
                            (request_id, change_kind, locale, lifecycle, created_at)
                        VALUES (NEW.request_id, 'copy', NEW.locale, NULL, CURRENT_TIMESTAMP);
                    END"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_update
                    AFTER UPDATE OF name, description ON registry_publish_request_translations
                    FOR EACH ROW
                    WHEN OLD.name IS NOT NEW.name OR OLD.description IS NOT NEW.description
                    BEGIN
                        INSERT INTO registry_publish_request_translation_changes
                            (request_id, change_kind, locale, lifecycle, created_at)
                        VALUES (NEW.request_id, 'copy', NEW.locale, NULL, CURRENT_TIMESTAMP);
                    END"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_delete
                    AFTER DELETE ON registry_publish_request_translations
                    FOR EACH ROW
                    BEGIN
                        INSERT INTO registry_publish_request_translation_changes
                            (request_id, change_kind, locale, lifecycle, created_at)
                        VALUES (OLD.request_id, 'copy', OLD.locale, NULL, CURRENT_TIMESTAMP);
                    END"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_lifecycle_update
                    AFTER UPDATE OF status ON registry_publish_requests
                    FOR EACH ROW
                    WHEN OLD.status IS NOT NEW.status
                    BEGIN
                        INSERT INTO registry_publish_request_translation_changes
                            (request_id, change_kind, locale, lifecycle, created_at)
                        VALUES (NEW.id, 'lifecycle', NULL, NEW.status, CURRENT_TIMESTAMP);
                    END"#,
                r#"CREATE TRIGGER trg_registry_publish_request_translation_lifecycle_delete
                    AFTER DELETE ON registry_publish_requests
                    FOR EACH ROW
                    BEGIN
                        INSERT INTO registry_publish_request_translation_changes
                            (request_id, change_kind, locale, lifecycle, created_at)
                        VALUES (OLD.id, 'lifecycle', NULL, 'deleted', CURRENT_TIMESTAMP);
                    END"#,
            ],
            _ => return Err(DbErr::Custom("Unsupported database backend".to_string())),
        };

        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*sql).to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_lifecycle_delete ON registry_publish_requests",
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_lifecycle_update ON registry_publish_requests",
                "DROP FUNCTION IF EXISTS rustok_log_registry_publish_request_translation_lifecycle()",
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_delete ON registry_publish_request_translations",
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_update ON registry_publish_request_translations",
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_insert ON registry_publish_request_translations",
                "DROP FUNCTION IF EXISTS rustok_log_registry_publish_request_translation_copy()",
                "DROP TABLE IF EXISTS registry_publish_request_translation_changes",
            ],
            DbBackend::Sqlite => &[
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_lifecycle_delete",
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_lifecycle_update",
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_delete",
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_update",
                "DROP TRIGGER IF EXISTS trg_registry_publish_request_translation_insert",
                "DROP TABLE IF EXISTS registry_publish_request_translation_changes",
            ],
            _ => return Err(DbErr::Custom("Unsupported database backend".to_string())),
        };

        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*sql).to_string(),
                ))
                .await?;
        }

        Ok(())
    }
}
