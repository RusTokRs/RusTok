//! Queued build execution command.

use std::{path::PathBuf, sync::Arc};

use rustok_cli_core::{CliCoreError, CliCoreResult, CommandOutcome};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

pub(super) async fn execute(
    db: &DatabaseConnection,
    args: &serde_json::Value,
    dry_run: bool,
) -> CliCoreResult<CommandOutcome> {
    let executor = rustok_build::BuildExecutionService::new(
        db.clone(),
        Arc::new(rustok_build::NoopBuildEventPublisher),
        Arc::new(rustok_build::NoopReleaseActivationHook),
        workspace_root(),
    );
    let report = match build_id(args)? {
        Some(build_id) => executor.execute_build(build_id, dry_run).await.map(Some),
        None => executor.execute_next_queued_build(dry_run).await,
    }
    .map_err(command_failed)?;

    match report {
        Some(report) => Ok(CommandOutcome::success("Build execution completed")
            .with_data(serde_json::to_value(report).map_err(command_failed)?)),
        None => Ok(CommandOutcome::success("No queued builds available")),
    }
}

fn build_id(args: &serde_json::Value) -> CliCoreResult<Option<Uuid>> {
    let options = args
        .get("options")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| CliCoreError::InvalidInput {
            message: "core rebuild expects normalized command options".to_string(),
        })?;
    options
        .get("build_id")
        .and_then(serde_json::Value::as_str)
        .map(|raw| {
            Uuid::parse_str(raw).map_err(|error| CliCoreError::InvalidInput {
                message: format!("invalid build_id '{raw}': {error}"),
            })
        })
        .transpose()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .expect("workspace root should be resolvable from rustok-cli-platform")
}

fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::build_id;

    #[test]
    fn parses_optional_build_id() {
        assert!(
            build_id(&serde_json::json!({ "options": {} }))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            build_id(
                &serde_json::json!({ "options": { "build_id": uuid::Uuid::nil().to_string() } })
            )
            .unwrap(),
            Some(uuid::Uuid::nil())
        );
    }
}
