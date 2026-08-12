use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DbBackend},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

const COMMAND_TABLE: &str = "index_consistency_finding_repair_commands";
const DECISION_TABLE: &str = "index_consistency_finding_repair_recovery_decisions";
const REVISION_INDEX: &str = "uq_index_finding_repair_recovery_revision";
const POSTGRES_IMMUTABLE_FUNCTION: &str = "rustok_index_guard_finding_repair_recovery_immutable";
const POSTGRES_IMMUTABLE_UPDATE_TRIGGER: &str = "trg_index_finding_repair_recovery_no_update";
const POSTGRES_IMMUTABLE_DELETE_TRIGGER: &str = "trg_index_finding_repair_recovery_no_delete";
const POSTGRES_COMPLETION_FUNCTION: &str = "rustok_index_guard_finding_repair_recovery_completion";
const POSTGRES_COMPLETION_TRIGGER: &str = "trg_index_finding_repair_recovery_completion";
const SQLITE_IMMUTABLE_UPDATE_TRIGGER: &str = "trg_index_finding_repair_recovery_no_update";
const SQLITE_IMMUTABLE_DELETE_TRIGGER: &str = "trg_index_finding_repair_recovery_no_delete";
const SQLITE_COMPLETION_TRIGGER: &str = "trg_index_finding_repair_recovery_completion";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_supported_backend(manager)?;
        manager
            .create_table(
                Table::create()
                    .table(IndexFindingRepairRecoveryDecisions::Table)
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::CommandId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::DecisionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::FindingId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::PayloadDigest)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::Revision)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::Action)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::PreviousState)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::NewState)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::ActorKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::ActorSubject)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::Reason)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(IndexFindingRepairRecoveryDecisions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_index_finding_repair_recovery_decisions")
                            .col(IndexFindingRepairRecoveryDecisions::TenantId)
                            .col(IndexFindingRepairRecoveryDecisions::CommandId)
                            .col(IndexFindingRepairRecoveryDecisions::DecisionId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_index_finding_repair_recovery_command")
                            .from(
                                IndexFindingRepairRecoveryDecisions::Table,
                                (
                                    IndexFindingRepairRecoveryDecisions::TenantId,
                                    IndexFindingRepairRecoveryDecisions::CommandId,
                                ),
                            )
                            .to(
                                IndexFindingRepairCommands::Table,
                                (
                                    IndexFindingRepairCommands::TenantId,
                                    IndexFindingRepairCommands::CommandId,
                                ),
                            )
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("revision >= 0"))
                    .check(Expr::cust(
                        "length(payload_digest) = 64 AND payload_digest = lower(payload_digest)",
                    ))
                    .check(Expr::cust(
                        "action IN ('activate', 'resume', 'pause', 'abandon')",
                    ))
                    .check(Expr::cust(
                        "previous_state IN ('unclassified', 'active', 'paused')",
                    ))
                    .check(Expr::cust(
                        "new_state IN ('active', 'paused', 'abandoned')",
                    ))
                    .check(Expr::cust(
                        "(action = 'activate' AND previous_state = 'unclassified' AND new_state = 'active') OR (action = 'resume' AND previous_state IN ('unclassified', 'paused') AND new_state = 'active') OR (action = 'pause' AND previous_state = 'active' AND new_state = 'paused') OR (action = 'abandon' AND previous_state IN ('unclassified', 'active', 'paused') AND new_state = 'abandoned')",
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
                    .name(REVISION_INDEX)
                    .table(IndexFindingRepairRecoveryDecisions::Table)
                    .col(IndexFindingRepairRecoveryDecisions::TenantId)
                    .col(IndexFindingRepairRecoveryDecisions::CommandId)
                    .col(IndexFindingRepairRecoveryDecisions::Revision)
                    .unique()
                    .to_owned(),
            )
            .await?;

        install_immutability_guards(manager).await?;
        install_completion_guard(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_supported_backend(manager)?;
        remove_completion_guard(manager).await?;
        remove_immutability_guards(manager).await?;
        manager
            .drop_table(
                Table::drop()
                    .table(IndexFindingRepairRecoveryDecisions::Table)
                    .to_owned(),
            )
            .await
    }
}

fn ensure_supported_backend(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    match manager.get_connection().get_database_backend() {
        DbBackend::Postgres | DbBackend::Sqlite => Ok(()),
        DbBackend::MySql => Err(DbErr::Custom(
            "rustok-index repair recovery supports PostgreSQL and SQLite".to_owned(),
        )),
    }
}

async fn install_immutability_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    match connection.get_database_backend() {
        DbBackend::Postgres => {
            connection
                .execute_unprepared(&format!(
                    "CREATE FUNCTION {POSTGRES_IMMUTABLE_FUNCTION}() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'Index finding repair recovery decisions are immutable'; RETURN OLD; END; $$"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {POSTGRES_IMMUTABLE_UPDATE_TRIGGER} BEFORE UPDATE ON {DECISION_TABLE} FOR EACH ROW EXECUTE FUNCTION {POSTGRES_IMMUTABLE_FUNCTION}()"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {POSTGRES_IMMUTABLE_DELETE_TRIGGER} BEFORE DELETE ON {DECISION_TABLE} FOR EACH ROW EXECUTE FUNCTION {POSTGRES_IMMUTABLE_FUNCTION}()"
                ))
                .await?;
            Ok(())
        }
        DbBackend::Sqlite => {
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {SQLITE_IMMUTABLE_UPDATE_TRIGGER} BEFORE UPDATE ON {DECISION_TABLE} BEGIN SELECT RAISE(ABORT, 'Index finding repair recovery decisions are immutable'); END"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {SQLITE_IMMUTABLE_DELETE_TRIGGER} BEFORE DELETE ON {DECISION_TABLE} BEGIN SELECT RAISE(ABORT, 'Index finding repair recovery decisions are immutable'); END"
                ))
                .await?;
            Ok(())
        }
        DbBackend::MySql => unreachable!("unsupported backend was rejected"),
    }
}

async fn install_completion_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    match connection.get_database_backend() {
        DbBackend::Postgres => {
            connection
                .execute_unprepared(&format!(
                    "CREATE FUNCTION {POSTGRES_COMPLETION_FUNCTION}() RETURNS trigger LANGUAGE plpgsql AS $$ DECLARE recovery_state text; BEGIN IF OLD.state = 'prepared' AND NEW.state = 'completed' THEN SELECT new_state INTO recovery_state FROM {DECISION_TABLE} WHERE tenant_id = OLD.tenant_id AND command_id = OLD.command_id ORDER BY revision DESC LIMIT 1; IF recovery_state IS DISTINCT FROM 'active' THEN RAISE EXCEPTION 'Index finding repair command is not active'; END IF; END IF; RETURN NEW; END; $$"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {POSTGRES_COMPLETION_TRIGGER} BEFORE UPDATE ON {COMMAND_TABLE} FOR EACH ROW EXECUTE FUNCTION {POSTGRES_COMPLETION_FUNCTION}()"
                ))
                .await?;
            Ok(())
        }
        DbBackend::Sqlite => {
            connection
                .execute_unprepared(&format!(
                    "CREATE TRIGGER {SQLITE_COMPLETION_TRIGGER} BEFORE UPDATE ON {COMMAND_TABLE} WHEN OLD.state = 'prepared' AND NEW.state = 'completed' AND COALESCE((SELECT new_state FROM {DECISION_TABLE} WHERE tenant_id = OLD.tenant_id AND command_id = OLD.command_id ORDER BY revision DESC LIMIT 1), '') <> 'active' BEGIN SELECT RAISE(ABORT, 'Index finding repair command is not active'); END"
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
                    "DROP TRIGGER IF EXISTS {POSTGRES_COMPLETION_TRIGGER} ON {COMMAND_TABLE}"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "DROP FUNCTION IF EXISTS {POSTGRES_COMPLETION_FUNCTION}()"
                ))
                .await?;
            Ok(())
        }
        DbBackend::Sqlite => {
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {SQLITE_COMPLETION_TRIGGER}"
                ))
                .await?;
            Ok(())
        }
        DbBackend::MySql => unreachable!("unsupported backend was rejected"),
    }
}

async fn remove_immutability_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    match connection.get_database_backend() {
        DbBackend::Postgres => {
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {POSTGRES_IMMUTABLE_UPDATE_TRIGGER} ON {DECISION_TABLE}"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {POSTGRES_IMMUTABLE_DELETE_TRIGGER} ON {DECISION_TABLE}"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "DROP FUNCTION IF EXISTS {POSTGRES_IMMUTABLE_FUNCTION}()"
                ))
                .await?;
            Ok(())
        }
        DbBackend::Sqlite => {
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {SQLITE_IMMUTABLE_UPDATE_TRIGGER}"
                ))
                .await?;
            connection
                .execute_unprepared(&format!(
                    "DROP TRIGGER IF EXISTS {SQLITE_IMMUTABLE_DELETE_TRIGGER}"
                ))
                .await?;
            Ok(())
        }
        DbBackend::MySql => unreachable!("unsupported backend was rejected"),
    }
}

#[derive(Iden)]
enum IndexFindingRepairRecoveryDecisions {
    #[iden = "index_consistency_finding_repair_recovery_decisions"]
    Table,
    TenantId,
    CommandId,
    DecisionId,
    FindingId,
    PayloadDigest,
    Revision,
    Action,
    PreviousState,
    NewState,
    ActorKind,
    ActorSubject,
    Reason,
    CreatedAt,
}

#[derive(Iden)]
enum IndexFindingRepairCommands {
    #[iden = "index_consistency_finding_repair_commands"]
    Table,
    TenantId,
    CommandId,
}
