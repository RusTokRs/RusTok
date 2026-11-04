use std::{collections::HashSet, env, error::Error, io, sync::Arc};

use rustok_migrations::{Migrator, verification::APPLIED_STEP_PREFIX};
use sea_orm_migration::{
    migrator::{Migration, MigratorTraitSelf},
    prelude::*,
    sea_orm::{Database, DatabaseConnection, DbBackend, EntityTrait, Statement, TransactionTrait},
    seaql_migrations::Entity as Journal,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Up,
    Status,
}

fn invalid_input(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    io::Error::new(io::ErrorKind::InvalidInput, message.into()).into()
}

fn parse_command(arguments: &[String]) -> Result<Command, Box<dyn Error + Send + Sync>> {
    match arguments {
        [command] if command == "up" => Ok(Command::Up),
        [command] if command == "status" => Ok(Command::Status),
        [] => Err(invalid_input(
            "missing command; usage: rustok-migrate <up|status>",
        )),
        [command] => Err(invalid_input(format!(
            "unsupported command {command:?}; only up and status are allowed",
        ))),
        _ => Err(invalid_input(
            "too many arguments; usage: rustok-migrate <up|status>",
        )),
    }
}

#[derive(Clone)]
struct Step(Arc<dyn MigrationTrait>);

impl MigrationName for Step {
    fn name(&self) -> &str {
        self.0.name()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Step {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
    ) -> Result<Vec<Migration>, DbErr> {
        if Journal::find_by_id(self.name()).one(db).await?.is_some() {
            return Err(DbErr::Migration(
                "migration was applied outside this run".into(),
            ));
        }
        Ok(self.get_migration_files())
    }
}

fn journal_is_prefix(plan: &[&str], applied: &HashSet<String>) -> bool {
    applied.len() <= plan.len()
        && plan[..applied.len()]
            .iter()
            .all(|name| applied.contains(*name))
}

async fn read_journal(database: &DatabaseConnection) -> Result<HashSet<String>, DbErr> {
    Ok(Journal::find()
        .all(database)
        .await?
        .into_iter()
        .map(|row| row.version)
        .collect())
}

async fn apply_steps(database: &DatabaseConnection) -> Result<(), Box<dyn Error + Send + Sync>> {
    // The dedicated transaction serializes cooperating CLI writers without
    // changing the transaction policy of any migration on the worker connection.
    let lock = if database.get_database_backend() == DbBackend::Postgres {
        let tx = database.begin().await?;
        let row = tx
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT pg_try_advisory_xact_lock(hashtext('rustok-migrate'), 1) AS acquired",
            ))
            .await?
            .ok_or_else(|| io::Error::other("migration lock result is missing"))?;
        if !row.try_get::<bool>("", "acquired")? {
            return Err(
                io::Error::other("another rustok-migrate writer holds the database lock").into(),
            );
        }
        Some(tx)
    } else {
        None
    };

    let migrations = <Migrator as MigratorTrait>::migrations()
        .into_iter()
        .map(|migration| Step(migration.into()))
        .collect::<Vec<_>>();
    let plan = migrations
        .iter()
        .map(|step| step.name())
        .collect::<Vec<_>>();
    if plan.is_empty() || plan.iter().collect::<HashSet<_>>().len() != plan.len() {
        return Err(
            io::Error::other("executable migration plan is empty or contains duplicates").into(),
        );
    }
    <Migrator as MigratorTrait>::install(database).await?;
    let applied = read_journal(database).await?;
    if !journal_is_prefix(&plan, &applied) {
        return Err(
            io::Error::other("migration journal is not a prefix of the executable plan").into(),
        );
    }

    for step in &migrations[applied.len()..] {
        let name = step.name();
        if let Some(lock) = &lock {
            lock.query_one_raw(Statement::from_string(DbBackend::Postgres, "SELECT 1"))
                .await
                .map_err(|error| {
                    io::Error::other(format!("migration lock lost before {name}: {error}"))
                })?;
        }
        MigratorTraitSelf::up(step, database, Some(1))
            .await
            .map_err(|error| io::Error::other(format!("migration {name} failed: {error}")))?;
        let recorded = Journal::find_by_id(name)
            .one(database)
            .await
            .map_err(|error| {
                io::Error::other(format!("cannot verify journal entry for {name}: {error}"))
            })?;
        if recorded.is_none() {
            return Err(io::Error::other(format!("journal entry for {name} is missing")).into());
        }
        println!("{APPLIED_STEP_PREFIX}{name}");
    }

    let applied = read_journal(database).await?;
    if applied.len() != plan.len() || !journal_is_prefix(&plan, &applied) {
        return Err(
            io::Error::other("final migration journal does not match the executable plan").into(),
        );
    }

    if let Some(lock) = lock {
        lock.rollback().await?;
    }
    Ok(())
}

async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let command = parse_command(&arguments)?;
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| invalid_input("DATABASE_URL must be set for rustok-migrate"))?;
    if database_url.trim().is_empty() {
        return Err(invalid_input("DATABASE_URL must not be empty"));
    }

    let database = Database::connect(database_url).await?;
    match command {
        Command::Up => {
            apply_steps(&database).await?;
            println!("all migrations are applied");
        }
        Command::Status => {
            let pending =
                <Migrator as MigratorTrait>::get_pending_migrations_read_only(&database).await?;
            if pending.is_empty() {
                println!("all migrations are applied");
            } else {
                return Err(invalid_input(format!(
                    "{} migration(s) are pending",
                    pending.len(),
                )));
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("rustok migration command failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, journal_is_prefix, parse_command};

    #[test]
    fn accepts_only_non_destructive_commands() {
        assert_eq!(parse_command(&["up".to_string()]).unwrap(), Command::Up);
        assert_eq!(
            parse_command(&["status".to_string()]).unwrap(),
            Command::Status
        );
        for forbidden in ["down", "fresh", "reset", "refresh"] {
            assert!(parse_command(&[forbidden.to_string()]).is_err());
        }
        assert!(parse_command(&[]).is_err());
        assert!(parse_command(&["up".to_string(), "extra".to_string()]).is_err());
    }

    #[test]
    fn requires_a_contiguous_journal_prefix() {
        let plan = ["a", "b", "c"];
        assert!(journal_is_prefix(&plan, &Default::default()));
        assert!(journal_is_prefix(&plan, &["a".into(), "b".into()].into()));
        for invalid in [["a", "c"], ["b", "c"], ["a", "unknown"]] {
            assert!(!journal_is_prefix(&plan, &invalid.map(String::from).into()));
        }
    }
}
