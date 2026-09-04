use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Records content-free, transaction-local invalidation evidence for static
/// Settings source projections and exact localized targets. The monotonic
/// sequence is the durable repair cursor; translated/source text never enters
/// this table.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                r#"CREATE TABLE module_static_settings_changes (
                    change_seq BIGSERIAL PRIMARY KEY,
                    tenant_id UUID NOT NULL,
                    module_slug TEXT NOT NULL,
                    change_kind TEXT NOT NULL CHECK (change_kind IN ('base_projection', 'localized_target')),
                    field_id TEXT NULL CHECK (field_id IS NULL OR length(field_id) BETWEEN 1 AND 128),
                    locale TEXT NULL CHECK (locale IS NULL OR length(locale) BETWEEN 1 AND 32),
                    owner_revision BIGINT NOT NULL CHECK (owner_revision > 0),
                    target_revision BIGINT NULL CHECK (target_revision IS NULL OR target_revision > 0),
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    UNIQUE (tenant_id, module_slug, owner_revision),
                    CHECK (
                        (change_kind = 'base_projection' AND field_id IS NULL AND locale IS NULL AND target_revision IS NULL)
                        OR
                        (change_kind = 'localized_target' AND field_id IS NOT NULL AND locale IS NOT NULL AND target_revision IS NOT NULL)
                    )
                )"#,
                r#"CREATE INDEX idx_module_static_settings_changes_scope
                    ON module_static_settings_changes (tenant_id, module_slug, change_seq)"#,
                "ALTER TABLE module_static_settings_changes ENABLE ROW LEVEL SECURITY",
                r#"CREATE POLICY module_static_settings_changes_scope
                    ON module_static_settings_changes
                    USING (tenant_id::text = current_setting('rustok.tenant_id', true))
                    WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))"#,
                r#"CREATE FUNCTION rustok_log_static_settings_base_projection() RETURNS trigger
                    LANGUAGE plpgsql SECURITY DEFINER SET search_path = public AS $$
                    BEGIN
                        INSERT INTO module_static_settings_changes (
                            tenant_id, module_slug, change_kind, field_id, locale,
                            owner_revision, target_revision, created_at
                        )
                        SELECT
                            NEW.tenant_id,
                            NEW.module_slug,
                            'base_projection',
                            NULL,
                            NULL,
                            lifecycle.revision + 1,
                            NULL,
                            NOW()
                        FROM module_static_tenant_lifecycle lifecycle
                        WHERE lifecycle.tenant_id = NEW.tenant_id
                          AND lifecycle.module_slug = NEW.module_slug
                          AND lifecycle.active_idempotency_key IS NOT NULL
                        ON CONFLICT (tenant_id, module_slug, owner_revision) DO NOTHING;
                        RETURN NEW;
                    END;
                    $$"#,
                r#"CREATE TRIGGER trg_static_settings_base_projection_insert
                    AFTER INSERT ON tenant_modules
                    FOR EACH ROW EXECUTE FUNCTION rustok_log_static_settings_base_projection()"#,
                r#"CREATE TRIGGER trg_static_settings_base_projection_update
                    AFTER UPDATE OF settings ON tenant_modules
                    FOR EACH ROW EXECUTE FUNCTION rustok_log_static_settings_base_projection()"#,
                r#"CREATE FUNCTION rustok_log_static_settings_localized_target() RETURNS trigger
                    LANGUAGE plpgsql SECURITY DEFINER SET search_path = public AS $$
                    BEGIN
                        INSERT INTO module_static_settings_changes (
                            tenant_id, module_slug, change_kind, field_id, locale,
                            owner_revision, target_revision, created_at
                        ) VALUES (
                            NEW.tenant_id,
                            NEW.module_slug,
                            'localized_target',
                            NEW.field_id,
                            NEW.locale,
                            NEW.owner_revision,
                            NEW.revision,
                            NOW()
                        )
                        ON CONFLICT (tenant_id, module_slug, owner_revision) DO NOTHING;
                        RETURN NEW;
                    END;
                    $$"#,
                r#"CREATE TRIGGER trg_static_settings_localized_target_insert
                    AFTER INSERT ON module_static_localized_settings
                    FOR EACH ROW EXECUTE FUNCTION rustok_log_static_settings_localized_target()"#,
                r#"CREATE TRIGGER trg_static_settings_localized_target_update
                    AFTER UPDATE OF value, revision, owner_revision ON module_static_localized_settings
                    FOR EACH ROW EXECUTE FUNCTION rustok_log_static_settings_localized_target()"#,
            ],
            DbBackend::Sqlite => &[
                r#"CREATE TABLE module_static_settings_changes (
                    change_seq INTEGER PRIMARY KEY AUTOINCREMENT,
                    tenant_id TEXT NOT NULL,
                    module_slug TEXT NOT NULL,
                    change_kind TEXT NOT NULL CHECK (change_kind IN ('base_projection', 'localized_target')),
                    field_id TEXT NULL CHECK (field_id IS NULL OR length(field_id) BETWEEN 1 AND 128),
                    locale TEXT NULL CHECK (locale IS NULL OR length(locale) BETWEEN 1 AND 32),
                    owner_revision INTEGER NOT NULL CHECK (owner_revision > 0),
                    target_revision INTEGER NULL CHECK (target_revision IS NULL OR target_revision > 0),
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    UNIQUE (tenant_id, module_slug, owner_revision),
                    CHECK (
                        (change_kind = 'base_projection' AND field_id IS NULL AND locale IS NULL AND target_revision IS NULL)
                        OR
                        (change_kind = 'localized_target' AND field_id IS NOT NULL AND locale IS NOT NULL AND target_revision IS NOT NULL)
                    )
                )"#,
                r#"CREATE INDEX idx_module_static_settings_changes_scope
                    ON module_static_settings_changes (tenant_id, module_slug, change_seq)"#,
                r#"CREATE TRIGGER trg_static_settings_base_projection_insert
                    AFTER INSERT ON tenant_modules
                    FOR EACH ROW
                    BEGIN
                        INSERT OR IGNORE INTO module_static_settings_changes (
                            tenant_id, module_slug, change_kind, field_id, locale,
                            owner_revision, target_revision, created_at
                        )
                        SELECT
                            NEW.tenant_id,
                            NEW.module_slug,
                            'base_projection',
                            NULL,
                            NULL,
                            lifecycle.revision + 1,
                            NULL,
                            CURRENT_TIMESTAMP
                        FROM module_static_tenant_lifecycle lifecycle
                        WHERE lifecycle.tenant_id = NEW.tenant_id
                          AND lifecycle.module_slug = NEW.module_slug
                          AND lifecycle.active_idempotency_key IS NOT NULL;
                    END"#,
                r#"CREATE TRIGGER trg_static_settings_base_projection_update
                    AFTER UPDATE OF settings ON tenant_modules
                    FOR EACH ROW
                    BEGIN
                        INSERT OR IGNORE INTO module_static_settings_changes (
                            tenant_id, module_slug, change_kind, field_id, locale,
                            owner_revision, target_revision, created_at
                        )
                        SELECT
                            NEW.tenant_id,
                            NEW.module_slug,
                            'base_projection',
                            NULL,
                            NULL,
                            lifecycle.revision + 1,
                            NULL,
                            CURRENT_TIMESTAMP
                        FROM module_static_tenant_lifecycle lifecycle
                        WHERE lifecycle.tenant_id = NEW.tenant_id
                          AND lifecycle.module_slug = NEW.module_slug
                          AND lifecycle.active_idempotency_key IS NOT NULL;
                    END"#,
                r#"CREATE TRIGGER trg_static_settings_localized_target_insert
                    AFTER INSERT ON module_static_localized_settings
                    FOR EACH ROW
                    BEGIN
                        INSERT OR IGNORE INTO module_static_settings_changes (
                            tenant_id, module_slug, change_kind, field_id, locale,
                            owner_revision, target_revision, created_at
                        ) VALUES (
                            NEW.tenant_id,
                            NEW.module_slug,
                            'localized_target',
                            NEW.field_id,
                            NEW.locale,
                            NEW.owner_revision,
                            NEW.revision,
                            CURRENT_TIMESTAMP
                        );
                    END"#,
                r#"CREATE TRIGGER trg_static_settings_localized_target_update
                    AFTER UPDATE OF value, revision, owner_revision ON module_static_localized_settings
                    FOR EACH ROW
                    BEGIN
                        INSERT OR IGNORE INTO module_static_settings_changes (
                            tenant_id, module_slug, change_kind, field_id, locale,
                            owner_revision, target_revision, created_at
                        ) VALUES (
                            NEW.tenant_id,
                            NEW.module_slug,
                            'localized_target',
                            NEW.field_id,
                            NEW.locale,
                            NEW.owner_revision,
                            NEW.revision,
                            CURRENT_TIMESTAMP
                        );
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
                "DROP TRIGGER IF EXISTS trg_static_settings_localized_target_update ON module_static_localized_settings",
                "DROP TRIGGER IF EXISTS trg_static_settings_localized_target_insert ON module_static_localized_settings",
                "DROP FUNCTION IF EXISTS rustok_log_static_settings_localized_target()",
                "DROP TRIGGER IF EXISTS trg_static_settings_base_projection_update ON tenant_modules",
                "DROP TRIGGER IF EXISTS trg_static_settings_base_projection_insert ON tenant_modules",
                "DROP FUNCTION IF EXISTS rustok_log_static_settings_base_projection()",
                "DROP TABLE IF EXISTS module_static_settings_changes",
            ],
            DbBackend::Sqlite => &[
                "DROP TRIGGER IF EXISTS trg_static_settings_localized_target_update",
                "DROP TRIGGER IF EXISTS trg_static_settings_localized_target_insert",
                "DROP TRIGGER IF EXISTS trg_static_settings_base_projection_update",
                "DROP TRIGGER IF EXISTS trg_static_settings_base_projection_insert",
                "DROP TABLE IF EXISTS module_static_settings_changes",
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
