use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(OrderCheckoutIdentities::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderCheckoutIdentities::CheckoutOperationId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(OrderCheckoutIdentities::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderCheckoutIdentities::OrderId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(OrderCheckoutIdentities::SourceCartId).uuid())
                    .col(ColumnDef::new(OrderCheckoutIdentities::SnapshotHash).string_len(128))
                    .col(ColumnDef::new(OrderCheckoutIdentities::RequestHash).string_len(64))
                    .col(
                        ColumnDef::new(OrderCheckoutIdentities::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_order_checkout_identity_order")
                            .from(
                                OrderCheckoutIdentities::Table,
                                OrderCheckoutIdentities::OrderId,
                            )
                            .to(Orders::Table, Orders::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_order_checkout_identity_order")
                    .table(OrderCheckoutIdentities::Table)
                    .col(OrderCheckoutIdentities::TenantId)
                    .col(OrderCheckoutIdentities::OrderId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ux_order_checkout_identity_cart")
                    .table(OrderCheckoutIdentities::Table)
                    .col(OrderCheckoutIdentities::TenantId)
                    .col(OrderCheckoutIdentities::SourceCartId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_order_checkout_identity_tenant_operation")
                    .table(OrderCheckoutIdentities::Table)
                    .col(OrderCheckoutIdentities::TenantId)
                    .col(OrderCheckoutIdentities::CheckoutOperationId)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS order_checkout_identity_integrity_guard
                            ON order_checkout_identities;
                        DROP TABLE IF EXISTS order_checkout_identities;
                        DROP FUNCTION IF EXISTS enforce_order_checkout_identity_integrity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared("DROP TABLE IF EXISTS order_checkout_identities;")
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS order_checkout_identity_guard_insert;",
                    )
                    .await?;
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS order_checkout_identity_guard_update;",
                    )
                    .await?;
                manager
                    .get_connection()
                    .execute_unprepared("DROP TABLE IF EXISTS order_checkout_identities;")
                    .await?;
            }
        }
        Ok(())
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE order_checkout_identities
                ADD CONSTRAINT ck_order_checkout_identity_facts
                CHECK (
                    checkout_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
                    AND (
                        source_cart_id IS NULL
                        OR source_cart_id <> '00000000-0000-0000-0000-000000000000'::uuid
                    )
                    AND (
                        (snapshot_hash IS NULL AND request_hash IS NULL)
                        OR (
                            snapshot_hash ~ '^[0-9a-f]{1,128}$'
                            AND request_hash ~ '^[0-9a-f]{64}$'
                        )
                    )
                );

            CREATE OR REPLACE FUNCTION enforce_order_checkout_identity_integrity()
            RETURNS trigger AS $$
            DECLARE
                order_tenant UUID;
            BEGIN
                IF TG_OP = 'UPDATE' THEN
                    RAISE EXCEPTION 'order checkout identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                SELECT tenant_id INTO order_tenant
                FROM orders
                WHERE id = NEW.order_id;

                IF order_tenant IS NULL OR order_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'order checkout identity tenant/order mismatch'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER order_checkout_identity_integrity_guard
            BEFORE INSERT OR UPDATE ON order_checkout_identities
            FOR EACH ROW
            EXECUTE FUNCTION enforce_order_checkout_identity_integrity();

            INSERT INTO order_checkout_identities (
                checkout_operation_id,
                tenant_id,
                order_id,
                source_cart_id,
                snapshot_hash,
                request_hash,
                created_at
            )
            SELECT
                (metadata #>> '{checkout,operation_id}')::uuid,
                tenant_id,
                id,
                NULL,
                CASE
                    WHEN lower(metadata #>> '{checkout,snapshot_hash}') ~ '^[0-9a-f]{1,128}$'
                     AND lower(metadata #>> '{checkout,order_request_hash}') ~ '^[0-9a-f]{64}$'
                    THEN lower(metadata #>> '{checkout,snapshot_hash}')
                    ELSE NULL
                END,
                CASE
                    WHEN lower(metadata #>> '{checkout,snapshot_hash}') ~ '^[0-9a-f]{1,128}$'
                     AND lower(metadata #>> '{checkout,order_request_hash}') ~ '^[0-9a-f]{64}$'
                    THEN lower(metadata #>> '{checkout,order_request_hash}')
                    ELSE NULL
                END,
                created_at
            FROM orders
            WHERE metadata #>> '{checkout,operation_id}'
                ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
              AND lower(metadata #>> '{checkout,operation_id}')
                    <> '00000000-0000-0000-0000-000000000000'
            ON CONFLICT DO NOTHING;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER order_checkout_identity_guard_insert
            BEFORE INSERT ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN length(NEW.checkout_operation_id) <> 36
                    OR lower(NEW.checkout_operation_id) GLOB '*[^0-9a-f-]*'
                    OR substr(NEW.checkout_operation_id, 9, 1) <> '-'
                    OR substr(NEW.checkout_operation_id, 14, 1) <> '-'
                    OR substr(NEW.checkout_operation_id, 19, 1) <> '-'
                    OR substr(NEW.checkout_operation_id, 24, 1) <> '-'
                    OR lower(NEW.checkout_operation_id)
                        = '00000000-0000-0000-0000-000000000000'
                    THEN RAISE(ABORT, 'invalid checkout operation identity') END;
                SELECT CASE WHEN NEW.source_cart_id IS NOT NULL AND (
                    length(NEW.source_cart_id) <> 36
                    OR lower(NEW.source_cart_id) GLOB '*[^0-9a-f-]*'
                    OR lower(NEW.source_cart_id)
                        = '00000000-0000-0000-0000-000000000000'
                ) THEN RAISE(ABORT, 'invalid source cart identity') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM orders
                    WHERE id = NEW.order_id
                      AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'order checkout identity tenant/order mismatch') END;
                SELECT CASE WHEN NOT (
                    (NEW.snapshot_hash IS NULL AND NEW.request_hash IS NULL)
                    OR (
                        NEW.snapshot_hash IS NOT NULL
                        AND length(NEW.snapshot_hash) BETWEEN 1 AND 128
                        AND NEW.snapshot_hash NOT GLOB '*[^0-9a-f]*'
                        AND NEW.request_hash IS NOT NULL
                        AND length(NEW.request_hash) = 64
                        AND NEW.request_hash NOT GLOB '*[^0-9a-f]*'
                    )
                ) THEN RAISE(ABORT, 'invalid order checkout identity hashes') END;
            END;

            CREATE TRIGGER order_checkout_identity_guard_update
            BEFORE UPDATE ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'order checkout identity is immutable');
            END;

            INSERT OR IGNORE INTO order_checkout_identities (
                checkout_operation_id,
                tenant_id,
                order_id,
                source_cart_id,
                snapshot_hash,
                request_hash,
                created_at
            )
            SELECT
                lower(trim(json_extract(metadata, '$.checkout.operation_id'))),
                tenant_id,
                id,
                NULL,
                CASE
                    WHEN lower(json_extract(metadata, '$.checkout.snapshot_hash'))
                            NOT GLOB '*[^0-9a-f]*'
                     AND length(json_extract(metadata, '$.checkout.snapshot_hash')) BETWEEN 1 AND 128
                     AND lower(json_extract(metadata, '$.checkout.order_request_hash'))
                            NOT GLOB '*[^0-9a-f]*'
                     AND length(json_extract(metadata, '$.checkout.order_request_hash')) = 64
                    THEN lower(json_extract(metadata, '$.checkout.snapshot_hash'))
                    ELSE NULL
                END,
                CASE
                    WHEN lower(json_extract(metadata, '$.checkout.snapshot_hash'))
                            NOT GLOB '*[^0-9a-f]*'
                     AND length(json_extract(metadata, '$.checkout.snapshot_hash')) BETWEEN 1 AND 128
                     AND lower(json_extract(metadata, '$.checkout.order_request_hash'))
                            NOT GLOB '*[^0-9a-f]*'
                     AND length(json_extract(metadata, '$.checkout.order_request_hash')) = 64
                    THEN lower(json_extract(metadata, '$.checkout.order_request_hash'))
                    ELSE NULL
                END,
                created_at
            FROM orders
            WHERE length(trim(json_extract(metadata, '$.checkout.operation_id'))) = 36
              AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                    NOT GLOB '*[^0-9a-f-]*'
              AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 9, 1) = '-'
              AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 14, 1) = '-'
              AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 19, 1) = '-'
              AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 24, 1) = '-'
              AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                    <> '00000000-0000-0000-0000-000000000000';
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER order_checkout_identity_guard_insert
            BEFORE INSERT ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                IF NEW.checkout_operation_id = '00000000-0000-0000-0000-000000000000'
                    OR NEW.source_cart_id = '00000000-0000-0000-0000-000000000000'
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid order checkout identity';
                END IF;
                IF (
                    SELECT COUNT(*) FROM orders
                    WHERE id = NEW.order_id
                      AND tenant_id = NEW.tenant_id
                ) <> 1 THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'order checkout identity tenant/order mismatch';
                END IF;
                IF NOT (
                    (NEW.snapshot_hash IS NULL AND NEW.request_hash IS NULL)
                    OR (
                        NEW.snapshot_hash REGEXP '^[0-9a-f]{1,128}$'
                        AND NEW.request_hash REGEXP '^[0-9a-f]{64}$'
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid order checkout identity hashes';
                END IF;
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER order_checkout_identity_guard_update
            BEFORE UPDATE ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                SIGNAL SQLSTATE '45000'
                    SET MESSAGE_TEXT = 'order checkout identity is immutable';
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            INSERT IGNORE INTO order_checkout_identities (
                checkout_operation_id,
                tenant_id,
                order_id,
                source_cart_id,
                snapshot_hash,
                request_hash,
                created_at
            )
            SELECT
                lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id'))),
                tenant_id,
                id,
                NULL,
                CASE
                    WHEN lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.snapshot_hash')))
                            REGEXP '^[0-9a-f]{1,128}$'
                     AND lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_request_hash')))
                            REGEXP '^[0-9a-f]{64}$'
                    THEN lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.snapshot_hash')))
                    ELSE NULL
                END,
                CASE
                    WHEN lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.snapshot_hash')))
                            REGEXP '^[0-9a-f]{1,128}$'
                     AND lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_request_hash')))
                            REGEXP '^[0-9a-f]{64}$'
                    THEN lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_request_hash')))
                    ELSE NULL
                END,
                created_at
            FROM orders
            WHERE JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id'))
                REGEXP '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
              AND lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                    <> '00000000-0000-0000-0000-000000000000';
            "#,
        )
        .await?;
    Ok(())
}

#[derive(Iden)]
enum OrderCheckoutIdentities {
    Table,
    CheckoutOperationId,
    TenantId,
    OrderId,
    SourceCartId,
    SnapshotHash,
    RequestHash,
    CreatedAt,
}

#[derive(Iden)]
enum Orders {
    Table,
    Id,
}
