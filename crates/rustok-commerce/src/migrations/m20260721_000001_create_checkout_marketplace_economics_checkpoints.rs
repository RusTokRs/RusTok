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
                    .table(CheckoutMarketplaceEconomicsCheckpoints::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(
                            CheckoutMarketplaceEconomicsCheckpoints::CheckoutOperationId,
                        )
                        .uuid()
                        .not_null()
                        .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::OrderId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::PlanHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::AllocationCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            CheckoutMarketplaceEconomicsCheckpoints::AllocationTotalAmount,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::AllocationSetHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::AssessmentCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            CheckoutMarketplaceEconomicsCheckpoints::CommissionTotalAmount,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            CheckoutMarketplaceEconomicsCheckpoints::SellerProceedsTotalAmount,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::AssessmentSetHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CheckoutMarketplaceEconomicsCheckpoints::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_checkout_marketplace_economics_operation")
                            .from(
                                CheckoutMarketplaceEconomicsCheckpoints::Table,
                                CheckoutMarketplaceEconomicsCheckpoints::CheckoutOperationId,
                            )
                            .to(CheckoutOperations::Table, CheckoutOperations::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_checkout_marketplace_economics_order")
                    .table(CheckoutMarketplaceEconomicsCheckpoints::Table)
                    .col(CheckoutMarketplaceEconomicsCheckpoints::TenantId)
                    .col(CheckoutMarketplaceEconomicsCheckpoints::OrderId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            DatabaseBackend::MySql => install_mysql_guards(manager).await?,
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => uninstall_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => {}
            DatabaseBackend::MySql => uninstall_mysql_guards(manager).await?,
        }

        manager
            .drop_table(
                Table::drop()
                    .table(CheckoutMarketplaceEconomicsCheckpoints::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE checkout_marketplace_economics_checkpoints
                ADD CONSTRAINT ck_checkout_marketplace_economics_identity
                CHECK (
                    btrim(plan_hash) <> ''
                    AND currency_code = upper(currency_code)
                    AND char_length(currency_code) = 3
                    AND btrim(allocation_set_hash) <> ''
                    AND btrim(assessment_set_hash) <> ''
                ),
                ADD CONSTRAINT ck_checkout_marketplace_economics_amounts
                CHECK (
                    allocation_count > 0
                    AND assessment_count = allocation_count
                    AND allocation_total_amount >= 0
                    AND commission_total_amount >= 0
                    AND seller_proceeds_total_amount >= 0
                    AND commission_total_amount + seller_proceeds_total_amount = allocation_total_amount
                );

            CREATE OR REPLACE FUNCTION enforce_checkout_marketplace_economics_integrity()
            RETURNS trigger AS $$
            DECLARE
                operation_tenant UUID;
                operation_order UUID;
            BEGIN
                IF TG_OP = 'UPDATE' THEN
                    RAISE EXCEPTION 'checkout marketplace economics checkpoint is immutable'
                        USING ERRCODE = '23514';
                END IF;

                SELECT tenant_id, order_id
                INTO operation_tenant, operation_order
                FROM checkout_operations
                WHERE id = NEW.checkout_operation_id;

                IF operation_tenant IS NULL
                    OR operation_tenant <> NEW.tenant_id
                    OR operation_order IS NULL
                    OR operation_order <> NEW.order_id
                THEN
                    RAISE EXCEPTION 'checkout marketplace economics operation identity mismatch'
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_marketplace_economics_integrity_guard
            BEFORE INSERT OR UPDATE ON checkout_marketplace_economics_checkpoints
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_marketplace_economics_integrity();
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_marketplace_economics_integrity_guard
                ON checkout_marketplace_economics_checkpoints;
            DROP FUNCTION IF EXISTS enforce_checkout_marketplace_economics_integrity();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER checkout_marketplace_economics_guard_insert
            BEFORE INSERT ON checkout_marketplace_economics_checkpoints
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN trim(NEW.plan_hash) = ''
                    OR NEW.currency_code <> upper(NEW.currency_code)
                    OR length(NEW.currency_code) <> 3
                    OR trim(NEW.allocation_set_hash) = ''
                    OR trim(NEW.assessment_set_hash) = ''
                    THEN RAISE(ABORT, 'invalid checkout marketplace economics identity') END;
                SELECT CASE WHEN NEW.allocation_count <= 0
                    OR NEW.assessment_count <> NEW.allocation_count
                    OR NEW.allocation_total_amount < 0
                    OR NEW.commission_total_amount < 0
                    OR NEW.seller_proceeds_total_amount < 0
                    OR NEW.commission_total_amount + NEW.seller_proceeds_total_amount
                        <> NEW.allocation_total_amount
                    THEN RAISE(ABORT, 'invalid checkout marketplace economics amounts') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM checkout_operations
                    WHERE id = NEW.checkout_operation_id
                      AND tenant_id = NEW.tenant_id
                      AND order_id = NEW.order_id
                ) THEN RAISE(ABORT, 'checkout marketplace economics operation identity mismatch') END;
            END;

            CREATE TRIGGER checkout_marketplace_economics_guard_update
            BEFORE UPDATE ON checkout_marketplace_economics_checkpoints
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'checkout marketplace economics checkpoint is immutable');
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE checkout_marketplace_economics_checkpoints
                ADD CONSTRAINT ck_checkout_marketplace_economics_identity
                CHECK (
                    trim(plan_hash) <> ''
                    AND currency_code = upper(currency_code)
                    AND char_length(currency_code) = 3
                    AND trim(allocation_set_hash) <> ''
                    AND trim(assessment_set_hash) <> ''
                ),
                ADD CONSTRAINT ck_checkout_marketplace_economics_amounts
                CHECK (
                    allocation_count > 0
                    AND assessment_count = allocation_count
                    AND allocation_total_amount >= 0
                    AND commission_total_amount >= 0
                    AND seller_proceeds_total_amount >= 0
                    AND commission_total_amount + seller_proceeds_total_amount = allocation_total_amount
                )
            "#,
        )
        .await?;

    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER checkout_marketplace_economics_guard_insert
            BEFORE INSERT ON checkout_marketplace_economics_checkpoints
            FOR EACH ROW
            BEGIN
                IF trim(NEW.plan_hash) = ''
                    OR NEW.currency_code <> upper(NEW.currency_code)
                    OR char_length(NEW.currency_code) <> 3
                    OR trim(NEW.allocation_set_hash) = ''
                    OR trim(NEW.assessment_set_hash) = ''
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid checkout marketplace economics identity';
                END IF;

                IF NEW.allocation_count <= 0
                    OR NEW.assessment_count <> NEW.allocation_count
                    OR NEW.allocation_total_amount < 0
                    OR NEW.commission_total_amount < 0
                    OR NEW.seller_proceeds_total_amount < 0
                    OR NEW.commission_total_amount + NEW.seller_proceeds_total_amount
                        <> NEW.allocation_total_amount
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid checkout marketplace economics amounts';
                END IF;

                IF (
                    SELECT COUNT(*)
                    FROM checkout_operations
                    WHERE id = NEW.checkout_operation_id
                      AND tenant_id = NEW.tenant_id
                      AND order_id = NEW.order_id
                ) <> 1
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'checkout marketplace economics operation identity mismatch';
                END IF;
            END
            "#,
        )
        .await?;

    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER checkout_marketplace_economics_guard_update
            BEFORE UPDATE ON checkout_marketplace_economics_checkpoints
            FOR EACH ROW
            BEGIN
                SIGNAL SQLSTATE '45000'
                    SET MESSAGE_TEXT = 'checkout marketplace economics checkpoint is immutable';
            END
            "#,
        )
        .await?;

    Ok(())
}

async fn uninstall_mysql_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared("DROP TRIGGER IF EXISTS checkout_marketplace_economics_guard_insert")
        .await?;
    manager
        .get_connection()
        .execute_unprepared("DROP TRIGGER IF EXISTS checkout_marketplace_economics_guard_update")
        .await?;
    Ok(())
}

#[derive(Iden)]
enum CheckoutMarketplaceEconomicsCheckpoints {
    Table,
    CheckoutOperationId,
    TenantId,
    OrderId,
    PlanHash,
    CurrencyCode,
    AllocationCount,
    AllocationTotalAmount,
    AllocationSetHash,
    AssessmentCount,
    CommissionTotalAmount,
    SellerProceedsTotalAmount,
    AssessmentSetHash,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum CheckoutOperations {
    Table,
    Id,
}
