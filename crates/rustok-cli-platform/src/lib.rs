use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::{db_clone, RuntimeComposition};

mod db_baseline;
mod rebuild;

pub struct PlatformCommandProvider {
    runtime: RuntimeComposition,
}

#[async_trait::async_trait]
impl CommandProvider for PlatformCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new("core", "version", "Print rustok-cli version metadata"),
            CommandDescriptor::new(
                "core",
                "db-baseline",
                "Collect pg_stat_statements and EXPLAIN plans for hot-path queries",
            ),
            CommandDescriptor::new(
                "core",
                "rebuild",
                "Execute a queued manifest-derived build plan",
            )
            .with_dry_run(),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("core", "version") => Ok(CommandOutcome::success(env!("CARGO_PKG_VERSION"))
                .with_data(serde_json::json!({
                    "package": env!("CARGO_PKG_NAME"),
                    "version": env!("CARGO_PKG_VERSION"),
                }))),
            ("core", "db-baseline") => {
                let db = db_clone(self.runtime.require_host().map_err(command_failed)?);
                db_baseline::execute(&db, &request.args).await
            }
            ("core", "rebuild") => {
                let db = db_clone(self.runtime.require_host().map_err(command_failed)?);
                rebuild::execute(&db, &request.args, request.dry_run).await
            }
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(PlatformCommandProvider {
        runtime: runtime.clone(),
    })
}

fn command_failed(error: impl std::fmt::Display) -> CliCoreError {
    CliCoreError::CommandFailed {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{command_provider, RuntimeComposition};
    use rustok_cli_core::{CommandProvider, CommandRequest};

    #[test]
    fn provider_describes_core_version_command() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let commands = provider.commands();

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].namespace, "core");
        assert_eq!(commands[0].name, "version");
    }

    #[tokio::test]
    async fn provider_executes_core_version_command() {
        let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
        let provider = command_provider(&runtime);
        let outcome = provider
            .execute(CommandRequest {
                namespace: "core".to_string(),
                name: "version".to_string(),
                args: serde_json::Value::Null,
                dry_run: false,
            })
            .await
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.message.contains(env!("CARGO_PKG_VERSION")));
        assert_eq!(outcome.data["package"], "rustok-cli-platform");
    }
}
