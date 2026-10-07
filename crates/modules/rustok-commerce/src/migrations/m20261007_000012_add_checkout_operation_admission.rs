use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

/// Adds the execution admission record to `checkout_operations`.
///
/// The record is the checkout-owned half of the provider execution admission
/// contract (`DECISIONS/2026-10-07-checkout-operation-invariants-owned-by-rust.md`,
/// section "Remaining cutover"): `execution_admission` carries the bounded level
/// (`open` / `settling` / `closed`) and `admission_epoch` carries the generation
/// that a provider operation is admitted under. `rustok-payment` refuses to start
/// an extending provider operation whose generation is not the admitted one, and
/// `checkout.operation.admission_changed` publishes every level change.
///
/// The level and the epoch are written by the journal together with the status
/// transition (one conditional update), not by a trigger: the migration only
/// declares the columns, the backfill for pre-existing rows, and the
/// column-level `CHECK` shape.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutOperations::Table)
                    .add_column(
                        ColumnDef::new(CheckoutOperations::ExecutionAdmission)
                            .string_len(16)
                            .not_null()
                            .default(Expr::cust("'open'")),
                    )
                    .add_column(
                        ColumnDef::new(CheckoutOperations::AdmissionEpoch)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        // Pre-existing rows get the level their status belongs to. A row that
        // already entered compensation must not read `open`, otherwise the
        // projection would admit extending provider execution for a checkout
        // that is being unwound.
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE checkout_operations
                SET execution_admission = CASE
                        WHEN status IN (
                            'compensation_required',
                            'compensating',
                            'reconciliation_required'
                        ) THEN 'settling'
                        WHEN status IN ('completed', 'compensated', 'failed') THEN 'closed'
                        ELSE 'open'
                    END
                WHERE execution_admission <> CASE
                        WHEN status IN (
                            'compensation_required',
                            'compensating',
                            'reconciliation_required'
                        ) THEN 'settling'
                        WHEN status IN ('completed', 'compensated', 'failed') THEN 'closed'
                        ELSE 'open'
                    END;
                "#,
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE checkout_operations
                            ADD CONSTRAINT ck_checkout_operations_execution_admission
                            CHECK (execution_admission IN ('open', 'settling', 'closed')),
                            ADD CONSTRAINT ck_checkout_operations_admission_epoch
                            CHECK (admission_epoch > 0);
                        "#,
                    )
                    .await?;
            }
            // SQLite cannot add a table-level constraint to an existing table, so
            // the level vocabulary is enforced by the typed journal writer there.
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE checkout_operations
                            DROP CONSTRAINT IF EXISTS ck_checkout_operations_execution_admission,
                            DROP CONSTRAINT IF EXISTS ck_checkout_operations_admission_epoch;
                        "#,
                    )
                    .await?;
                manager
                    .alter_table(
                        Table::alter()
                            .table(CheckoutOperations::Table)
                            .drop_column(CheckoutOperations::ExecutionAdmission)
                            .drop_column(CheckoutOperations::AdmissionEpoch)
                            .to_owned(),
                    )
                    .await?;
            }
            // SQLite cannot drop columns; the added columns stay inert and the
            // journal falls back to the pre-admission shape after a rollback of
            // the Rust writers.
            _ => {}
        }

        Ok(())
    }
}

#[derive(DeriveIden)]
enum CheckoutOperations {
    Table,
    ExecutionAdmission,
    AdmissionEpoch,
}
