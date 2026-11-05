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
}

type Result<T> = std::result::Result<T, ExecutionError>;

#[derive(Clone)]
struct Step(Arc<dyn MigrationTrait>);

impl MigrationName for Step {
    fn name(&self) -> &str {
        self.0.name()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Step {
    async fn up(&self, manager: &SchemaManager) -> std::result::Result<(), DbErr> {
        self.0.up(manager).await
    }

    fn use_transaction(&self) -> Option<bool> {
        self.0.use_transaction()
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
    state: State,
}

impl StepRunner {
    pub async fn connect(url: &str, migrations: Vec<Box<dyn MigrationTrait>>) -> Result<Self> {
        let migrations: Vec<_> = migrations
            .into_iter()
            .map(|step| Step(step.into()))
            .collect();
        let names: HashSet<_> = migrations.iter().map(|step| step.name()).collect();
        if migrations.is_empty()
            || names.len() != migrations.len()
            || names.iter().any(|name| name.trim().is_empty())
        {
            return Err(ExecutionError::InvalidPlan);
        }
        for step in &migrations {
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
            migrations[0].install(&database).await?;
            let applied = read_journal(&database).await?;
            verify_prefix(&migrations, &applied, applied.len())?;
            Ok(applied.len())
        }
        .await;
        match initial {
            Ok(next) => Ok(Self {
                database,
                migrations,
                state: State::Ready(next),
            }),
            Err(error) => {
                database.close().await?;
                Err(error)
            }
        }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.database
    }

    pub fn pending_migrations(&self) -> Result<impl Iterator<Item = &str>> {
        let State::Ready(next) = self.state else {
            return Err(ExecutionError::Interrupted);
        };
        Ok(self.migrations[next..].iter().map(|step| step.name()))
    }

    /// Returns only after commit and exact journal-prefix verification. A failed
    /// or cancelled call poisons this runner. No next step may follow it.
    pub async fn apply_next(&mut self) -> Result<Option<String>> {
        let State::Ready(next) = self.state else {
            return Err(ExecutionError::Interrupted);
        };
        self.state = State::Interrupted;
        verify_prefix(&self.migrations, &read_journal(&self.database).await?, next)?;
        let Some(step) = self.migrations.get(next) else {
            self.state = State::Ready(next);
            return Ok(None);
        };
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
