use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DatabaseBackend},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Durable aggregate sequencing relies on PostgreSQL transaction identity. Portable
        // owner/presentation tests continue to use SQLite, but must not pretend to provide a
        // cross-replica Translation ChangeCursor or durable apply-receipt contract.
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
CREATE TABLE alloy_script_presentation_translation_resource_state (
    tenant_id UUID NOT NULL,
    script_id UUID NOT NULL,
    revision BIGINT NOT NULL,
    last_tx_id BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (tenant_id, script_id),
    CONSTRAINT chk_alloy_script_presentation_translation_state_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_alloy_script_presentation_translation_state_script_non_nil
        CHECK (script_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_alloy_script_presentation_translation_state_revision_positive
        CHECK (revision > 0),
    CONSTRAINT chk_alloy_script_presentation_translation_state_tx_nonnegative
        CHECK (last_tx_id >= 0)
);

CREATE TABLE alloy_script_presentation_translation_change_journal (
    change_seq BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tx_id BIGINT NOT NULL,
    tenant_id UUID NOT NULL,
    script_id UUID NOT NULL,
    resource_revision VARCHAR(128) NOT NULL,
    lifecycle VARCHAR(16) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_alloy_script_presentation_translation_change_tx_resource
        UNIQUE (tx_id, tenant_id, script_id),
    CONSTRAINT chk_alloy_script_presentation_translation_change_seq_positive
        CHECK (change_seq > 0),
    CONSTRAINT chk_alloy_script_presentation_translation_change_tx_positive
        CHECK (tx_id > 0),
    CONSTRAINT chk_alloy_script_presentation_translation_change_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_alloy_script_presentation_translation_change_script_non_nil
        CHECK (script_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_alloy_script_presentation_translation_change_revision_nonblank
        CHECK (length(trim(resource_revision)) > 0),
    CONSTRAINT chk_alloy_script_presentation_translation_change_lifecycle
        CHECK (lifecycle IN ('active', 'deleted'))
);

CREATE INDEX idx_alloy_script_presentation_translation_change_tenant_seq
    ON alloy_script_presentation_translation_change_journal (tenant_id, change_seq);
CREATE INDEX idx_alloy_script_presentation_translation_change_resource_seq
    ON alloy_script_presentation_translation_change_journal (
        tenant_id, script_id, change_seq DESC
    );

CREATE TABLE alloy_script_presentation_translation_apply_receipts (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    script_id UUID NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL,
    proposal_id VARCHAR(255) NOT NULL,
    approval_receipt_id VARCHAR(255) NOT NULL,
    request_fingerprint VARCHAR(128) NOT NULL,
    source_locale VARCHAR(32) NOT NULL,
    target_locale VARCHAR(32) NOT NULL,
    expected_resource_revision VARCHAR(128) NOT NULL,
    expected_source_copy_revision BIGINT NOT NULL,
    expected_target_copy_revision BIGINT NULL,
    requested_description TEXT NULL,
    completed BOOLEAN NOT NULL DEFAULT FALSE,
    resource_revision VARCHAR(128) NULL,
    target_copy_revision BIGINT NULL,
    target_description TEXT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ NULL,
    CONSTRAINT uq_alloy_script_presentation_translation_apply_idempotency
        UNIQUE (tenant_id, idempotency_key),
    CONSTRAINT chk_alloy_script_presentation_translation_apply_id_non_nil
        CHECK (id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_alloy_script_presentation_translation_apply_tenant_non_nil
        CHECK (tenant_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_alloy_script_presentation_translation_apply_script_non_nil
        CHECK (script_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    CONSTRAINT chk_alloy_script_presentation_translation_apply_identity_nonblank
        CHECK (
            length(trim(idempotency_key)) > 0
            AND length(trim(proposal_id)) > 0
            AND length(trim(approval_receipt_id)) > 0
            AND length(trim(request_fingerprint)) > 0
            AND length(trim(source_locale)) > 0
            AND length(trim(target_locale)) > 0
            AND length(trim(expected_resource_revision)) > 0
        ),
    CONSTRAINT chk_alloy_script_presentation_translation_apply_locale_pair
        CHECK (source_locale <> target_locale),
    CONSTRAINT chk_alloy_script_presentation_translation_apply_source_revision_positive
        CHECK (expected_source_copy_revision > 0),
    CONSTRAINT chk_alloy_script_presentation_translation_apply_target_revision_positive
        CHECK (expected_target_copy_revision IS NULL OR expected_target_copy_revision > 0),
    CONSTRAINT chk_alloy_script_presentation_translation_apply_completion
        CHECK (
            (completed = FALSE
             AND resource_revision IS NULL
             AND target_copy_revision IS NULL
             AND completed_at IS NULL)
            OR
            (completed = TRUE
             AND resource_revision IS NOT NULL
             AND length(trim(resource_revision)) > 0
             AND target_copy_revision IS NOT NULL
             AND target_copy_revision > 0
             AND completed_at IS NOT NULL)
        )
);

CREATE INDEX idx_alloy_script_presentation_translation_apply_resource
    ON alloy_script_presentation_translation_apply_receipts (
        tenant_id, script_id, created_at DESC
    );

-- Existing presentation aggregates start at revision 1 without manufacturing historical
-- changes. Subsequent semantic presentation writes are the only active change source.
INSERT INTO alloy_script_presentation_translation_resource_state (
    tenant_id, script_id, revision, last_tx_id
)
SELECT DISTINCT tenant_id, script_id, 1, 0
FROM alloy_script_presentations
ON CONFLICT (tenant_id, script_id) DO NOTHING;

-- One Script presentation aggregate advances at most once per database transaction even when a
-- transaction touches more than one locale. The journal records the post-transaction aggregate
-- revision, never the operational whole-Script version.
CREATE OR REPLACE FUNCTION rustok_alloy_bump_script_presentation_translation_resource(
    p_tenant_id UUID,
    p_script_id UUID
) RETURNS TEXT
LANGUAGE plpgsql
AS $$
DECLARE
    current_tx BIGINT := txid_current();
    next_revision BIGINT;
    revision_token TEXT;
BEGIN
    INSERT INTO alloy_script_presentation_translation_resource_state (
        tenant_id, script_id, revision, last_tx_id, updated_at
    ) VALUES (
        p_tenant_id, p_script_id, 1, current_tx, CURRENT_TIMESTAMP
    )
    ON CONFLICT (tenant_id, script_id)
    DO UPDATE SET
        revision = CASE
            WHEN alloy_script_presentation_translation_resource_state.last_tx_id = EXCLUDED.last_tx_id
                THEN alloy_script_presentation_translation_resource_state.revision
            ELSE alloy_script_presentation_translation_resource_state.revision + 1
        END,
        last_tx_id = EXCLUDED.last_tx_id,
        updated_at = CURRENT_TIMESTAMP
    RETURNING revision INTO next_revision;

    revision_token := format('presentation:%s', next_revision);
    INSERT INTO alloy_script_presentation_translation_change_journal (
        tx_id, tenant_id, script_id, resource_revision, lifecycle
    ) VALUES (
        current_tx, p_tenant_id, p_script_id, revision_token, 'active'
    )
    ON CONFLICT (tx_id, tenant_id, script_id)
    DO UPDATE SET
        resource_revision = EXCLUDED.resource_revision,
        lifecycle = 'active';

    RETURN revision_token;
END;
$$;

-- Presentation rows are the complete semantic copy plane. Operational Script updates never touch
-- this trigger, and metadata-only row rewrites are ignored explicitly. Locale moves are semantic
-- because they change exact-locale inventory.
CREATE OR REPLACE FUNCTION rustok_alloy_record_script_presentation_translation_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    same_resource BOOLEAN := FALSE;
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM rustok_alloy_bump_script_presentation_translation_resource(
            NEW.tenant_id, NEW.script_id
        );
        RETURN NEW;
    END IF;

    IF OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
       AND OLD.script_id IS NOT DISTINCT FROM NEW.script_id
       AND OLD.locale IS NOT DISTINCT FROM NEW.locale
       AND OLD.description IS NOT DISTINCT FROM NEW.description THEN
        RETURN NEW;
    END IF;

    same_resource := OLD.tenant_id IS NOT DISTINCT FROM NEW.tenant_id
        AND OLD.script_id IS NOT DISTINCT FROM NEW.script_id;

    PERFORM rustok_alloy_bump_script_presentation_translation_resource(
        OLD.tenant_id, OLD.script_id
    );
    IF NOT same_resource THEN
        PERFORM rustok_alloy_bump_script_presentation_translation_resource(
            NEW.tenant_id, NEW.script_id
        );
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_alloy_script_presentation_translation_change
AFTER INSERT OR UPDATE ON alloy_script_presentations
FOR EACH ROW EXECUTE FUNCTION rustok_alloy_record_script_presentation_translation_change();

-- Canonical Script hard delete is the presentation lifecycle boundary. The tombstone is emitted
-- before locale rows cascade, coalesces with any active change already recorded in this owner
-- transaction, and removes live resource state while retaining ordered deletion evidence.
CREATE OR REPLACE FUNCTION rustok_alloy_record_script_presentation_translation_deleted()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    revision_token TEXT;
BEGIN
    PERFORM 1
    FROM alloy_script_presentation_translation_resource_state
    WHERE tenant_id = OLD.tenant_id AND script_id = OLD.id
    FOR UPDATE;

    IF NOT FOUND THEN
        RETURN OLD;
    END IF;

    revision_token := format('deleted:alloy.script_presentation:%s', OLD.id);
    INSERT INTO alloy_script_presentation_translation_change_journal (
        tx_id, tenant_id, script_id, resource_revision, lifecycle
    ) VALUES (
        txid_current(), OLD.tenant_id, OLD.id, revision_token, 'deleted'
    )
    ON CONFLICT (tx_id, tenant_id, script_id)
    DO UPDATE SET
        resource_revision = EXCLUDED.resource_revision,
        lifecycle = 'deleted';

    DELETE FROM alloy_script_presentation_translation_resource_state
    WHERE tenant_id = OLD.tenant_id AND script_id = OLD.id;

    RETURN OLD;
END;
$$;

CREATE TRIGGER trg_alloy_script_presentation_translation_deleted
BEFORE DELETE ON scripts
FOR EACH ROW EXECUTE FUNCTION rustok_alloy_record_script_presentation_translation_deleted();
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
DROP TRIGGER IF EXISTS trg_alloy_script_presentation_translation_deleted ON scripts;
DROP FUNCTION IF EXISTS rustok_alloy_record_script_presentation_translation_deleted();
DROP TRIGGER IF EXISTS trg_alloy_script_presentation_translation_change
    ON alloy_script_presentations;
DROP FUNCTION IF EXISTS rustok_alloy_record_script_presentation_translation_change();
DROP FUNCTION IF EXISTS rustok_alloy_bump_script_presentation_translation_resource(UUID, UUID);
DROP TABLE IF EXISTS alloy_script_presentation_translation_apply_receipts;
DROP TABLE IF EXISTS alloy_script_presentation_translation_change_journal;
DROP TABLE IF EXISTS alloy_script_presentation_translation_resource_state;
"#,
            )
            .await?;

        Ok(())
    }
}
