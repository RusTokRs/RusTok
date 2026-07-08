use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};

pub struct PlatformCommandProvider;

impl CommandProvider for PlatformCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor::new(
            "core",
            "version",
            "Print rustok-cli version metadata",
        )]
    }

    fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("core", "version") => Ok(CommandOutcome::success(env!("CARGO_PKG_VERSION"))
                .with_data(serde_json::json!({
                    "package": env!("CARGO_PKG_NAME"),
                    "version": env!("CARGO_PKG_VERSION"),
                }))),
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

pub fn command_provider() -> Box<dyn CommandProvider> {
    Box::new(PlatformCommandProvider)
}

#[cfg(test)]
mod tests {
    use super::{command_provider, PlatformCommandProvider};
    use rustok_cli_core::{CommandProvider, CommandRequest};

    #[test]
    fn provider_describes_core_version_command() {
        let provider = PlatformCommandProvider;
        let commands = provider.commands();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].namespace, "core");
        assert_eq!(commands[0].name, "version");
    }

    #[test]
    fn provider_executes_core_version_command() {
        let provider = command_provider();
        let outcome = provider
            .execute(CommandRequest {
                namespace: "core".to_string(),
                name: "version".to_string(),
                args: serde_json::Value::Null,
                dry_run: false,
            })
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.message.contains(env!("CARGO_PKG_VERSION")));
        assert_eq!(outcome.data["package"], "rustok-cli-platform");
    }
}
