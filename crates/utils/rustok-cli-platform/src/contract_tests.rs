use rustok_cli_core::{CliCoreError, CommandProvider, CommandRequest};
use rustok_runtime::RuntimeComposition;

use crate::{PlatformCommandProvider, command_provider};

#[test]
fn test_platform_command_provider_descriptors() {
    let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
    let provider = PlatformCommandProvider::new(runtime.clone());
    assert_eq!(provider.runtime().settings(), runtime.settings());

    let debug_repr = format!("{provider:?}");
    assert!(debug_repr.contains("PlatformCommandProvider"));

    let cloned = provider.clone();
    let commands = cloned.commands();
    assert_eq!(commands.len(), 5);

    let version_cmd = commands.iter().find(|c| c.name == "version").unwrap();
    assert_eq!(version_cmd.namespace, "core");
    assert!(!version_cmd.supports_dry_run);

    let db_baseline_cmd = commands.iter().find(|c| c.name == "db-baseline").unwrap();
    assert_eq!(db_baseline_cmd.namespace, "core");
    assert!(!db_baseline_cmd.supports_dry_run);

    let rebuild_cmd = commands.iter().find(|c| c.name == "rebuild").unwrap();
    assert_eq!(rebuild_cmd.namespace, "core");
    assert!(rebuild_cmd.supports_dry_run);

    let migrate_up_cmd = commands
        .iter()
        .find(|c| c.namespace == "migrate" && c.name == "up")
        .unwrap();
    assert!(!migrate_up_cmd.supports_dry_run);

    let migrate_status_cmd = commands
        .iter()
        .find(|c| c.namespace == "migrate" && c.name == "status")
        .unwrap();
    assert!(!migrate_status_cmd.supports_dry_run);
}

#[tokio::test]
async fn test_version_command_outcome() {
    let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
    let provider = command_provider(&runtime);

    let req = CommandRequest::new("core", "version", serde_json::Value::Null, false);
    let outcome = provider
        .execute(req)
        .await
        .expect("version command succeeds");

    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.data["package"], "rustok-cli-platform");
    assert_eq!(outcome.data["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn test_unknown_command_returns_typed_error() {
    let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
    let provider = command_provider(&runtime);

    let req = CommandRequest::new("core", "nonexistent", serde_json::Value::Null, false);
    let err = provider.execute(req).await.unwrap_err();

    assert_eq!(
        err,
        CliCoreError::UnknownCommand {
            namespace: "core".to_string(),
            name: "nonexistent".to_string(),
        }
    );
}

#[tokio::test]
async fn test_db_dependent_commands_fail_gracefully_without_host() {
    let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
    let provider = command_provider(&runtime);

    // migrate up requires DB
    let req = CommandRequest::new("migrate", "up", serde_json::Value::Null, false);
    let err = provider.execute(req).await.unwrap_err();
    assert!(matches!(err, CliCoreError::CommandFailed { .. }));

    // migrate status requires DB
    let req = CommandRequest::new("migrate", "status", serde_json::Value::Null, false);
    let err = provider.execute(req).await.unwrap_err();
    assert!(matches!(err, CliCoreError::CommandFailed { .. }));

    // db-baseline requires DB
    let req = CommandRequest::new(
        "core",
        "db-baseline",
        serde_json::json!({ "options": {} }),
        false,
    );
    let err = provider.execute(req).await.unwrap_err();
    assert!(matches!(err, CliCoreError::CommandFailed { .. }));

    // rebuild requires DB
    let req = CommandRequest::new(
        "core",
        "rebuild",
        serde_json::json!({ "options": {} }),
        false,
    );
    let err = provider.execute(req).await.unwrap_err();
    assert!(matches!(err, CliCoreError::CommandFailed { .. }));
}
