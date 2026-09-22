use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DatabaseBackend},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE seo_translation_resource_state (
    tenant_id UUID NOT NULL,
    target_kind VARCHAR(128) NOT NULL,
    target_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    last_tx_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, target_kind, target_id),
    CONSTRAINT chk_seo_translation_resource_revision_positive CHECK (revision > 0),
    CONSTRAINT chk_seo_translation_resource_tx_nonnegative CHECK (last_tx_id >= 0),
    CONSTRAINT chk_seo_translation_resource_kind_nonblank CHECK (length(trim(target_kind)) > 0)
);

CREATE TABLE seo_translation_locale_state (
    tenant_id UUID NOT NULL,
    target_kind VARCHAR(128) NOT NULL,
    target_id UUID NOT NULL,
    locale VARCHAR(35) NOT NULL,
    revision BIGINT NOT NULL,
    last_tx_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, target_kind, target_id, locale),
    CONSTRAINT chk_seo_translation_locale_revision_positive CHECK (revision > 0),
    CONSTRAINT chk_seo_translation_locale_tx_nonnegative CHECK (last_tx_id >= 0),
    CONSTRAINT chk_seo_translation_locale_kind_nonblank CHECK (length(trim(target_kind)) > 0),
    CONSTRAINT chk_seo_translation_locale_nonblank CHECK (length(trim(locale)) > 0)
);

CREATE TABLE seo_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tx_id BIGINT NOT NULL,
    tenant_id UUID NOT NULL,
    target_kind VARCHAR(128) NOT NULL,
    target_id UUID NOT NULL,
    resource_revision VARCHAR(96) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_seo_translation_change_tx_resource
        UNIQUE (tx_id, tenant_id, target_kind, target_id),
    CONSTRAINT chk_seo_translation_change_tx_positive CHECK (tx_id > 0),
    CONSTRAINT chk_seo_translation_change_kind_nonblank CHECK (length(trim(target_kind)) > 0),
    CONSTRAINT chk_seo_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_seo_translation_change_lifecycle CHECK (lifecycle IN ('active', 'deleted'))
);

CREATE INDEX idx_seo_translation_change_tenant_seq
    ON seo_translation_change_journal (tenant_id, change_seq);
CREATE INDEX idx_seo_translation_change_resource_seq
    ON seo_translation_change_journal (tenant_id, target_kind, target_id, change_seq DESC);

-- Existing explicit SEO metadata gets stable revision state without fabricating historical changes.
INSERT INTO seo_translation_resource_state (
    tenant_id, target_kind, target_id, revision, last_tx_id
)
SELECT tenant_id, target_type, target_id, 1, 0
FROM meta
ON CONFLICT (tenant_id, target_kind, target_id) DO NOTHING;

INSERT INTO seo_translation_locale_state (
    tenant_id, target_kind, target_id, locale, revision, last_tx_id
)
SELECT meta.tenant_id, meta.target_type, meta.target_id, translation.locale, 1, 0
FROM meta_translations translation
JOIN meta ON meta.id = translation.meta_id
ON CONFLICT (tenant_id, target_kind, target_id, locale) DO NOTHING;

-- Collapse every localized mutation in one transaction into one resource revision. The journal is
-- intentionally owner-side so every canonical SEO writer, including bulk/direct service paths,
-- produces the same Translation change evidence.
CREATE OR REPLACE FUNCTION rustok_seo_bump_translation_resource(
    p_tenant_id UUID,
    p_target_kind TEXT,
    p_target_id UUID
) RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    current_tx BIGINT := txid_current();
    next_revision BIGINT;
    revision_token TEXT;
BEGIN
    INSERT INTO seo_translation_resource_state (
        tenant_id, target_kind, target_id, revision, last_tx_id, updated_at
    ) VALUES (
        p_tenant_id, p_target_kind, p_target_id, 1, current_tx, CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, target_kind, target_id)
    DO UPDATE SET
        revision = CASE
            WHEN seo_translation_resource_state.last_tx_id = EXCLUDED.last_tx_id
                THEN seo_translation_resource_state.revision
            ELSE seo_translation_resource_state.revision + 1
        END,
        last_tx_id = EXCLUDED.last_tx_id,
        updated_at = CURRENT_TIMESTAMP
    RETURNING revision INTO next_revision;

    revision_token := format('seo:%s', next_revision);
    INSERT INTO seo_translation_change_journal (
        tx_id, tenant_id, target_kind, target_id, resource_revision, lifecycle
    ) VALUES (
        current_tx, p_tenant_id, p_target_kind, p_target_id, revision_token, 'active'
    )
    ON CONFLICT (tx_id, tenant_id, target_kind, target_id)
    DO UPDATE SET
        resource_revision = EXCLUDED.resource_revision,
        lifecycle = 'active';

    RETURN revision_token;
END;
$$;

CREATE OR REPLACE FUNCTION rustok_seo_bump_translation_locale(
    p_tenant_id UUID,
    p_target_kind TEXT,
    p_target_id UUID,
    p_locale TEXT
) RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    current_tx BIGINT := txid_current();
    next_revision BIGINT;
BEGIN
    INSERT INTO seo_translation_locale_state (
        tenant_id, target_kind, target_id, locale, revision, last_tx_id, updated_at
    ) VALUES (
        p_tenant_id, p_target_kind, p_target_id, p_locale, 1, current_tx, CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, target_kind, target_id, locale)
    DO UPDATE SET
        revision = CASE
            WHEN seo_translation_locale_state.last_tx_id = EXCLUDED.last_tx_id
                THEN seo_translation_locale_state.revision
            ELSE seo_translation_locale_state.revision + 1
        END,
        last_tx_id = EXCLUDED.last_tx_id,
        updated_at = CURRENT_TIMESTAMP
    RETURNING revision INTO next_revision;

    PERFORM rustok_seo_bump_translation_resource(p_tenant_id, p_target_kind, p_target_id);
    RETURN format('seo-locale:%s', next_revision);
END;
$$;

CREATE OR REPLACE FUNCTION rustok_record_seo_translation_meta_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM rustok_seo_bump_translation_resource(NEW.tenant_id, NEW.target_type, NEW.target_id);
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        INSERT INTO seo_translation_change_journal (
            tx_id, tenant_id, target_kind, target_id, resource_revision, lifecycle
        )
        SELECT txid_current(), OLD.tenant_id, OLD.target_type, OLD.target_id,
               format('deleted:%s:%s', OLD.target_type, OLD.target_id), 'deleted'
        WHERE EXISTS (
            SELECT 1 FROM seo_translation_resource_state state
            WHERE state.tenant_id = OLD.tenant_id
              AND state.target_kind = OLD.target_type
              AND state.target_id = OLD.target_id
        )
        ON CONFLICT (tx_id, tenant_id, target_kind, target_id)
        DO UPDATE SET resource_revision = EXCLUDED.resource_revision, lifecycle = 'deleted';

        DELETE FROM seo_translation_locale_state
        WHERE tenant_id = OLD.tenant_id
          AND target_kind = OLD.target_type
          AND target_id = OLD.target_id;
        DELETE FROM seo_translation_resource_state
        WHERE tenant_id = OLD.tenant_id
          AND target_kind = OLD.target_type
          AND target_id = OLD.target_id;
        RETURN OLD;
    END IF;

    -- Non-localized explicit SEO fields (robots/canonical/structured data) are outside seo_copy.
    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.target_type IS NOT DISTINCT FROM NEW.target_type
       AND OLD.target_id IS NOT DISTINCT FROM NEW.target_id THEN
        RETURN NEW;
    END IF;

    INSERT INTO seo_translation_change_journal (
        tx_id, tenant_id, target_kind, target_id, resource_revision, lifecycle
    ) VALUES (
        txid_current(), OLD.tenant_id, OLD.target_type, OLD.target_id,
        format('deleted:%s:%s', OLD.target_type, OLD.target_id), 'deleted'
    )
    ON CONFLICT (tx_id, tenant_id, target_kind, target_id)
    DO UPDATE SET resource_revision = EXCLUDED.resource_revision, lifecycle = 'deleted';

    DELETE FROM seo_translation_locale_state
    WHERE tenant_id = OLD.tenant_id
      AND target_kind = OLD.target_type
      AND target_id = OLD.target_id;
    DELETE FROM seo_translation_resource_state
    WHERE tenant_id = OLD.tenant_id
      AND target_kind = OLD.target_type
      AND target_id = OLD.target_id;

    PERFORM rustok_seo_bump_translation_resource(NEW.tenant_id, NEW.target_type, NEW.target_id);
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_seo_translation_meta_change
AFTER INSERT OR UPDATE OR DELETE ON meta
FOR EACH ROW EXECUTE FUNCTION rustok_record_seo_translation_meta_change();

-- Serialize localized writers through the owning meta row. This is the CAS boundary shared by
-- GraphQL, bulk operations, Translation apply and direct SQL writers.
CREATE OR REPLACE FUNCTION rustok_lock_seo_translation_parent()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    parent_id UUID;
BEGIN
    parent_id := CASE WHEN TG_OP = 'DELETE' THEN OLD.meta_id ELSE NEW.meta_id END;
    PERFORM id FROM meta WHERE id = parent_id FOR UPDATE;
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER trg_seo_translation_parent_lock
BEFORE INSERT OR UPDATE OR DELETE ON meta_translations
FOR EACH ROW EXECUTE FUNCTION rustok_lock_seo_translation_parent();

CREATE OR REPLACE FUNCTION rustok_record_seo_translation_locale_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    parent_tenant UUID;
    parent_kind TEXT;
    parent_target UUID;
BEGIN
    IF TG_OP = 'UPDATE'
       AND OLD.meta_id IS NOT DISTINCT FROM NEW.meta_id
       AND OLD.locale IS NOT DISTINCT FROM NEW.locale
       AND OLD.title IS NOT DISTINCT FROM NEW.title
       AND OLD.description IS NOT DISTINCT FROM NEW.description
       AND OLD.keywords IS NOT DISTINCT FROM NEW.keywords
       AND OLD.og_title IS NOT DISTINCT FROM NEW.og_title
       AND OLD.og_description IS NOT DISTINCT FROM NEW.og_description THEN
        RETURN NEW;
    END IF;

    IF TG_OP IN ('UPDATE', 'DELETE') THEN
        SELECT tenant_id, target_type, target_id
          INTO parent_tenant, parent_kind, parent_target
        FROM meta WHERE id = OLD.meta_id;
        IF parent_tenant IS NOT NULL THEN
            DELETE FROM seo_translation_locale_state
            WHERE tenant_id = parent_tenant
              AND target_kind = parent_kind
              AND target_id = parent_target
              AND locale = OLD.locale;
            PERFORM rustok_seo_bump_translation_resource(parent_tenant, parent_kind, parent_target);
        END IF;
    END IF;

    IF TG_OP IN ('INSERT', 'UPDATE') THEN
        SELECT tenant_id, target_type, target_id
          INTO parent_tenant, parent_kind, parent_target
        FROM meta WHERE id = NEW.meta_id;
        IF parent_tenant IS NULL THEN
            RAISE EXCEPTION 'SEO Translation row % has no owning meta row %', NEW.id, NEW.meta_id;
        END IF;
        PERFORM rustok_seo_bump_translation_locale(
            parent_tenant, parent_kind, parent_target, NEW.locale
        );
    END IF;

    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER trg_seo_translation_locale_change
AFTER INSERT OR UPDATE OR DELETE ON meta_translations
FOR EACH ROW EXECUTE FUNCTION rustok_record_seo_translation_locale_change();
"#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS trg_seo_translation_locale_change ON meta_translations;
DROP TRIGGER IF EXISTS trg_seo_translation_parent_lock ON meta_translations;
DROP TRIGGER IF EXISTS trg_seo_translation_meta_change ON meta;
DROP FUNCTION IF EXISTS rustok_record_seo_translation_locale_change();
DROP FUNCTION IF EXISTS rustok_lock_seo_translation_parent();
DROP FUNCTION IF EXISTS rustok_record_seo_translation_meta_change();
DROP FUNCTION IF EXISTS rustok_seo_bump_translation_locale(UUID, TEXT, UUID, TEXT);
DROP FUNCTION IF EXISTS rustok_seo_bump_translation_resource(UUID, TEXT, UUID);
DROP TABLE IF EXISTS seo_translation_change_journal;
DROP TABLE IF EXISTS seo_translation_locale_state;
DROP TABLE IF EXISTS seo_translation_resource_state;
"#,
            )
            .await?;
        Ok(())
    }
}
