use std::{env, error::Error, io};

use rustok_migrations::{Migrator, execution::StepRunner, verification::APPLIED_STEP_PREFIX};
use sea_orm_migration::{prelude::MigratorTrait, sea_orm::Database};

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

async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let command = parse_command(&arguments)?;
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| invalid_input("DATABASE_URL must be set for rustok-migrate"))?;
    if database_url.trim().is_empty() {
        return Err(invalid_input("DATABASE_URL must not be empty"));
    }

    match command {
        Command::Up => {
            let mut runner = StepRunner::connect(&database_url, Migrator::migrations()).await?;
            let result = async {
                while let Some(name) = runner.apply_next().await? {
                    println!("{APPLIED_STEP_PREFIX}{name}");
                }
                Ok::<_, rustok_migrations::execution::ExecutionError>(())
            }
            .await;
            let close = runner.close().await;
            match (result, close) {
                (Err(error), Err(cleanup)) => {
                    return Err(
                        io::Error::other(format!("{error}; session cleanup: {cleanup}")).into(),
                    );
                }
                (Err(error), _) | (_, Err(error)) => return Err(error.into()),
                (Ok(()), Ok(())) => {}
            }
            println!("all migrations are applied");
        }
        Command::Status => {
            let database = Database::connect(database_url).await?;
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
    use super::{Command, parse_command};

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
}
