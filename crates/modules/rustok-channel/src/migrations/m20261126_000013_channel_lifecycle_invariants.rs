use sea_orm::sea_query::Expr;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

/// Enforces the channel lifecycle and target-claim invariants at the database
/// level, and retires the duplicated `channels.status` column.
///
/// Invariants made structural here:
///
/// * at most one default channel per tenant (partial unique index),
/// * at most one active resolution policy set per tenant (partial unique index),
/// * at most one primary target per channel (partial unique index),
/// * a web host is claimed by at most one channel per tenant (partial unique
///   index over the denormalized `channel_targets.tenant_id`),
/// * a target can only reference a channel of the same tenant (composite
///   foreign key on PostgreSQL),
/// * `channels.status` no longer exists: the publication state is derived from
///   `is_active`, so no writer can store a status that contradicts it.
///
/// Existing contradicting rows are repaired (or, for duplicated host claims,
/// removed keeping the oldest claim) before the indexes are created so the
/// migration is safe on databases written before the invariant existed.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                connection
                    .execute_unprepared(
                        r#"
-- A default channel must be active: the previous flag pair could disagree.
UPDATE channels SET is_default = FALSE WHERE is_default AND NOT is_active;

WITH ranked_defaults AS (
    SELECT id, ROW_NUMBER() OVER (
        PARTITION BY tenant_id ORDER BY created_at ASC, id ASC
    ) AS position
    FROM channels
    WHERE is_default
)
UPDATE channels SET is_default = FALSE
WHERE id IN (SELECT id FROM ranked_defaults WHERE position > 1);

CREATE UNIQUE INDEX IF NOT EXISTS uq_channels_tenant_default
    ON channels (tenant_id)
    WHERE is_default;

-- Target claims become tenant-scoped so exclusivity can be enforced per tenant.
ALTER TABLE channel_targets ADD COLUMN IF NOT EXISTS tenant_id UUID;

UPDATE channel_targets AS target
SET tenant_id = channel.tenant_id
FROM channels AS channel
WHERE channel.id = target.channel_id
  AND target.tenant_id IS DISTINCT FROM channel.tenant_id;

-- A host may only be claimed by one channel per tenant; keep the oldest claim.
WITH ranked_claims AS (
    SELECT id, ROW_NUMBER() OVER (
        PARTITION BY tenant_id, value ORDER BY created_at ASC, id ASC
    ) AS position
    FROM channel_targets
    WHERE target_type = 'web_domain'
      AND tenant_id IS NOT NULL
)
DELETE FROM channel_targets
WHERE id IN (SELECT id FROM ranked_claims WHERE position > 1);

-- A channel has at most one primary target.
WITH ranked_primary AS (
    SELECT id, ROW_NUMBER() OVER (
        PARTITION BY channel_id ORDER BY created_at ASC, id ASC
    ) AS position
    FROM channel_targets
    WHERE is_primary
)
UPDATE channel_targets SET is_primary = FALSE
WHERE id IN (SELECT id FROM ranked_primary WHERE position > 1);

ALTER TABLE channel_targets
    ALTER COLUMN tenant_id SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS uq_channels_tenant_id_id
    ON channels (tenant_id, id);

ALTER TABLE channel_targets
    DROP CONSTRAINT IF EXISTS fk_channel_targets_tenant_channel;

ALTER TABLE channel_targets
    ADD CONSTRAINT fk_channel_targets_tenant_channel
    FOREIGN KEY (tenant_id, channel_id)
    REFERENCES channels (tenant_id, id)
    ON DELETE CASCADE;

CREATE UNIQUE INDEX IF NOT EXISTS uq_channel_targets_tenant_host
    ON channel_targets (tenant_id, value)
    WHERE target_type = 'web_domain';

CREATE UNIQUE INDEX IF NOT EXISTS uq_channel_targets_channel_primary
    ON channel_targets (channel_id)
    WHERE is_primary;

WITH ranked_active_sets AS (
    SELECT id, ROW_NUMBER() OVER (
        PARTITION BY tenant_id ORDER BY updated_at DESC, created_at DESC, id DESC
    ) AS position
    FROM channel_resolution_policy_sets
    WHERE is_active
)
UPDATE channel_resolution_policy_sets SET is_active = FALSE
WHERE id IN (SELECT id FROM ranked_active_sets WHERE position > 1);

CREATE UNIQUE INDEX IF NOT EXISTS uq_channel_resolution_policy_sets_tenant_active
    ON channel_resolution_policy_sets (tenant_id)
    WHERE is_active;

ALTER TABLE channels DROP COLUMN IF EXISTS status;
"#,
                    )
                    .await?;
                Ok(())
            }
            DatabaseBackend::Sqlite => {
                if let Err(error) = connection
                    .execute_unprepared("ALTER TABLE channel_targets ADD COLUMN tenant_id TEXT;")
                    .await
                {
                    let message = error.to_string();
                    if !message.contains("duplicate column") {
                        return Err(error);
                    }
                }

                connection
                    .execute_unprepared(
                        r#"
UPDATE channels SET is_default = 0 WHERE is_default = 1 AND is_active = 0;

WITH ranked_defaults AS (
    SELECT id, ROW_NUMBER() OVER (
        PARTITION BY tenant_id ORDER BY created_at ASC, id ASC
    ) AS position
    FROM channels
    WHERE is_default = 1
)
UPDATE channels SET is_default = 0
WHERE id IN (SELECT id FROM ranked_defaults WHERE position > 1);

CREATE UNIQUE INDEX IF NOT EXISTS uq_channels_tenant_default
    ON channels (tenant_id)
    WHERE is_default = 1;

UPDATE channel_targets
SET tenant_id = (
    SELECT channel.tenant_id FROM channels AS channel
    WHERE channel.id = channel_targets.channel_id
);

WITH ranked_claims AS (
    SELECT id, ROW_NUMBER() OVER (
        PARTITION BY tenant_id, value ORDER BY created_at ASC, id ASC
    ) AS position
    FROM channel_targets
    WHERE target_type = 'web_domain'
      AND tenant_id IS NOT NULL
)
DELETE FROM channel_targets
WHERE id IN (SELECT id FROM ranked_claims WHERE position > 1);

WITH ranked_primary AS (
    SELECT id, ROW_NUMBER() OVER (
        PARTITION BY channel_id ORDER BY created_at ASC, id ASC
    ) AS position
    FROM channel_targets
    WHERE is_primary = 1
)
UPDATE channel_targets SET is_primary = 0
WHERE id IN (SELECT id FROM ranked_primary WHERE position > 1);

CREATE UNIQUE INDEX IF NOT EXISTS uq_channel_targets_tenant_host
    ON channel_targets (tenant_id, value)
    WHERE target_type = 'web_domain';

CREATE UNIQUE INDEX IF NOT EXISTS uq_channel_targets_channel_primary
    ON channel_targets (channel_id)
    WHERE is_primary = 1;

WITH ranked_active_sets AS (
    SELECT id, ROW_NUMBER() OVER (
        PARTITION BY tenant_id ORDER BY updated_at DESC, created_at DESC, id DESC
    ) AS position
    FROM channel_resolution_policy_sets
    WHERE is_active = 1
)
UPDATE channel_resolution_policy_sets SET is_active = 0
WHERE id IN (SELECT id FROM ranked_active_sets WHERE position > 1);

CREATE UNIQUE INDEX IF NOT EXISTS uq_channel_resolution_policy_sets_tenant_active
    ON channel_resolution_policy_sets (tenant_id)
    WHERE is_active = 1;
"#,
                    )
                    .await?;

                // SQLite only learned `ALTER TABLE ... DROP COLUMN` in 3.35 and
                // cannot add `NOT NULL`/foreign keys to an existing table. A
                // database that pre-dates it keeps the column; it is harmless
                // because no code reads or writes it any more.
                if let Err(error) = connection
                    .execute_unprepared("ALTER TABLE channels DROP COLUMN status;")
                    .await
                {
                    let message = error.to_string();
                    if !message.contains("no such column") && !message.contains("syntax error") {
                        return Err(error);
                    }
                }
                Ok(())
            }
            backend => Err(DbErr::Custom(format!(
                "rustok-channel migrations require PostgreSQL or SQLite (got {backend:?})"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = manager.get_database_backend();

        if backend == DatabaseBackend::Postgres {
            connection
                .execute_unprepared(
                    r#"
DROP INDEX IF EXISTS uq_channel_resolution_policy_sets_tenant_active;
DROP INDEX IF EXISTS uq_channel_targets_channel_primary;
DROP INDEX IF EXISTS uq_channel_targets_tenant_host;
ALTER TABLE channel_targets DROP CONSTRAINT IF EXISTS fk_channel_targets_tenant_channel;
DROP INDEX IF EXISTS uq_channels_tenant_id_id;
"#,
                )
                .await?;
        } else {
            connection
                .execute_unprepared(
                    r#"
DROP INDEX IF EXISTS uq_channel_resolution_policy_sets_tenant_active;
DROP INDEX IF EXISTS uq_channel_targets_channel_primary;
DROP INDEX IF EXISTS uq_channel_targets_tenant_host;
"#,
                )
                .await?;
        }

        connection
            .execute_unprepared("DROP INDEX IF EXISTS uq_channels_tenant_default;")
            .await?;
        // SQLite only learned `ALTER TABLE ... DROP COLUMN` in 3.35; on older
        // engines the rollback keeps the column, exactly like `up` keeps `status`.
        if let Err(error) = connection
            .execute_unprepared("ALTER TABLE channel_targets DROP COLUMN tenant_id;")
            .await
        {
            let message = error.to_string();
            if !message.contains("no such column") && !message.contains("syntax error") {
                return Err(error);
            }
        }

        manager
            .alter_table(
                Table::alter()
                    .table(Channels::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Channels::Status)
                            .string_len(50)
                            .not_null()
                            .default(Expr::value("experimental")),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Channels {
    Table,
    Status,
}
