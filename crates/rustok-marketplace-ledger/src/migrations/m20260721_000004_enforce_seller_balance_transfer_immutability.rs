use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await,
            DatabaseBackend::MySql => install_mysql_guards(manager).await,
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS marketplace_entry_balance_bucket_immutable ON marketplace_ledger_entry_balance_buckets;
                        DROP TRIGGER IF EXISTS marketplace_seller_balance_transfer_immutable ON marketplace_seller_balance_transfers;
                        DROP TRIGGER IF EXISTS marketplace_seller_balance_transfer_line_immutable ON marketplace_seller_balance_transfer_lines;
                        DROP FUNCTION IF EXISTS reject_marketplace_balance_transfer_mutation();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite | DatabaseBackend::MySql => {
                for trigger in trigger_names() {
                    manager
                        .get_connection()
                        .execute_unprepared(format!("DROP TRIGGER IF EXISTS {trigger};").as_str())
                        .await?;
                }
            }
        }
        Ok(())
    }
}

async fn install_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION reject_marketplace_balance_transfer_mutation()
            RETURNS trigger AS $$
            BEGIN
                RAISE EXCEPTION 'marketplace seller balance transfer records are append-only'
                    USING ERRCODE = '23514';
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER marketplace_entry_balance_bucket_immutable
            BEFORE UPDATE OR DELETE ON marketplace_ledger_entry_balance_buckets
            FOR EACH ROW EXECUTE FUNCTION reject_marketplace_balance_transfer_mutation();

            CREATE TRIGGER marketplace_seller_balance_transfer_immutable
            BEFORE UPDATE OR DELETE ON marketplace_seller_balance_transfers
            FOR EACH ROW EXECUTE FUNCTION reject_marketplace_balance_transfer_mutation();

            CREATE TRIGGER marketplace_seller_balance_transfer_line_immutable
            BEFORE UPDATE OR DELETE ON marketplace_seller_balance_transfer_lines
            FOR EACH ROW EXECUTE FUNCTION reject_marketplace_balance_transfer_mutation();
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
            CREATE TRIGGER marketplace_entry_balance_bucket_immutable_update
            BEFORE UPDATE ON marketplace_ledger_entry_balance_buckets
            FOR EACH ROW BEGIN
                SELECT RAISE(ABORT, 'marketplace seller balance transfer records are append-only');
            END;
            CREATE TRIGGER marketplace_entry_balance_bucket_immutable_delete
            BEFORE DELETE ON marketplace_ledger_entry_balance_buckets
            FOR EACH ROW BEGIN
                SELECT RAISE(ABORT, 'marketplace seller balance transfer records are append-only');
            END;

            CREATE TRIGGER marketplace_seller_balance_transfer_immutable_update
            BEFORE UPDATE ON marketplace_seller_balance_transfers
            FOR EACH ROW BEGIN
                SELECT RAISE(ABORT, 'marketplace seller balance transfer records are append-only');
            END;
            CREATE TRIGGER marketplace_seller_balance_transfer_immutable_delete
            BEFORE DELETE ON marketplace_seller_balance_transfers
            FOR EACH ROW BEGIN
                SELECT RAISE(ABORT, 'marketplace seller balance transfer records are append-only');
            END;

            CREATE TRIGGER marketplace_seller_balance_transfer_line_immutable_update
            BEFORE UPDATE ON marketplace_seller_balance_transfer_lines
            FOR EACH ROW BEGIN
                SELECT RAISE(ABORT, 'marketplace seller balance transfer records are append-only');
            END;
            CREATE TRIGGER marketplace_seller_balance_transfer_line_immutable_delete
            BEFORE DELETE ON marketplace_seller_balance_transfer_lines
            FOR EACH ROW BEGIN
                SELECT RAISE(ABORT, 'marketplace seller balance transfer records are append-only');
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for trigger in trigger_names() {
        manager
            .get_connection()
            .execute_unprepared(format!("DROP TRIGGER IF EXISTS {trigger};").as_str())
            .await?;
    }
    for statement in [
        "CREATE TRIGGER marketplace_entry_balance_bucket_immutable_update BEFORE UPDATE ON marketplace_ledger_entry_balance_buckets FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace seller balance transfer records are append-only'",
        "CREATE TRIGGER marketplace_entry_balance_bucket_immutable_delete BEFORE DELETE ON marketplace_ledger_entry_balance_buckets FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace seller balance transfer records are append-only'",
        "CREATE TRIGGER marketplace_seller_balance_transfer_immutable_update BEFORE UPDATE ON marketplace_seller_balance_transfers FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace seller balance transfer records are append-only'",
        "CREATE TRIGGER marketplace_seller_balance_transfer_immutable_delete BEFORE DELETE ON marketplace_seller_balance_transfers FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace seller balance transfer records are append-only'",
        "CREATE TRIGGER marketplace_seller_balance_transfer_line_immutable_update BEFORE UPDATE ON marketplace_seller_balance_transfer_lines FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace seller balance transfer records are append-only'",
        "CREATE TRIGGER marketplace_seller_balance_transfer_line_immutable_delete BEFORE DELETE ON marketplace_seller_balance_transfer_lines FOR EACH ROW SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace seller balance transfer records are append-only'",
    ] {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }
    Ok(())
}

fn trigger_names() -> [&'static str; 6] {
    [
        "marketplace_entry_balance_bucket_immutable_update",
        "marketplace_entry_balance_bucket_immutable_delete",
        "marketplace_seller_balance_transfer_immutable_update",
        "marketplace_seller_balance_transfer_immutable_delete",
        "marketplace_seller_balance_transfer_line_immutable_update",
        "marketplace_seller_balance_transfer_line_immutable_delete",
    ]
}
