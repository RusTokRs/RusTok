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
                    .table(CheckoutOrderPlans::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CheckoutOrderPlans::CheckoutOperationId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOrderPlans::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOrderPlans::SnapshotHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOrderPlans::PlanHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOrderPlans::Payload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOrderPlans::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CheckoutOrderPlans::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CheckoutOrderPlans::Table,
                                CheckoutOrderPlans::CheckoutOperationId,
                            )
                            .to(CheckoutOperations::Table, CheckoutOperations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_checkout_order_plans_tenant_operation")
                    .table(CheckoutOrderPlans::Table)
                    .col(CheckoutOrderPlans::TenantId)
                    .col(CheckoutOrderPlans::CheckoutOperationId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guard(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            _ => {}
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
                        DROP TRIGGER IF EXISTS checkout_order_plans_integrity_guard
                            ON checkout_order_plans;
                        DROP FUNCTION IF EXISTS enforce_checkout_order_plan_integrity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS checkout_order_plans_guard_insert;
                        DROP TRIGGER IF EXISTS checkout_order_plans_guard_update;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        manager
            .drop_table(
                Table::drop()
                    .table(CheckoutOrderPlans::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE checkout_order_plans
                ADD CONSTRAINT ck_checkout_order_plans_hashes
                CHECK (
                    btrim(snapshot_hash) <> ''
                    AND btrim(plan_hash) <> ''
                    AND jsonb_typeof(payload) = 'object'
                );

            CREATE OR REPLACE FUNCTION enforce_checkout_order_plan_integrity()
            RETURNS trigger AS $$
            DECLARE
                operation_tenant UUID;
                operation_snapshot VARCHAR(128);
                operation_stage VARCHAR(32);
            BEGIN
                IF TG_OP = 'UPDATE' THEN
                    RAISE EXCEPTION 'checkout order plans are immutable'
                        USING ERRCODE = '23514';
                END IF;

                SELECT tenant_id, snapshot_hash, stage
                INTO operation_tenant, operation_snapshot, operation_stage
                FROM checkout_operations
                WHERE id = NEW.checkout_operation_id;

                IF operation_tenant IS NULL OR operation_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'checkout order plan tenant mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF operation_snapshot IS NULL OR operation_snapshot <> NEW.snapshot_hash THEN
                    RAISE EXCEPTION 'checkout order plan snapshot mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF operation_stage <> 'cart_locked' THEN
                    RAISE EXCEPTION 'checkout order plan must be created from cart_locked stage'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_order_plans_integrity_guard
            BEFORE INSERT OR UPDATE ON checkout_order_plans
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_order_plan_integrity();
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
            CREATE TRIGGER checkout_order_plans_guard_insert
            BEFORE INSERT ON checkout_order_plans
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN trim(NEW.snapshot_hash) = '' OR trim(NEW.plan_hash) = ''
                    THEN RAISE(ABORT, 'checkout order plan hashes must not be empty') END;
                SELECT CASE WHEN json_type(NEW.payload) <> 'object'
                    THEN RAISE(ABORT, 'checkout order plan payload must be an object') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM checkout_operations co
                    WHERE co.id = NEW.checkout_operation_id
                      AND co.tenant_id = NEW.tenant_id
                      AND co.snapshot_hash = NEW.snapshot_hash
                      AND co.stage = 'cart_locked'
                ) THEN RAISE(ABORT, 'checkout order plan operation mismatch') END;
            END;

            CREATE TRIGGER checkout_order_plans_guard_update
            BEFORE UPDATE ON checkout_order_plans
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'checkout order plans are immutable');
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum CheckoutOrderPlans {
    Table,
    CheckoutOperationId,
    TenantId,
    SnapshotHash,
    PlanHash,
    Payload,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum CheckoutOperations {
    Table,
    Id,
}
