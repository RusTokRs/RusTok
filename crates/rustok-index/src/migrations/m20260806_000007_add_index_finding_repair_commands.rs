use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DbBackend},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const TABLE_NAME: &str = "index_consistency_finding_repair_commands";
const ACTIVE_INDEX: &str = "uq_index_finding_repair_active";
const FINDING_INDEX: &str = "idx_index_finding_repair_finding";
const POSTGRES_GUARD_FUNCTION: &str = "rustok_index_guard_finding_repair_completion";
const POSTGRES_GUARD_TRIGGER: &str = "trg_index_finding_repair_completion_guard";
const SQLITE_GUARD_TRIGGER: &str = "trg_index_finding_repair_completion_guard";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_supported_backend(manager)?;
        manager
            .create_table(
                Table::create()
                    .table(IndexFindingRepairCommands::Table)
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::CommandId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::FindingId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::PayloadDigest)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::TargetKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::ActorKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::ActorSubject)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::Reason)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::State)
                            .string_len(16)
                            .not_null()
                            .default("prepared"),
                    )
                    .col(ColumnDef::new(IndexFindingRepairCommands::Outcome).string_len(16))
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::OutcomeCode)
                            .string_len(128),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::OwnerName)
                            .string_len(128),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::BeforeDigest)
                            .string_len(64),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::AfterDigest)
                            .string_len(64),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::OwnerReceiptDigest)
                            .string_len(64),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairCommands::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_finding_repair_commands")
                            .col(IndexFindingRepairCommands::TenantId)
                            .col(IndexFindingRepairCommands::CommandId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_finding_repair_command_finding")
                            .from(
                                IndexFindingRepairCommands::Table,
                                (
                                    IndexFindingRepairCommands::TenantId,
                                    IndexFindingRepairCommands::FindingId,
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
                    .check(Expr::cust(
                        "length(payload_digest) = 64 AND payload_digest = lower(payload_digest)",
                    ))
                    .check(Expr::cust(
                        "target_kind IN ('missing_entity', 'orphan_link')",
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
                    .check(Expr::cust("state IN ('prepared', 'completed')"))
                    .check(Expr::cust(
                        "outcome IS NULL OR outcome IN ('repaired', 'not_repaired')",
                    ))
                    .check(Expr::cust(
                        "outcome_code IS NULL OR (length(outcome_code) BETWEEN 1 AND 128 AND outcome_code = lower(outcome_code))",
                    ))
                    .check(Expr::cust(
                        "owner_name IS NULL OR (length(owner_name) BETWEEN 1 AND 128 AND owner_name = lower(owner_name))",
                    ))
                    .check(Expr::cust(
                        "before_digest IS NULL OR (length(before_digest) = 64 AND before_digest = lower(before_digest))",
                    ))
                    .check(Expr::cust(
                        "after_digest IS NULL OR (length(after_digest) = 64 AND after_digest = lower(after_digest))",
                    ))
                    .check(Expr::cust(
                        "owner_receipt_digest IS NULL OR (length(owner_receipt_digest) = 64 AND owner_receipt_digest = lower(owner_receipt_digest))",
                    ))
                    .check(Expr::cust(
                        "(state = 'prepared' AND outcome IS NULL AND outcome_code IS NULL AND owner_name IS NULL AND before_digest IS NULL AND after_digest IS NULL AND owner_receipt_digest IS NULL AND completed_at IS NULL) OR (state = 'completed' AND outcome IS NOT NULL AND owner_name IS NOT NULL AND before_digest IS NOT NULL AND completed_at IS NOT NULL AND ((outcome = 'repaired' AND outcome_code IS NULL AND after_digest IS NOT NULL AND owner_receipt_digest IS NOT NULL) OR (outcome = 'not_repaired' AND outcome_code IS NOT NULL)))",
                    ))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name(FINDING_INDEX)
                    .table(IndexFindingRepairCommands::Table)
                    .col(IndexFindingRepairCommands::TenantId)
                    .col(IndexFindingRepairCommands::FindingId)
                    .col(IndexFindingRepairCommands::CreatedAt)
                    .to_owned(),
            )
            .await?;

        install_active_index(manager).await?;
        install_completion_guard(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_supported_backend(manager)?;
        remove_completion_guard(manager).await?;
        manager
            .drop_table(
                Table::drop()
                    .table(IndexFindingRepairCommands::Table)
                    .to_owned(),
            )
            .await
    }
}

fn ensure_supported_backend(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    match manager.get_connection().get_database_backend() {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        DbBackend::MySql => Err(DbErr::Custom(
            "rustok-index targeted repair receipts support PostgreSQL and SQLite".to_owned(),
        )),
    }
}

async fn install_active_index(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            "CREATE UNIQUE INDEX {ACTIVE_INDEX} ON {TABLE_NAME} (tenant_id, finding_id) WHERE state = 'prepared'"
        ))
        .await?;
    Ok(())
}

async fn install_completion_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    match connection.get_database_backend() {
        DbBackend::Postgres => {
            connection
                .execute_unprepared(&format!(
                    "CREATE FUNCTION {POSTGRES_GUARD_FUNCTION}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.state <> 'prepared' OR NEW.state <> 'completed' OR OLD.tenant_id <> NEW.tenant_id OR OLD.command_id <> NEW.command_id OR OLD.finding_id <> NEW.finding_id OR OLD.payload_digest <> NEW.payload_digest OR OLD.target_kind <> NEW.target_kind OR OLD.actor_kind <> NEW.actor_kind OR OLD.actor_subject <> NEW.actor_subject OR OLD.reason <> NEW.reason THEN RAISE EXCEPTION 'Index finding repair receipt transition is invalid'; END IF; RETURN NEW; END; $$"
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
                    "CREATE TRIGGER {SQLITE_GUARD_TRIGGER} BEFORE UPDATE ON {TABLE_NAME} WHEN OLD.state <> 'prepared' OR NEW.state <> 'completed' OR OLD.tenant_id <> NEW.tenant_id OR OLD.command_id <> NEW.command_id OR OLD.finding_id <> NEW.finding_id OR OLD.payload_digest <> NEW.payload_digest OR OLD.target_kind <> NEW.target_kind OR OLD.actor_kind <> NEW.actor_kind OR OLD.actor_subject <> NEW.actor_subject OR OLD.reason <> NEW.reason BEGIN SELECT RAISE(ABORT, 'Index finding repair receipt transition is invalid'); END"
                ))
                .await?;
            Ok(())
        }
        DbBackend::MySql => unreachable!("unsupported backend was rejected"),
    }
}

async fn remove_completion_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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
                .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {SQLITE_GUARD_TRIGGER}"))
                .await?;
            Ok(())
        }
        DbBackend::MySql => unreachable!("unsupported backend was rejected"),
    }
}

#[derive(Iden)]
enum IndexFindingRepairCommands {
    #[iden = "index_consistency_finding_repair_commands"]
    Table,
    TenantId,
    CommandId,
    FindingId,
    PayloadDigest,
    TargetKind,
    ActorKind,
    ActorSubject,
    Reason,
    State,
    Outcome,
    OutcomeCode,
    OwnerName,
    BeforeDigest,
    AfterDigest,
    OwnerReceiptDigest,
    CreatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum IndexConsistencyFindings {
    Table,
    TenantId,
    FindingId,
}
