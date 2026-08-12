use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DbBackend},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE_NAME: &str = "index_consistency_finding_lifecycle_events";
const POSTGRES_GUARD_FUNCTION: &str = "rustok_index_reject_finding_lifecycle_event_update";
const POSTGRES_GUARD_TRIGGER: &str = "trg_index_finding_lifecycle_events_no_update";
const SQLITE_UPDATE_TRIGGER: &str = "trg_index_finding_lifecycle_events_no_update";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_supported_backend(manager)?;
        manager
            .create_table(
                Table::create()
                    .table(IndexFindingLifecycleEvents::Table)
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::CommandId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::FindingId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::Action)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::FromState)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::ToState)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::ActorKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::ActorSubject)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::Reason)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingLifecycleEvents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_finding_lifecycle_events")
                            .col(IndexFindingLifecycleEvents::TenantId)
                            .col(IndexFindingLifecycleEvents::CommandId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_finding_lifecycle_event_finding")
                            .from(
                                IndexFindingLifecycleEvents::Table,
                                (
                                    IndexFindingLifecycleEvents::TenantId,
                                    IndexFindingLifecycleEvents::FindingId,
                                ),
                            )
                            .to(
                                IndexConsistencyFindings::Table,
                                (
                                    IndexConsistencyFindings::TenantId,
                                    IndexConsistencyFindings::FindingId,
                                ),
                            )
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("action IN ('resolve', 'ignore')"))
                    .check(Expr::cust("from_state = 'open'"))
                    .check(Expr::cust(
                        "(action = 'resolve' AND to_state = 'resolved') OR (action = 'ignore' AND to_state = 'ignored')",
                    ))
                    .check(Expr::cust(
                        "length(actor_kind) BETWEEN 1 AND 32 AND actor_kind = trim(actor_kind)",
                    ))
                    .check(Expr::cust(
                        "length(actor_subject) BETWEEN 1 AND 191 AND actor_subject = trim(actor_subject)",
                    ))
                    .check(Expr::cust(
                        "length(reason) BETWEEN 1 AND 512 AND reason = trim(reason)",
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_index_finding_lifecycle_events_finding")
                    .table(IndexFindingLifecycleEvents::Table)
                    .col(IndexFindingLifecycleEvents::TenantId)
                    .col(IndexFindingLifecycleEvents::FindingId)
                    .col(IndexFindingLifecycleEvents::CreatedAt)
                    .to_owned(),
            )
            .await?;

        install_update_guard(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_supported_backend(manager)?;
        remove_update_guard(manager).await?;
        manager
            .drop_table(
                Table::drop()
                    .table(IndexFindingLifecycleEvents::Table)
                    .to_owned(),
            )
            .await
    }
}

fn ensure_supported_backend(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    match manager.get_connection().get_database_backend() {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        DbBackend::MySql => Err(DbErr::Custom(
            "rustok-index finding lifecycle audit supports PostgreSQL and SQLite".to_owned(),
        )),
    }
}

async fn install_update_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    match connection.get_database_backend() {
        DbBackend::Postgres => {
            connection
                .execute_unprepared(&format!(
                    "CREATE FUNCTION {POSTGRES_GUARD_FUNCTION}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Index finding lifecycle audit rows cannot be rewritten'; END; $$"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {POSTGRES_GUARD_TRIGGER} BEFORE UPDATE ON {TABLE_NAME} FOR EACH ROW EXECUTE FUNCTION {POSTGRES_GUARD_FUNCTION}()"
                ))
                .await?;
            Ok(())
        }
        DbBackend::Sqlite => {
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {SQLITE_UPDATE_TRIGGER} BEFORE UPDATE ON {TABLE_NAME} BEGIN SELECT RAISE(ABORT, 'Index finding lifecycle audit rows cannot be rewritten'); END"
                ))
                .await?;
            Ok(())
        }
        DbBackend::MySql => unreachable!("unsupported backend was rejected before lifecycle DDL"),
    }
}

async fn remove_update_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    match connection.get_database_backend() {
        DbBackend::Postgres => {
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {POSTGRES_GUARD_TRIGGER} ON {TABLE_NAME}"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "DROP FUNCTION IF EXISTS {POSTGRES_GUARD_FUNCTION}()"
                ))
                .await?;
            Ok(())
        }
        DbBackend::Sqlite => {
            connection
                .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {SQLITE_UPDATE_TRIGGER}"))
                .await?;
            Ok(())
        }
        DbBackend::MySql => unreachable!("unsupported backend was rejected before lifecycle DDL"),
    }
}

#[derive(Iden)]
enum IndexFindingLifecycleEvents {
    #[iden = "index_consistency_finding_lifecycle_events"]
    Table,
    TenantId,
    CommandId,
    FindingId,
    Action,
    FromState,
    ToState,
    ActorKind,
    ActorSubject,
    Reason,
    CreatedAt,
}

#[derive(DeriveIden)]
enum IndexConsistencyFindings {
    Table,
    TenantId,
    FindingId,
}
