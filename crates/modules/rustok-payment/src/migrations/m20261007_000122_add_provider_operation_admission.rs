use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

/// Adds the checkout execution admission columns to `payment_provider_operations`.
///
/// The claim gate refuses an *extending* effect (`authorize`, `capture`) unless
/// the linked checkout operation is `open` and the generation beneath
/// `admission_epoch` still matches the one the checkout owner reports. The level
/// and its generation stay owned by `checkout_operations`
/// (`execution_admission` / `admission_epoch`, migration `m20261007_000012` in
/// `rustok-commerce`) and are read through the
/// `rustok_payment::CheckoutExecutionAdmissionPort`; payment keeps no derived
/// copy, so there is nothing to drift and no projection to rebuild.
///
/// `admission_epoch = 0` marks a row created before this contract (a legacy row,
/// admitted only while the level is `open`, exactly like the database guard this
/// replaces). `admission_refusal_code` carries the bounded reason of the last
/// refused claim, which turns a silent database exception into an observable
/// state; `admission_refused_at` records when it happened.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(PaymentProviderOperations::Table)
                    .add_column(
                        ColumnDef::new(PaymentProviderOperations::AdmissionEpoch)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .add_column(
                        ColumnDef::new(PaymentProviderOperations::AdmissionRefusalCode)
                            .string_len(64),
                    )
                    .add_column(
                        ColumnDef::new(PaymentProviderOperations::AdmissionRefusedAt)
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE payment_provider_operations
                            ADD CONSTRAINT ck_payment_provider_operations_admission_epoch
                            CHECK (admission_epoch >= 0),
                            ADD CONSTRAINT ck_payment_provider_operations_admission_refusal
                            CHECK (
                                admission_refusal_code IS NULL
                                OR admission_refusal_code IN (
                                    'checkout_admission_settling',
                                    'checkout_admission_closed',
                                    'checkout_admission_unavailable',
                                    'checkout_admission_epoch_mismatch',
                                    'checkout_admission_effect_unknown'
                                )
                            );
                        "#,
                    )
                    .await?;
            }
            // SQLite cannot add a table-level constraint to an existing table, so
            // the bounded refusal vocabulary is enforced by the typed claim gate
            // there.
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
                        ALTER TABLE payment_provider_operations
                            DROP CONSTRAINT IF EXISTS ck_payment_provider_operations_admission_epoch,
                            DROP CONSTRAINT IF EXISTS ck_payment_provider_operations_admission_refusal;
                        "#,
                    )
                    .await?;
                manager
                    .alter_table(
                        Table::alter()
                            .table(PaymentProviderOperations::Table)
                            .drop_column(PaymentProviderOperations::AdmissionEpoch)
                            .drop_column(PaymentProviderOperations::AdmissionRefusalCode)
                            .drop_column(PaymentProviderOperations::AdmissionRefusedAt)
                            .to_owned(),
                    )
                    .await?;
            }
            // SQLite cannot drop columns; the added columns stay inert.
            _ => {}
        }

        Ok(())
    }
}

#[derive(DeriveIden)]
enum PaymentProviderOperations {
    Table,
    AdmissionEpoch,
    AdmissionRefusalCode,
    AdmissionRefusedAt,
}
