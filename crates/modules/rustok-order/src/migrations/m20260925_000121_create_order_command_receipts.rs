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
                    .table(OrderCommandReceipts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OrderCommandReceipts::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(OrderCommandReceipts::TenantId).uuid().not_null())
                    .col(ColumnDef::new(OrderCommandReceipts::ActorId).uuid().not_null())
                    .col(
                        ColumnDef::new(OrderCommandReceipts::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderCommandReceipts::CommandKind)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderCommandReceipts::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(OrderCommandReceipts::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(OrderCommandReceipts::ResponseKind).string_len(32))
                    .col(ColumnDef::new(OrderCommandReceipts::ResponseJson).json_binary())
                    .col(
                        ColumnDef::new(OrderCommandReceipts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(OrderCommandReceipts::CompletedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_order_command_receipt_key")
                    .table(OrderCommandReceipts::Table)
                    .col(OrderCommandReceipts::TenantId)
                    .col(OrderCommandReceipts::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_order_command_receipt_audit")
                    .table(OrderCommandReceipts::Table)
                    .col(OrderCommandReceipts::TenantId)
                    .col(OrderCommandReceipts::CommandKind)
                    .col(OrderCommandReceipts::CreatedAt)
                    .to_owned(),
            )
            .await?;

        install_constraints(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql | DatabaseBackend::Sqlite => {
                manager
                    .drop_table(
                        Table::drop()
                            .table(OrderCommandReceipts::Table)
                            .to_owned(),
                    )
                    .await
            }
            backend => Err(DbErr::Custom(format!(
                "unsupported database backend: {backend:?}"
            ))),
        }
    }
}

async fn install_constraints(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                ALTER TABLE order_command_receipts
                    ADD CONSTRAINT ck_order_command_receipt_identity
                        CHECK (
                            actor_id <> '00000000-0000-0000-0000-000000000000'::uuid
                            AND length(idempotency_key) BETWEEN 1 AND 191
                            AND command_kind IN (
                                'create_order_change',
                                'apply_order_change',
                                'cancel_order_change',
                                'create_return',
                                'complete_return',
                                'cancel_return'
                            )
                            AND request_hash ~ '^[0-9a-f]{64}$'
                        ),
                    ADD CONSTRAINT ck_order_command_receipt_state
                        CHECK (
                            (status = 'pending' AND response_kind IS NULL AND response_json IS NULL AND completed_at IS NULL)
                            OR
                            (status = 'completed' AND response_kind IS NOT NULL AND response_json IS NOT NULL AND completed_at IS NOT NULL)
                        );
                "#,
                )
                .await?;
        }
        DatabaseBackend::MySql => {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                ALTER TABLE order_command_receipts
                    ADD CONSTRAINT ck_order_command_receipt_identity CHECK (
                        actor_id <> '00000000-0000-0000-0000-000000000000'
                        AND CHAR_LENGTH(idempotency_key) BETWEEN 1 AND 191
                        AND command_kind IN (
                            'create_order_change',
                            'apply_order_change',
                            'cancel_order_change',
                            'create_return',
                            'complete_return',
                            'cancel_return'
                        )
                        AND request_hash REGEXP '^[0-9a-f]{64}$'
                    ),
                    ADD CONSTRAINT ck_order_command_receipt_state CHECK (
                        (status = 'pending' AND response_kind IS NULL AND response_json IS NULL AND completed_at IS NULL)
                        OR
                        (status = 'completed' AND response_kind IS NOT NULL AND response_json IS NOT NULL AND completed_at IS NOT NULL)
                    );
                "#,
                )
                .await?;
        }
        DatabaseBackend::Sqlite => {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                CREATE TRIGGER order_command_receipt_insert_guard
                BEFORE INSERT ON order_command_receipts
                FOR EACH ROW BEGIN
                    SELECT CASE WHEN lower(NEW.actor_id) = '00000000-0000-0000-0000-000000000000'
                        OR length(trim(NEW.idempotency_key)) NOT BETWEEN 1 AND 191
                        OR NEW.command_kind NOT IN (
                            'create_order_change',
                            'apply_order_change',
                            'cancel_order_change',
                            'create_return',
                            'complete_return',
                            'cancel_return'
                        )
                        OR length(NEW.request_hash) <> 64
                        OR lower(NEW.request_hash) GLOB '*[^0-9a-f]*'
                        THEN RAISE(ABORT, 'invalid order command receipt identity') END;
                    SELECT CASE WHEN NOT (
                        (NEW.status = 'pending' AND NEW.response_kind IS NULL AND NEW.response_json IS NULL AND NEW.completed_at IS NULL)
                        OR
                        (NEW.status = 'completed' AND NEW.response_kind IS NOT NULL AND NEW.response_json IS NOT NULL AND NEW.completed_at IS NOT NULL)
                    ) THEN RAISE(ABORT, 'invalid order command receipt state') END;
                END;

                CREATE TRIGGER order_command_receipt_update_guard
                BEFORE UPDATE ON order_command_receipts
                FOR EACH ROW BEGIN
                    SELECT CASE WHEN OLD.tenant_id <> NEW.tenant_id
                        OR OLD.actor_id <> NEW.actor_id
                        OR OLD.idempotency_key <> NEW.idempotency_key
                        OR OLD.command_kind <> NEW.command_kind
                        OR OLD.request_hash <> NEW.request_hash
                        OR OLD.created_at <> NEW.created_at
                        OR OLD.id <> NEW.id
                        OR NOT (
                            OLD.status = 'pending'
                            AND NEW.status = 'completed'
                            AND OLD.response_kind IS NULL
                            AND OLD.response_json IS NULL
                            AND OLD.completed_at IS NULL
                            AND NEW.response_kind IS NOT NULL
                            AND NEW.response_json IS NOT NULL
                            AND NEW.completed_at IS NOT NULL
                        )
                        THEN RAISE(ABORT, 'invalid order command receipt transition') END;
                END;
                "#,
                )
                .await?;
        }
        backend => {
            return Err(DbErr::Custom(format!(
                "unsupported database backend: {backend:?}"
            )));
        }
    }
    Ok(())
}

#[derive(Iden)]
enum OrderCommandReceipts {
    Table,
    Id,
    TenantId,
    ActorId,
    IdempotencyKey,
    CommandKind,
    RequestHash,
    Status,
    ResponseKind,
    ResponseJson,
    CreatedAt,
    CompletedAt,
}
