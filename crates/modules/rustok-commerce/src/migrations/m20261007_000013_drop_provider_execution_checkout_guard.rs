use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

/// Moves the provider execution admission from the database trigger to the
/// payment claim gate.
///
/// `m20260713_000016` installed `block_provider_execution_during_checkout_compensation()`
/// on `payment_provider_operations`: a `BEFORE UPDATE OF status` trigger that
/// read `payment_collections.metadata -> checkout.operation_id` across module
/// boundaries and refused every `executing` write while the checkout was in
/// `compensation_required`, `compensating`, `reconciliation_required`,
/// `compensated` or `failed`. That is a cross-aggregate workflow guard inside
/// database code, which the repository contract forbids, and it was wrong in two
/// ways: it could not tell an *extending* effect (`authorize`, `capture`) from an
/// *unwinding* one (`cancel`, `refund`), so it also blocked the compensations it
/// was meant to protect, and it made every refusal an opaque exception with no
/// bounded reason.
///
/// The admission is now owned by typed Rust code: the checkout journal carries
/// `execution_admission` (`open`, `settling`, `closed`) and its monotone
/// `admission_epoch` (`rustok-commerce` migration `m20261007_000012`), the
/// payment claim gate fences extending execution on that level and generation
/// through `rustok_payment::CheckoutExecutionAdmissionPort`
/// (`rustok-payment` migration `m20261007_000122`), unwinding effects stay
/// admitted, and a refusal records a bounded
/// `payment_provider_operations.admission_refusal_code` plus
/// `rustok_payment_provider_execution_admission_refused_total`. A park stamps the
/// new generation on the collection's non-terminal provider operations inside
/// the transition's own transaction, so an in-flight claim fails its conditional
/// write instead of racing the park.
///
/// `down()` restores the exact text installed by `m20260713_000016`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_provider_operations_checkout_guard
                            ON payment_provider_operations;
                        DROP FUNCTION IF EXISTS block_provider_execution_during_checkout_compensation();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS payment_provider_operations_checkout_guard;",
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS payment_provider_operations_checkout_guard;",
                    )
                    .await?;
            }
            _ => unreachable!("unsupported SeaORM database backend"),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
            _ => unreachable!("unsupported SeaORM database backend"),
        }
        Ok(())
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION block_provider_execution_during_checkout_compensation()
            RETURNS trigger AS $$
            DECLARE
                checkout_status VARCHAR(48);
            BEGIN
                IF NEW.status <> 'executing' OR OLD.status = 'executing' THEN
                    RETURN NEW;
                END IF;

                SELECT co.status
                INTO checkout_status
                FROM payment_collections pc
                JOIN checkout_operations co
                  ON lower(co.id::text) = lower(pc.metadata #>> '{checkout,operation_id}')
                 AND co.tenant_id = pc.tenant_id
                WHERE pc.id = NEW.payment_collection_id
                  AND pc.tenant_id = NEW.tenant_id;

                IF checkout_status IN (
                    'compensation_required',
                    'compensating',
                    'reconciliation_required',
                    'compensated',
                    'failed'
                ) THEN
                    RAISE EXCEPTION
                        'payment provider operation cannot execute while checkout is %',
                        checkout_status
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER payment_provider_operations_checkout_guard
            BEFORE UPDATE OF status ON payment_provider_operations
            FOR EACH ROW
            EXECUTE FUNCTION block_provider_execution_during_checkout_compensation();
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
            CREATE TRIGGER payment_provider_operations_checkout_guard
            BEFORE UPDATE OF status ON payment_provider_operations
            FOR EACH ROW
            WHEN NEW.status = 'executing'
             AND OLD.status <> 'executing'
            BEGIN
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM payment_collections pc
                    JOIN checkout_operations co
                      ON (co.id = json_extract(pc.metadata, '$.checkout.operation_id')
                          OR lower(hex(co.id)) = lower(replace(json_extract(pc.metadata, '$.checkout.operation_id'), '-', '')))
                     AND co.tenant_id = pc.tenant_id
                    WHERE pc.id = NEW.payment_collection_id
                      AND pc.tenant_id = NEW.tenant_id
                      AND co.status IN (
                          'compensation_required',
                          'compensating',
                          'reconciliation_required',
                          'compensated',
                          'failed'
                      )
                ) THEN RAISE(ABORT, 'payment provider operation blocked by checkout compensation') END;
            END;
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
            CREATE TRIGGER payment_provider_operations_checkout_guard
            BEFORE UPDATE ON payment_provider_operations
            FOR EACH ROW
            BEGIN
                DECLARE blocked_operations INT DEFAULT 0;
                IF NEW.status = 'executing' AND OLD.status <> 'executing' THEN
                    SELECT COUNT(*) INTO blocked_operations
                    FROM payment_collections pc
                    JOIN checkout_operations co
                      ON co.id = JSON_UNQUOTE(JSON_EXTRACT(pc.metadata, '$.checkout.operation_id'))
                     AND co.tenant_id = pc.tenant_id
                    WHERE pc.id = NEW.payment_collection_id
                      AND pc.tenant_id = NEW.tenant_id
                      AND co.status IN (
                          'compensation_required',
                          'compensating',
                          'reconciliation_required',
                          'compensated',
                          'failed'
                      );
                    IF blocked_operations > 0 THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'payment provider operation blocked by checkout compensation';
                    END IF;
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}
