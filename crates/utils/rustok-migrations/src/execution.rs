//! Stepwise PostgreSQL execution with a session-bound writer lock. Independent
//! checks may use `database()` between steps, while the same lock remains held.

use sea_orm_migration::{
    migrator::{Migration, MigratorTraitSelf},
    prelude::*,
    sea_orm::{
        DatabaseConnection, DbBackend, EntityTrait, SqlxPostgresConnector, Statement,
        sqlx::{self, postgres::PgPoolOptions},
    },
    seaql_migrations::Entity as Journal,
};
use std::{
    collections::HashSet,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("migration plan must be nonempty with distinct, nonempty identities")]
    InvalidPlan,
    #[error(
        "phase must name a complete ordered prerequisite prefix and nonempty contiguous steps from the compiled plan"
    )]
    InvalidPhase,
    #[error("migration journal is before the phase prerequisites or beyond its terminal boundary")]
    JournalOutsidePhase,
    #[error("nontransactional migration {0} requires an owner-proven recovery contract")]
    UnsupportedRecovery(String),
    #[error("another migration writer holds the database lock")]
    ConcurrentWriter,
    #[error("migration journal does not match the expected prefix")]
    InvalidJournal,
    #[error("runner was interrupted or failed; close it and reconcile through a new session")]
    Interrupted,
    #[error("migration {name} failed: {source}")]
    Migration { name: String, source: DbErr },
    #[error("migration database operation failed: {0}")]
    Database(#[from] DbErr),
    #[error("migration session connection failed: {0}")]
    Connection(#[from] sqlx::Error),
    #[error("{execution}; session cleanup also failed: {cleanup}")]
    Cleanup {
        #[source]
        execution: Box<ExecutionError>,
        cleanup: DbErr,
    },
}

type Result<T> = std::result::Result<T, ExecutionError>;

/// Untrusted execution restriction, not module/release authorization. Both lists
/// are checked against the full compiled composition by `connect_phase`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionPhase {
    pub expected_prefix: Vec<String>,
    pub steps: Vec<String>,
}

impl ExecutionPhase {
    fn range(&self, migrations: &[Step]) -> Result<Range<usize>> {
        let start = self.expected_prefix.len();
        let end = start
            .checked_add(self.steps.len())
            .ok_or(ExecutionError::InvalidPhase)?;
        if self.steps.is_empty()
            || end > migrations.len()
            || !self
                .expected_prefix
                .iter()
                .chain(&self.steps)
                .zip(&migrations[..end])
                .all(|(expected, step)| expected == step.name())
        {
            return Err(ExecutionError::InvalidPhase);
        }
        Ok(start..end)
    }
}

#[derive(Clone)]
struct Step {
    name: Arc<str>,
    transaction: Option<bool>,
    migration: Arc<dyn MigrationTrait>,
}

impl MigrationName for Step {
    fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Step {
    async fn up(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        self.migration.up(manager).await
    }

    fn use_transaction(&self) -> Option<bool> {
        self.transaction
    }
}

#[async_trait::async_trait]
impl MigratorTraitSelf for Step {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(self.clone())]
    }

    async fn get_pending_migrations<C: ConnectionTrait>(
        &self,
        db: &C,
    ) -> std::result::Result<Vec<Migration>, DbErr> {
        if Journal::find_by_id(self.name()).one(db).await?.is_some() {
            return Err(DbErr::Migration(
                "migration was applied outside this run".into(),
            ));
        }
        Ok(self.get_migration_files())
    }
}

enum State {
    Ready(usize),
    Interrupted,
}

/// Owns a single PostgreSQL backend. A lost backend can never be replaced in
/// this runner, including while a migration or boundary assertion is running.
/// Call `close()` on every result path to release its connection and lock.
pub struct StepRunner {
    database: DatabaseConnection,
    migrations: Vec<Step>,
    end: usize,
    state: State,
}

impl StepRunner {
    pub async fn connect(url: &str, migrations: Vec<Box<dyn MigrationTrait>>) -> Result<Self> {
        Self::connect_plan(url, migrations, None).await
    }

    /// Execute only the requested contiguous phase while retaining the complete
    /// plan for predecessor/journal checks. Resume accepts committed prefixes
    /// inside this phase, including its end, but never outside its boundaries.
    pub async fn connect_phase(
        url: &str,
        migrations: Vec<Box<dyn MigrationTrait>>,
        phase: ExecutionPhase,
    ) -> Result<Self> {
        Self::connect_plan(url, migrations, Some(phase)).await
    }

    async fn connect_plan(
        url: &str,
        migrations: Vec<Box<dyn MigrationTrait>>,
        phase: Option<ExecutionPhase>,
    ) -> Result<Self> {
        let migrations: Vec<_> = migrations
            .into_iter()
            .map(|step| Step {
                name: step.name().into(),
                transaction: step.use_transaction(),
                migration: step.into(),
            })
            .collect();
        let names: HashSet<_> = migrations.iter().map(|step| step.name()).collect();
        if migrations.is_empty()
            || names.len() != migrations.len()
            || names.iter().any(|name| name.trim().is_empty())
        {
            return Err(ExecutionError::InvalidPlan);
        }
        let range = match phase {
            Some(phase) => phase.range(&migrations)?,
            None => 0..migrations.len(),
        };
        for step in &migrations[range.clone()] {
            if step.use_transaction() == Some(false) {
                return Err(ExecutionError::UnsupportedRecovery(step.name().into()));
            }
        }

        let connected = Arc::new(AtomicBool::new(false));
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .min_connections(0)
            .idle_timeout(None)
            .max_lifetime(None)
            .acquire_timeout(Duration::from_secs(5))
            .after_connect(move |_, _| {
                let replacement = connected.swap(true, Ordering::SeqCst);
                Box::pin(async move {
                    if replacement {
                        Err(sqlx::Error::Protocol(
                            "migration session lost; reconnect forbidden".into(),
                        ))
                    } else {
                        Ok(())
                    }
                })
            })
            .connect(url)
            .await?;
        let database = SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
        let initial = async {
            let row = database
                .query_one_raw(Statement::from_string(
                    DbBackend::Postgres,
                    "SELECT pg_try_advisory_lock(hashtext('rustok-migrate'), 1) AS acquired",
                ))
                .await?
                .ok_or_else(|| DbErr::Custom("migration lock result is missing".into()))?;
            if !row.try_get::<bool>("", "acquired")? {
                return Err(ExecutionError::ConcurrentWriter);
            }
            // A missing journal is an explicitly empty initial history. Reject
            // missing prerequisites before even creating the journal table.
            let applied = if SchemaManager::new(&database)
                .has_table("seaql_migrations")
                .await?
            {
                read_journal(&database).await?
            } else {
                HashSet::new()
            };
            verify_prefix(&migrations, &applied, applied.len())?;
            if applied.len() < range.start || applied.len() > range.end {
                return Err(ExecutionError::JournalOutsidePhase);
            }
            migrations[0].install(&database).await?;
            Ok(applied.len())
        }
        .await;
        match initial {
            Ok(next) => Ok(Self {
                database,
                migrations,
                end: range.end,
                state: State::Ready(next),
            }),
            Err(error) => match database.close().await {
                Ok(()) => Err(error),
                Err(cleanup) => Err(ExecutionError::Cleanup {
                    execution: Box::new(error),
                    cleanup,
                }),
            },
        }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.database
    }

    pub fn pending_migrations(&self) -> Result<impl Iterator<Item = &str>> {
        let State::Ready(next) = self.state else {
            return Err(ExecutionError::Interrupted);
        };
        Ok(self.migrations[next..self.end]
            .iter()
            .map(|step| step.name()))
    }

    /// Returns only after commit and exact journal-prefix verification. A failed
    /// or cancelled call poisons this runner. No next step may follow it.
    pub async fn apply_next(&mut self) -> Result<Option<String>> {
        let State::Ready(next) = self.state else {
            return Err(ExecutionError::Interrupted);
        };
        self.state = State::Interrupted;
        verify_prefix(&self.migrations, &read_journal(&self.database).await?, next)?;
        if next == self.end {
            self.state = State::Ready(next);
            return Ok(None);
        }
        let step = &self.migrations[next];
        let name = step.name().to_owned();
        MigratorTraitSelf::up(step, &self.database, Some(1))
            .await
            .map_err(|source| ExecutionError::Migration {
                name: name.clone(),
                source,
            })?;
        verify_prefix(
            &self.migrations,
            &read_journal(&self.database).await?,
            next + 1,
        )?;
        self.state = State::Ready(next + 1);
        Ok(Some(name))
    }

    /// Releases resources even when execution stopped early or failed. Closing
    /// a runner does not certify completion or authorize retrying partial work.
    pub async fn close(self) -> Result<()> {
        self.database.close().await?;
        Ok(())
    }
}

async fn read_journal(database: &DatabaseConnection) -> Result<HashSet<String>> {
    let rows = Journal::find().all(database).await?;
    let count = rows.len();
    let names: HashSet<_> = rows.into_iter().map(|row| row.version).collect();
    if names.len() != count {
        return Err(ExecutionError::InvalidJournal);
    }
    Ok(names)
}

fn verify_prefix(migrations: &[Step], applied: &HashSet<String>, expected: usize) -> Result<()> {
    if expected > migrations.len()
        || applied.len() != expected
        || !migrations[..expected]
            .iter()
            .all(|step| applied.contains(step.name()))
    {
        return Err(ExecutionError::InvalidJournal);
    }
    Ok(())
}
