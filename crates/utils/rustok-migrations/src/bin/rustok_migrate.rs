use std::{
    env,
    error::Error,
    fs::File,
    io::{self, Read},
    path::PathBuf,
};

use rustok_migrations::{
    Migrator,
    execution::{ExecutionPhase, StepRunner},
    verification::APPLIED_STEP_PREFIX,
};
use sea_orm_migration::{prelude::MigratorTrait, sea_orm::Database};

const PHASE_LIMIT: u64 = 1024 * 1024;
const USAGE: &str = "usage: rustok-migrate <up [--phase <json-file>]|status>";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Up { phase: Option<PathBuf> },
    Status,
}

fn invalid_input(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    io::Error::new(io::ErrorKind::InvalidInput, message.into()).into()
}

fn parse_command(arguments: &[String]) -> Result<Command, Box<dyn Error + Send + Sync>> {
    match arguments {
        [command] if command == "up" => Ok(Command::Up { phase: None }),
        [command, flag, path]
            if command == "up" && flag == "--phase" && !path.trim().is_empty() =>
        {
            Ok(Command::Up {
                phase: Some(path.into()),
            })
        }
        [command] if command == "status" => Ok(Command::Status),
        [] => Err(invalid_input(format!("missing command; {USAGE}"))),
        [command] => Err(invalid_input(format!(
            "unsupported command {command:?}; only up and status are allowed",
        ))),
        _ => Err(invalid_input(format!("invalid arguments; {USAGE}"))),
    }
}

fn read_phase(reader: impl Read) -> Result<ExecutionPhase, Box<dyn Error + Send + Sync>> {
    let mut bytes = Vec::new();
    reader.take(PHASE_LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > PHASE_LIMIT {
        return Err(invalid_input("phase request exceeds 1 MiB"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

async fn run() -> Result<(), Box<dyn Error + Send + Sync>> {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let command = parse_command(&arguments)?;
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| invalid_input("DATABASE_URL must be set for rustok-migrate"))?;
    if database_url.trim().is_empty() {
        return Err(invalid_input("DATABASE_URL must not be empty"));
    }
    let inventory = Migrator::inventory()?;

    match command {
        Command::Up { phase } => {
            let bounded = phase.is_some();
            let mut runner = match phase {
                Some(path) => {
                    StepRunner::connect_phase(
                        &database_url,
                        inventory.into_migrations(),
                        read_phase(File::open(path)?)?,
                    )
                    .await?
                }
                None => StepRunner::connect(&database_url, inventory.into_migrations()).await?,
            };
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
            if bounded {
                println!("requested migration phase is complete");
            } else {
                println!("all migrations are applied");
            }
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
    use super::{Command, PHASE_LIMIT, parse_command, read_phase};

    #[test]
    fn accepts_only_non_destructive_commands() {
        assert_eq!(
            parse_command(&["up".to_string()]).unwrap(),
            Command::Up { phase: None }
        );
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
    fn phase_requires_an_explicit_file_on_up() {
        let args = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).into())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            parse_command(&args(&["up", "--phase", "phase.json"])).unwrap(),
            Command::Up {
                phase: Some("phase.json".into())
            }
        );
        for values in [
            vec!["up", "--phase"],
            vec!["up", "--phase", " "],
            vec!["status", "--phase", "phase.json"],
            vec!["up", "--through", "tail"],
            vec!["up", "--phase", "phase.json", "extra"],
        ] {
            assert!(parse_command(&args(&values)).is_err());
        }
    }

    #[test]
    fn phase_file_is_strict_and_bounded() {
        let phase =
            read_phase(br#"{"expected_prefix":["create"],"steps":["backfill"]}"#.as_slice())
                .unwrap();
        assert_eq!(phase.expected_prefix, ["create"]);
        assert_eq!(phase.steps, ["backfill"]);
        for json in [
            "{}",
            r#"{"steps":["backfill"]}"#,
            r#"{"expected_prefix":[],"steps":["create"],"approved":true}"#,
            r#"{"expected_prefix":[],"steps":[],"steps":["create"]}"#,
            r#"{"expected_prefix":[],"steps":["create"]} trailing"#,
        ] {
            assert!(read_phase(json.as_bytes()).is_err());
        }
        // This is valid JSON with leading whitespace: without the input bound
        // it would deserialize successfully rather than fail for another reason.
        let mut oversized = vec![b' '; PHASE_LIMIT as usize];
        oversized.extend_from_slice(br#"{"expected_prefix":[],"steps":["create"]}"#);
        assert!(
            read_phase(oversized.as_slice())
                .unwrap_err()
                .to_string()
                .contains("exceeds 1 MiB")
        );
    }
}
