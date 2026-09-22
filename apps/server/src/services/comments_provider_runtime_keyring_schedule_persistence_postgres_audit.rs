use std::{
    fmt,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::Duration,
};

use rustok_api::AuthPrincipalKind;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use tokio::runtime::{Builder, Runtime};
use uuid::Uuid;

use super::{
    keyring, keyring_schedule_persistence as persistence,
    keyring_schedule_persistence_postgres as postgres, keyring_schedule_trigger as trigger,
};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE: &str =
    "blog_comments_tcp_delegation_schedule_audit_outbox";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_SCHEMA_VERSION: u16 = 1;
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_EVENT_TYPE: &str =
    "comments_tcp_delegation_schedule_replaced";

const POSTGRES_AUDIT_STORE_QUEUE_CAPACITY: usize = 1;
const COMMIT_RECONCILIATION_ATTEMPTS: usize = 20;
const COMMIT_RECONCILIATION_DELAY_MS: u64 = 100;
const REPLACEMENT_SUCCEEDED_OUTCOME: &str = "replacement_succeeded";

type StoreResult =
    std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>;
type StoreResultWith<T> =
    std::result::Result<T, persistence::CommentsTcpDelegationSchedulePersistenceStoreError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CommentsTcpDelegationSchedulePostgresAuditContext {
    request_id: Uuid,
    actor_id: Uuid,
    principal_kind: AuthPrincipalKind,
    operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
    occurred_at_unix_ms: u64,
}

impl CommentsTcpDelegationSchedulePostgresAuditContext {
    pub(super) fn new(
        context: trigger::CommentsTcpDelegationScheduleTriggerContext,
        operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
        occurred_at_unix_ms: u64,
    ) -> std::result::Result<Self, String> {
        if context.request_id().is_nil() || context.actor_id().is_nil() {
            return Err(
                "Comments TCP delegation schedule durable audit identity is invalid".to_string(),
            );
        }
        Ok(Self {
            request_id: context.request_id(),
            actor_id: context.actor_id(),
            principal_kind: context.principal_kind(),
            operation,
            occurred_at_unix_ms,
        })
    }
}

#[derive(Clone)]
pub struct PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore {
    commands: SyncSender<PostgresAuditStoreCommand>,
}

enum PostgresAuditStoreCommand {
    VerifyCurrent {
        expected: persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        response: SyncSender<StoreResult>,
    },
    BootstrapEmpty {
        candidate: persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        response: SyncSender<StoreResult>,
    },
    CompareAndStoreWithAudit {
        expected: persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        candidate: persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        audit: CommentsTcpDelegationSchedulePostgresAuditContext,
        response: SyncSender<StoreResult>,
    },
}

enum PostgresAuditWorkerStartup {
    Ready,
    Failed,
}

#[derive(Clone, Eq, PartialEq)]
struct StoredScheduleRecord {
    schema_version: i16,
    source: String,
    generation: i64,
    schedule_digest_hex: String,
}

impl StoredScheduleRecord {
    fn from_public(
        record: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> StoreResultWith<Self> {
        let schema_version = i16::try_from(record.schema_version()).map_err(|_| {
            persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
        })?;
        let generation = i64::try_from(record.generation()).map_err(|_| {
            persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
        })?;
        Ok(Self {
            schema_version,
            source: source_text(record.source()).to_string(),
            generation,
            schedule_digest_hex: record.schedule_digest().to_hex(),
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
struct StoredAuditRecord {
    audit_schema_version: i16,
    request_id: Uuid,
    state_key: String,
    event_type: String,
    occurred_at_unix_ms: i64,
    actor_id: Uuid,
    principal_kind: String,
    operation: String,
    source: String,
    previous_generation: i64,
    candidate_generation: i64,
    outcome: String,
}

impl StoredAuditRecord {
    fn from_context(
        context: &CommentsTcpDelegationSchedulePostgresAuditContext,
        expected: &StoredScheduleRecord,
        candidate: &StoredScheduleRecord,
    ) -> StoreResultWith<Self> {
        if expected.source != candidate.source || candidate.generation <= expected.generation {
            return Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict);
        }
        let audit_schema_version =
            i16::try_from(COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_SCHEMA_VERSION).map_err(
                |_| persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
            )?;
        let occurred_at_unix_ms = i64::try_from(context.occurred_at_unix_ms).map_err(|_| {
            persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
        })?;
        Ok(Self {
            audit_schema_version,
            request_id: context.request_id,
            state_key: postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.to_string(),
            event_type: COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_EVENT_TYPE.to_string(),
            occurred_at_unix_ms,
            actor_id: context.actor_id,
            principal_kind: principal_kind_text(context.principal_kind)
                .ok_or(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict)?
                .to_string(),
            operation: operation_text(context.operation).to_string(),
            source: candidate.source.clone(),
            previous_generation: expected.generation,
            candidate_generation: candidate.generation,
            outcome: REPLACEMENT_SUCCEEDED_OUTCOME.to_string(),
        })
    }
}

impl PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore {
    pub fn new(database: DatabaseConnection) -> std::result::Result<Self, String> {
        if database.get_database_backend() != DbBackend::Postgres {
            return Err(
                "Comments TCP delegation schedule audited persistence requires a PostgreSQL database"
                    .to_string(),
            );
        }

        let (commands, receiver) = mpsc::sync_channel(POSTGRES_AUDIT_STORE_QUEUE_CAPACITY);
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("comments-delegation-schedule-postgres-audit".to_string())
            .spawn(move || {
                let runtime = match Builder::new_current_thread().enable_all().build() {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        let _ = startup_sender.send(PostgresAuditWorkerStartup::Failed);
                        return;
                    }
                };
                if startup_sender
                    .send(PostgresAuditWorkerStartup::Ready)
                    .is_err()
                {
                    return;
                }
                run_postgres_audit_store_worker(runtime, database, receiver);
            })
            .map_err(|_| {
                "Comments TCP delegation schedule audited persistence worker could not start"
                    .to_string()
            })?;

        match startup_receiver.recv() {
            Ok(PostgresAuditWorkerStartup::Ready) => Ok(Self { commands }),
            Ok(PostgresAuditWorkerStartup::Failed) | Err(_) => Err(
                "Comments TCP delegation schedule audited persistence worker is unavailable"
                    .to_string(),
            ),
        }
    }

    pub(super) fn verify_current(
        &self,
        expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> StoreResult {
        self.request(|response| PostgresAuditStoreCommand::VerifyCurrent {
            expected: *expected,
            response,
        })
    }

    pub(super) fn bootstrap_empty(
        &self,
        candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> StoreResult {
        self.request(|response| PostgresAuditStoreCommand::BootstrapEmpty {
            candidate: *candidate,
            response,
        })
    }

    pub(super) fn compare_and_store_with_audit(
        &self,
        expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        audit: &CommentsTcpDelegationSchedulePostgresAuditContext,
    ) -> StoreResult {
        self.request(
            |response| PostgresAuditStoreCommand::CompareAndStoreWithAudit {
                expected: *expected,
                candidate: *candidate,
                audit: *audit,
                response,
            },
        )
    }

    fn request(
        &self,
        build: impl FnOnce(SyncSender<StoreResult>) -> PostgresAuditStoreCommand,
    ) -> StoreResult {
        let (response_sender, response_receiver) = mpsc::sync_channel(1);
        self.commands.send(build(response_sender)).map_err(|_| {
            persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
        })?;
        response_receiver.recv().unwrap_or(Err(
            persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
        ))
    }
}

impl fmt::Debug for PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCommentsTcpDelegationScheduleAuditedPersistenceStore")
            .field("database", &"[CONFIGURED]")
            .field(
                "state_key",
                &postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY,
            )
            .field("audit_outbox", &"[CONFIGURED]")
            .finish()
    }
}

fn run_postgres_audit_store_worker(
    runtime: Runtime,
    database: DatabaseConnection,
    receiver: Receiver<PostgresAuditStoreCommand>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            PostgresAuditStoreCommand::VerifyCurrent { expected, response } => {
                let result = runtime.block_on(verify_current_on_postgres(&database, &expected));
                let _ = response.send(result);
            }
            PostgresAuditStoreCommand::BootstrapEmpty {
                candidate,
                response,
            } => {
                let result = runtime.block_on(bootstrap_empty_on_postgres(&database, &candidate));
                let _ = response.send(result);
            }
            PostgresAuditStoreCommand::CompareAndStoreWithAudit {
                expected,
                candidate,
                audit,
                response,
            } => {
                let result = runtime.block_on(compare_and_store_with_audit_on_postgres(
                    &database, &expected, &candidate, &audit,
                ));
                let _ = response.send(result);
            }
        }
    }
}

async fn verify_current_on_postgres(
    database: &DatabaseConnection,
    expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
) -> StoreResult {
    let expected = StoredScheduleRecord::from_public(expected)?;
    match read_current_record(database).await {
        Ok(Some(current)) if current == expected => Ok(()),
        Ok(_) | Err(sea_orm::DbErr::Type(_)) => {
            Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict)
        }
        Err(_) => Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable),
    }
}

async fn bootstrap_empty_on_postgres(
    database: &DatabaseConnection,
    candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
) -> StoreResult {
    let candidate = StoredScheduleRecord::from_public(candidate)?;
    let transaction = database.begin().await.map_err(|_| {
        persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
    })?;
    let execution = transaction
        .execute_raw(insert_state_statement(&candidate))
        .await;
    let rows_affected = match execution {
        Ok(result) => result.rows_affected(),
        Err(_) => {
            let _ = transaction.rollback().await;
            return Err(
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
            );
        }
    };
    if rows_affected != 1 {
        let _ = transaction.rollback().await;
        return Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict);
    }
    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(_) => reconcile_ambiguous_bootstrap(database, &candidate).await,
    }
}

async fn compare_and_store_with_audit_on_postgres(
    database: &DatabaseConnection,
    expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    audit: &CommentsTcpDelegationSchedulePostgresAuditContext,
) -> StoreResult {
    let expected = StoredScheduleRecord::from_public(expected)?;
    let candidate = StoredScheduleRecord::from_public(candidate)?;
    let audit = StoredAuditRecord::from_context(audit, &expected, &candidate)?;

    let transaction = database.begin().await.map_err(|_| {
        persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
    })?;
    let state_execution = transaction
        .execute_raw(update_state_statement(&expected, &candidate))
        .await;
    let state_rows = match state_execution {
        Ok(result) => result.rows_affected(),
        Err(_) => {
            let _ = transaction.rollback().await;
            return Err(
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
            );
        }
    };
    if state_rows != 1 {
        let _ = transaction.rollback().await;
        return Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict);
    }

    let audit_execution = transaction
        .execute_raw(insert_audit_statement(&audit))
        .await;
    let audit_rows = match audit_execution {
        Ok(result) => result.rows_affected(),
        Err(_) => {
            let _ = transaction.rollback().await;
            return Err(
                persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
            );
        }
    };
    if audit_rows != 1 {
        let _ = transaction.rollback().await;
        return Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict);
    }

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(_) => reconcile_ambiguous_audited_commit(database, &candidate, &audit).await,
    }
}

async fn reconcile_ambiguous_bootstrap(
    database: &DatabaseConnection,
    candidate: &StoredScheduleRecord,
) -> StoreResult {
    for attempt in 0..COMMIT_RECONCILIATION_ATTEMPTS {
        match read_current_record(database).await {
            Ok(Some(current)) if &current == candidate => return Ok(()),
            Err(_) if attempt + 1 < COMMIT_RECONCILIATION_ATTEMPTS => {
                tokio::time::sleep(Duration::from_millis(COMMIT_RECONCILIATION_DELAY_MS)).await;
            }
            _ => abort_on_indeterminate_postgres_audit_commit(),
        }
    }
    abort_on_indeterminate_postgres_audit_commit()
}

async fn reconcile_ambiguous_audited_commit(
    database: &DatabaseConnection,
    candidate: &StoredScheduleRecord,
    audit: &StoredAuditRecord,
) -> StoreResult {
    for attempt in 0..COMMIT_RECONCILIATION_ATTEMPTS {
        let state = read_current_record(database).await;
        let outbox = read_audit_record(database, audit.request_id).await;
        match (state, outbox) {
            (Ok(Some(current)), Ok(Some(current_audit)))
                if &current == candidate && &current_audit == audit =>
            {
                return Ok(());
            }
            (Err(_), _) | (_, Err(_)) if attempt + 1 < COMMIT_RECONCILIATION_ATTEMPTS => {
                tokio::time::sleep(Duration::from_millis(COMMIT_RECONCILIATION_DELAY_MS)).await;
            }
            _ => abort_on_indeterminate_postgres_audit_commit(),
        }
    }
    abort_on_indeterminate_postgres_audit_commit()
}

async fn read_current_record(
    database: &DatabaseConnection,
) -> std::result::Result<Option<StoredScheduleRecord>, sea_orm::DbErr> {
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT schema_version, source, generation, schedule_digest_hex \
                 FROM {} WHERE state_key = $1",
                postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE,
            ),
            vec![postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into()],
        ))
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let schema_version: i16 = row.try_get("", "schema_version")?;
    let source: String = row.try_get("", "source")?;
    let generation: i64 = row.try_get("", "generation")?;
    let schedule_digest_hex: String = row.try_get("", "schedule_digest_hex")?;
    if schema_version != 1
        || !matches!(source.as_str(), "host_provided" | "file")
        || generation <= 0
        || schedule_digest_hex.len() != 64
        || hex::decode(&schedule_digest_hex).is_err()
    {
        return Err(sea_orm::DbErr::Type(
            "invalid Comments delegation schedule persistence record".to_string(),
        ));
    }
    Ok(Some(StoredScheduleRecord {
        schema_version,
        source,
        generation,
        schedule_digest_hex,
    }))
}

async fn read_audit_record(
    database: &DatabaseConnection,
    request_id: Uuid,
) -> std::result::Result<Option<StoredAuditRecord>, sea_orm::DbErr> {
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT audit_schema_version, request_id, state_key, event_type, \
                 occurred_at_unix_ms, actor_id, principal_kind, operation, source, \
                 previous_generation, candidate_generation, outcome \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE} \
                 WHERE request_id = $1",
            ),
            vec![request_id.into()],
        ))
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let record = StoredAuditRecord {
        audit_schema_version: row.try_get("", "audit_schema_version")?,
        request_id: row.try_get("", "request_id")?,
        state_key: row.try_get("", "state_key")?,
        event_type: row.try_get("", "event_type")?,
        occurred_at_unix_ms: row.try_get("", "occurred_at_unix_ms")?,
        actor_id: row.try_get("", "actor_id")?,
        principal_kind: row.try_get("", "principal_kind")?,
        operation: row.try_get("", "operation")?,
        source: row.try_get("", "source")?,
        previous_generation: row.try_get("", "previous_generation")?,
        candidate_generation: row.try_get("", "candidate_generation")?,
        outcome: row.try_get("", "outcome")?,
    };
    if record.audit_schema_version != 1
        || record.request_id.is_nil()
        || record.actor_id.is_nil()
        || record.state_key != postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY
        || record.event_type != COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_EVENT_TYPE
        || record.occurred_at_unix_ms <= 0
        || !matches!(record.principal_kind.as_str(), "direct_user" | "service")
        || !matches!(
            record.operation.as_str(),
            "reload_file" | "replace_host_schedule"
        )
        || !matches!(record.source.as_str(), "host_provided" | "file")
        || record.previous_generation <= 0
        || record.candidate_generation <= record.previous_generation
        || record.outcome != REPLACEMENT_SUCCEEDED_OUTCOME
    {
        return Err(sea_orm::DbErr::Type(
            "invalid Comments delegation schedule durable audit record".to_string(),
        ));
    }
    Ok(Some(record))
}

fn insert_state_statement(candidate: &StoredScheduleRecord) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "INSERT INTO {} \
             (state_key, schema_version, source, generation, schedule_digest_hex, updated_at) \
             VALUES ($1, $2, $3, $4, $5, NOW()) \
             ON CONFLICT (state_key) DO NOTHING",
            postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE,
        ),
        vec![
            postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into(),
            candidate.schema_version.into(),
            candidate.source.clone().into(),
            candidate.generation.into(),
            candidate.schedule_digest_hex.clone().into(),
        ],
    )
}

fn update_state_statement(
    expected: &StoredScheduleRecord,
    candidate: &StoredScheduleRecord,
) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "UPDATE {} SET schema_version = $2, source = $3, generation = $4, \
             schedule_digest_hex = $5, updated_at = NOW() \
             WHERE state_key = $1 AND schema_version = $6 AND source = $7 \
             AND generation = $8 AND schedule_digest_hex = $9",
            postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE,
        ),
        vec![
            postgres::COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into(),
            candidate.schema_version.into(),
            candidate.source.clone().into(),
            candidate.generation.into(),
            candidate.schedule_digest_hex.clone().into(),
            expected.schema_version.into(),
            expected.source.clone().into(),
            expected.generation.into(),
            expected.schedule_digest_hex.clone().into(),
        ],
    )
}

fn insert_audit_statement(audit: &StoredAuditRecord) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "INSERT INTO {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_AUDIT_OUTBOX_TABLE} \
             (audit_schema_version, request_id, state_key, event_type, occurred_at_unix_ms, \
              actor_id, principal_kind, operation, source, previous_generation, \
              candidate_generation, outcome, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW()) \
             ON CONFLICT DO NOTHING",
        ),
        vec![
            audit.audit_schema_version.into(),
            audit.request_id.into(),
            audit.state_key.clone().into(),
            audit.event_type.clone().into(),
            audit.occurred_at_unix_ms.into(),
            audit.actor_id.into(),
            audit.principal_kind.clone().into(),
            audit.operation.clone().into(),
            audit.source.clone().into(),
            audit.previous_generation.into(),
            audit.candidate_generation.into(),
            audit.outcome.clone().into(),
        ],
    )
}

fn source_text(source: keyring::CommentsTcpDelegationKeyringSource) -> &'static str {
    match source {
        keyring::CommentsTcpDelegationKeyringSource::HostProvided => "host_provided",
        keyring::CommentsTcpDelegationKeyringSource::File => "file",
    }
}

fn principal_kind_text(kind: AuthPrincipalKind) -> Option<&'static str> {
    match kind {
        AuthPrincipalKind::DirectUser => Some("direct_user"),
        AuthPrincipalKind::Service => Some("service"),
        _ => None,
    }
}

fn operation_text(
    operation: trigger::CommentsTcpDelegationScheduleTriggerOperation,
) -> &'static str {
    match operation {
        trigger::CommentsTcpDelegationScheduleTriggerOperation::ReloadFile => "reload_file",
        trigger::CommentsTcpDelegationScheduleTriggerOperation::ReplaceHostSchedule => {
            "replace_host_schedule"
        }
    }
}

fn abort_on_indeterminate_postgres_audit_commit() -> ! {
    std::process::abort()
}
