//! External operational command adapters for `rustok-media`.
//!
//! This crate maps platform CLI requests to owner-owned media services. It
//! deliberately contains no domain policy, terminal output, or exit handling.

use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::{db_clone, RuntimeComposition};
use rustok_storage::{StorageConfig, StorageService};

const CLEANUP_DEFAULT_LIMIT: u64 = 1_000;

pub struct MediaCommandProvider {
    runtime: RuntimeComposition,
}

impl MediaCommandProvider {
    fn storage_config(&self) -> CliCoreResult<StorageConfig> {
        self.runtime
            .settings()
            .get("storage")
            .cloned()
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| CliCoreError::InvalidInput {
                message: format!("invalid storage settings for media CLI: {error}"),
            })
            .map(|config| config.unwrap_or_default())
    }

    async fn cleanup(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        let limit = cleanup_limit(&request.args)?;
        let host = self
            .runtime
            .require_host()
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!("media cleanup requires a database runtime: {error}"),
            })?;
        let storage = StorageService::from_config(&self.storage_config()?)
            .await
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!("failed to initialize media storage: {error}"),
            })?;
        let service = rustok_media::MediaService::new(db_clone(host), storage);
        let report = service
            .cleanup_storage_orphans_all_tenants(limit)
            .await
            .map_err(|error| CliCoreError::CommandFailed {
                message: format!("media cleanup failed: {error}"),
            })?;

        Ok(
            CommandOutcome::success("Media cleanup complete").with_data(serde_json::json!({
                "inspected": report.inspected,
                "deleted_records": report.deleted_records,
                "kept_records": report.kept_records,
                "retry_later": report.retry_later,
            })),
        )
    }
}

#[async_trait::async_trait]
impl CommandProvider for MediaCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor::new(
            "media",
            "cleanup",
            "Remove media records whose storage objects are definitively absent",
        )]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("media", "cleanup") => self.cleanup(request).await,
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(MediaCommandProvider {
        runtime: runtime.clone(),
    })
}

fn cleanup_limit(args: &serde_json::Value) -> CliCoreResult<u64> {
    let Some(options) = args.get("options").and_then(serde_json::Value::as_object) else {
        return Err(CliCoreError::InvalidInput {
            message: "media cleanup expects normalized command options".to_string(),
        });
    };

    let Some(value) = options.get("limit") else {
        return Ok(CLEANUP_DEFAULT_LIMIT);
    };
    let Some(value) = value.as_str() else {
        return Err(CliCoreError::InvalidInput {
            message: "--limit must be a positive integer".to_string(),
        });
    };
    let limit = value
        .parse::<u64>()
        .map_err(|_| CliCoreError::InvalidInput {
            message: "--limit must be a positive integer".to_string(),
        })?;
    if limit == 0 {
        return Err(CliCoreError::InvalidInput {
            message: "--limit must be a positive integer".to_string(),
        });
    }
    Ok(limit)
}

#[cfg(test)]
mod tests {
    use super::{cleanup_limit, command_provider};
    use rustok_cli_core::CommandRequest;
    use rustok_runtime::RuntimeComposition;

    #[test]
    fn provider_describes_media_cleanup() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let commands = provider.commands();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].namespace, "media");
        assert_eq!(commands[0].name, "cleanup");
    }

    #[test]
    fn cleanup_limit_defaults_and_rejects_invalid_values() {
        assert_eq!(
            cleanup_limit(&serde_json::json!({ "options": {} })).unwrap(),
            1_000
        );
        assert_eq!(
            cleanup_limit(&serde_json::json!({ "options": { "limit": "25" } })).unwrap(),
            25
        );
        assert!(cleanup_limit(&serde_json::json!({ "options": { "limit": "0" } })).is_err());
    }

    #[tokio::test]
    async fn cleanup_requires_a_database_runtime() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let error = provider
            .execute(CommandRequest {
                namespace: "media".to_string(),
                name: "cleanup".to_string(),
                args: serde_json::json!({ "options": {} }),
                dry_run: false,
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("requires a database runtime"));
    }
}
