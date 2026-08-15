use std::{
    fmt,
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};
use tokio::runtime::{Builder, Runtime};

use super::{keyring, keyring_schedule_persistence as persistence};

pub const COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY: &str =
    "comments_tcp_delegation_schedule";
pub const COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE: &str =
    "blog_comments_tcp_delegation_schedule_state";

const POSTGRES_STORE_QUEUE_CAPACITY: usize = 1;
const COMMIT_RECONCILIATION_ATTEMPTS: usize = 20;
const COMMIT_RECONCILIATION_DELAY_MS: u64 = 100;

type StoreResult =
    std::result::Result<(), persistence::CommentsTcpDelegationSchedulePersistenceStoreError>;

#[derive(Clone)]
pub struct PostgresCommentsTcpDelegationSchedulePersistenceStore {
    commands: SyncSender<PostgresStoreCommand>,
}

enum PostgresStoreCommand {
    VerifyCurrent {
        expected: persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        response: SyncSender<StoreResult>,
    },
    CompareAndStore {
        expected: Option<persistence::CommentsTcpDelegationSchedulePersistenceRecord>,
        candidate: persistence::CommentsTcpDelegationSchedulePersistenceRecord,
        response: SyncSender<StoreResult>,
    },
}

enum PostgresWorkerStartup {
    Ready,
    Failed,
}

#[derive(Clone, Eq, PartialEq)]
struct StoredScheduleRecord {
    schema_version: i16,
    source: keyring::CommentsTcpDelegationKeyringSource,
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
            source: record.source(),
            generation,
            schedule_digest_hex: record.schedule_digest().to_hex(),
        })
    }
}

type StoreResultWith<T> =
    std::result::Result<T, persistence::CommentsTcpDelegationSchedulePersistenceStoreError>;

impl PostgresCommentsTcpDelegationSchedulePersistenceStore {
    pub fn new(database: DatabaseConnection) -> std::result::Result<Self, String> {
        if database.get_database_backend() != DbBackend::Postgres {
            return Err(
                "Comments TCP delegation schedule PostgreSQL persistence requires a PostgreSQL database"
                    .to_string(),
            );
        }

        let (commands, receiver) = mpsc::sync_channel(POSTGRES_STORE_QUEUE_CAPACITY);
        let (startup_sender, startup_receiver) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("comments-delegation-schedule-postgres".to_string())
            .spawn(move || {
                let runtime = match Builder::new_current_thread().enable_all().build() {
                    Ok(runtime) => runtime,
                    Err(_) => {
                        let _ = startup_sender.send(PostgresWorkerStartup::Failed);
                        return;
                    }
                };
                if startup_sender.send(PostgresWorkerStartup::Ready).is_err() {
                    return;
                }
                run_postgres_store_worker(runtime, database, receiver);
            })
            .map_err(|_| {
                "Comments TCP delegation schedule PostgreSQL persistence worker could not start"
                    .to_string()
            })?;

        match startup_receiver.recv() {
            Ok(PostgresWorkerStartup::Ready) => Ok(Self { commands }),
            Ok(PostgresWorkerStartup::Failed) | Err(_) => Err(
                "Comments TCP delegation schedule PostgreSQL persistence worker is unavailable"
                    .to_string(),
            ),
        }
    }

    pub fn into_shared(self) -> persistence::SharedCommentsTcpDelegationSchedulePersistenceStore {
        Arc::new(self)
    }

    fn request(
        &self,
        build: impl FnOnce(SyncSender<StoreResult>) -> PostgresStoreCommand,
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

impl persistence::CommentsTcpDelegationSchedulePersistenceStore
    for PostgresCommentsTcpDelegationSchedulePersistenceStore
{
    fn verify_current(
        &self,
        expected: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> StoreResult {
        self.request(|response| PostgresStoreCommand::VerifyCurrent {
            expected: *expected,
            response,
        })
    }

    fn compare_and_store(
        &self,
        expected: Option<&persistence::CommentsTcpDelegationSchedulePersistenceRecord>,
        candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
    ) -> StoreResult {
        self.request(|response| PostgresStoreCommand::CompareAndStore {
            expected: expected.copied(),
            candidate: *candidate,
            response,
        })
    }
}

impl fmt::Debug for PostgresCommentsTcpDelegationSchedulePersistenceStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresCommentsTcpDelegationSchedulePersistenceStore")
            .field("database", &"[CONFIGURED]")
            .field(
                "state_key",
                &COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY,
            )
            .finish()
    }
}

fn run_postgres_store_worker(
    runtime: Runtime,
    database: DatabaseConnection,
    receiver: Receiver<PostgresStoreCommand>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            PostgresStoreCommand::VerifyCurrent { expected, response } => {
                let result = runtime.block_on(verify_current_on_postgres(&database, &expected));
                let _ = response.send(result);
            }
            PostgresStoreCommand::CompareAndStore {
                expected,
                candidate,
                response,
            } => {
                let result = runtime.block_on(compare_and_store_on_postgres(
                    &database,
                    expected.as_ref(),
                    &candidate,
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
        Ok(_) => Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict),
        Err(sea_orm::DbErr::Type(_)) => {
            Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Conflict)
        }
        Err(_) => Err(persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable),
    }
}

async fn compare_and_store_on_postgres(
    database: &DatabaseConnection,
    expected: Option<&persistence::CommentsTcpDelegationSchedulePersistenceRecord>,
    candidate: &persistence::CommentsTcpDelegationSchedulePersistenceRecord,
) -> StoreResult {
    let expected = expected
        .map(StoredScheduleRecord::from_public)
        .transpose()?;
    let candidate = StoredScheduleRecord::from_public(candidate)?;

    let transaction = database.begin().await.map_err(|_| {
        persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable
    })?;
    let execution = match expected.as_ref() {
        Some(expected) => {
            transaction
                .execute(update_statement(expected, &candidate))
                .await
        }
        None => transaction.execute(insert_statement(&candidate)).await,
    };

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
        Err(_) => reconcile_ambiguous_commit(database, expected.as_ref(), &candidate).await,
    }
}

async fn reconcile_ambiguous_commit(
    database: &DatabaseConnection,
    expected: Option<&StoredScheduleRecord>,
    candidate: &StoredScheduleRecord,
) -> StoreResult {
    for attempt in 0..COMMIT_RECONCILIATION_ATTEMPTS {
        match read_current_record(database).await {
            Ok(Some(current)) if &current == candidate => return Ok(()),
            Ok(current) if current.as_ref() == expected => {
                return Err(
                    persistence::CommentsTcpDelegationSchedulePersistenceStoreError::Unavailable,
                );
            }
            Ok(_) => abort_on_indeterminate_postgres_commit(),
            Err(_) if attempt + 1 < COMMIT_RECONCILIATION_ATTEMPTS => {
                tokio::time::sleep(Duration::from_millis(COMMIT_RECONCILIATION_DELAY_MS)).await;
            }
            Err(_) => abort_on_indeterminate_postgres_commit(),
        }
    }
    abort_on_indeterminate_postgres_commit()
}

async fn read_current_record(
    database: &DatabaseConnection,
) -> std::result::Result<Option<StoredScheduleRecord>, sea_orm::DbErr> {
    let row = database
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            format!(
                "SELECT schema_version, source, generation, schedule_digest_hex \
                 FROM {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE} \
                 WHERE state_key = $1"
            ),
            vec![COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into()],
        ))
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };

    let schema_version: i16 = row.try_get("", "schema_version")?;
    let source: String = row.try_get("", "source")?;
    let generation: i64 = row.try_get("", "generation")?;
    let schedule_digest_hex: String = row.try_get("", "schedule_digest_hex")?;
    let source = match source.as_str() {
        "host_provided" => keyring::CommentsTcpDelegationKeyringSource::HostProvided,
        "file" => keyring::CommentsTcpDelegationKeyringSource::File,
        _ => {
            return Err(sea_orm::DbErr::Type(
                "invalid Comments delegation schedule persistence source".to_string(),
            ));
        }
    };
    if generation <= 0
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

fn insert_statement(candidate: &StoredScheduleRecord) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "INSERT INTO {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE} \
             (state_key, schema_version, source, generation, schedule_digest_hex, updated_at) \
             VALUES ($1, $2, $3, $4, $5, NOW()) \
             ON CONFLICT (state_key) DO NOTHING"
        ),
        vec![
            COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into(),
            candidate.schema_version.into(),
            source_text(candidate.source).into(),
            candidate.generation.into(),
            candidate.schedule_digest_hex.clone().into(),
        ],
    )
}

fn update_statement(
    expected: &StoredScheduleRecord,
    candidate: &StoredScheduleRecord,
) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        format!(
            "UPDATE {COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_TABLE} \
             SET schema_version = $2, source = $3, generation = $4, \
                 schedule_digest_hex = $5, updated_at = NOW() \
             WHERE state_key = $1 \
               AND schema_version = $6 \
               AND source = $7 \
               AND generation = $8 \
               AND schedule_digest_hex = $9"
        ),
        vec![
            COMMENTS_TCP_DELEGATION_SCHEDULE_POSTGRES_STATE_KEY.into(),
            candidate.schema_version.into(),
            source_text(candidate.source).into(),
            candidate.generation.into(),
            candidate.schedule_digest_hex.clone().into(),
            expected.schema_version.into(),
            source_text(expected.source).into(),
            expected.generation.into(),
            expected.schedule_digest_hex.clone().into(),
        ],
    )
}

fn source_text(source: keyring::CommentsTcpDelegationKeyringSource) -> &'static str {
    match source {
        keyring::CommentsTcpDelegationKeyringSource::HostProvided => "host_provided",
        keyring::CommentsTcpDelegationKeyringSource::File => "file",
    }
}

fn abort_on_indeterminate_postgres_commit() -> ! {
    std::process::abort()
}
