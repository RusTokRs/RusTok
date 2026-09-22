use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(TenantLocales::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(TenantLocales::PolicyRevision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(TenantLocales::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(TenantLocales::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            // SQLite cannot add a column with a non-constant
                            // CURRENT_TIMESTAMP default. Existing rows use a
                            // sentinel; owner writes always set the real time.
                            .default("1970-01-01 00:00:00+00:00"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(TenantLocalePolicyReceipts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TenantLocalePolicyReceipts::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TenantLocalePolicyReceipts::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TenantLocalePolicyReceipts::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TenantLocalePolicyReceipts::Response)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TenantLocalePolicyReceipts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(TenantLocalePolicyReceipts::TenantId)
                            .col(TenantLocalePolicyReceipts::IdempotencyKey),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_tenant_locale_policy_receipts_tenant")
                            .from(
                                TenantLocalePolicyReceipts::Table,
                                TenantLocalePolicyReceipts::TenantId,
                            )
                            .to(Tenants::Table, Tenants::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // `tenant_locales` was introduced after `tenants`, so an incremental
        // installation can contain tenant rows with no locale-policy row. Seed
        // one enabled default before the new constraints are installed. Reusing
        // the tenant UUID is deterministic and safe because IDs are scoped by
        // table; the owner will generate independent locale IDs on later writes.
        manager
            .get_connection()
            .execute_unprepared(
                r#"
INSERT INTO tenant_locales (
    id,
    tenant_id,
    locale,
    name,
    native_name,
    is_default,
    is_enabled,
    fallback_locale,
    policy_revision,
    created_at,
    updated_at
)
SELECT
    t.id,
    t.id,
    t.default_locale,
    t.default_locale,
    t.default_locale,
    TRUE,
    TRUE,
    NULL,
    0,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
FROM tenants t
WHERE NOT EXISTS (
    SELECT 1
    FROM tenant_locales tl
    WHERE tl.tenant_id = t.id
);
"#,
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
ALTER TABLE tenant_locales
    ADD CONSTRAINT ck_tenant_locales_default_enabled
    CHECK (NOT is_default OR is_enabled);
ALTER TABLE tenant_locales
    ADD CONSTRAINT ck_tenant_locales_no_self_fallback
    CHECK (fallback_locale IS NULL OR fallback_locale <> locale);
CREATE UNIQUE INDEX uq_tenant_locales_one_default
    ON tenant_locales (tenant_id)
    WHERE is_default;
"#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
CREATE UNIQUE INDEX IF NOT EXISTS uq_tenant_locales_one_default
    ON tenant_locales (tenant_id)
    WHERE is_default = 1;
CREATE TRIGGER IF NOT EXISTS tenant_locales_validate_insert
BEFORE INSERT ON tenant_locales
WHEN (NEW.is_default = 1 AND NEW.is_enabled = 0)
   OR (NEW.fallback_locale IS NOT NULL AND NEW.fallback_locale = NEW.locale)
BEGIN
    SELECT RAISE(ABORT, 'invalid tenant locale policy row');
END;
CREATE TRIGGER IF NOT EXISTS tenant_locales_validate_update
BEFORE UPDATE ON tenant_locales
WHEN (NEW.is_default = 1 AND NEW.is_enabled = 0)
   OR (NEW.fallback_locale IS NOT NULL AND NEW.fallback_locale = NEW.locale)
BEGIN
    SELECT RAISE(ABORT, 'invalid tenant locale policy row');
END;
"#,
                    )
                    .await?;
            }
            _ => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
ALTER TABLE tenant_locales
    ADD COLUMN default_tenant_guard BINARY(16)
        GENERATED ALWAYS AS (
            CASE WHEN is_default THEN tenant_id ELSE NULL END
        ) STORED,
    ADD UNIQUE INDEX uq_tenant_locales_one_default (default_tenant_guard),
    ADD CONSTRAINT ck_tenant_locales_default_enabled
    CHECK (NOT is_default OR is_enabled),
    ADD CONSTRAINT ck_tenant_locales_no_self_fallback
    CHECK (fallback_locale IS NULL OR fallback_locale <> locale);
"#,
                    )
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: policy revisions and idempotency receipts are durable
        // correctness evidence and must not be silently discarded.
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Tenants {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TenantLocales {
    Table,
    PolicyRevision,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum TenantLocalePolicyReceipts {
    Table,
    TenantId,
    IdempotencyKey,
    RequestHash,
    Response,
    CreatedAt,
}
