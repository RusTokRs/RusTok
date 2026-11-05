#[cfg(test)]
mod tests {
    use rustok_cli_core::{CliCoreError, CommandRequest};
    use rustok_runtime::RuntimeComposition;

    use crate::{InstallerCommandProvider, command_provider};

    #[test]
    fn test_installer_command_provider_construction_and_debug() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = InstallerCommandProvider::new(runtime);
        let formatted = format!("{provider:?}");
        assert!(formatted.contains("InstallerCommandProvider"));

        assert!(provider.runtime().settings().is_null());
    }

    #[test]
    fn test_installer_command_descriptors() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let commands = provider.commands();

        assert_eq!(commands.len(), 6);

        let seed_apply = commands
            .iter()
            .find(|c| c.namespace == "seed" && c.name == "apply")
            .expect("seed apply command registered");
        assert!(seed_apply.supports_dry_run);

        let starter_import = commands
            .iter()
            .find(|c| c.namespace == "starter" && c.name == "import")
            .expect("starter import command registered");
        assert!(starter_import.supports_dry_run);

        let install_plan = commands
            .iter()
            .find(|c| c.namespace == "install" && c.name == "plan")
            .expect("install plan command registered");
        assert!(!install_plan.supports_dry_run);

        let install_preflight = commands
            .iter()
            .find(|c| c.namespace == "install" && c.name == "preflight")
            .expect("install preflight command registered");
        assert!(!install_preflight.supports_dry_run);

        let install_apply = commands
            .iter()
            .find(|c| c.namespace == "install" && c.name == "apply")
            .expect("install apply command registered");
        assert!(install_apply.supports_dry_run);

        let install_status = commands
            .iter()
            .find(|c| c.namespace == "install" && c.name == "status")
            .expect("install status command registered");
        assert!(!install_status.supports_dry_run);
    }

    #[tokio::test]
    async fn test_unknown_command_returns_typed_error() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);

        let result = provider
            .execute(CommandRequest {
                namespace: "installer".to_string(),
                name: "unknown".to_string(),
                args: serde_json::json!({}),
                dry_run: false,
            })
            .await;

        match result {
            Err(CliCoreError::UnknownCommand { namespace, name }) => {
                assert_eq!(namespace, "installer");
                assert_eq!(name, "unknown");
            }
            _ => panic!("expected CliCoreError::UnknownCommand"),
        }
    }

    #[tokio::test]
    async fn test_starter_import_rejects_unknown_starter() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);

        let result = provider
            .execute(CommandRequest {
                namespace: "starter".to_string(),
                name: "import".to_string(),
                args: serde_json::json!({
                    "options": {
                        "starter": "nonexistent_starter_pack"
                    }
                }),
                dry_run: true,
            })
            .await;

        match result {
            Err(CliCoreError::InvalidInput { message }) => {
                assert!(message.contains("Unknown starter blueprint"));
            }
            _ => panic!("expected CliCoreError::InvalidInput for unknown starter"),
        }
    }

    #[tokio::test]
    async fn test_starter_import_rejects_missing_file() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);

        let result = provider
            .execute(CommandRequest {
                namespace: "starter".to_string(),
                name: "import".to_string(),
                args: serde_json::json!({
                    "options": {
                        "file": "nonexistent_blueprint_file_12345.json"
                    }
                }),
                dry_run: true,
            })
            .await;

        match result {
            Err(CliCoreError::CommandFailed { message }) => {
                assert!(message.contains("Failed to read blueprint file"));
            }
            _ => panic!("expected CliCoreError::CommandFailed for missing file"),
        }
    }
}
