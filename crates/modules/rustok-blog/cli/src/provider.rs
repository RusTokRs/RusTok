use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::RuntimeComposition;

use crate::commands::{cleanup_tokens, publish_scheduled};

pub struct BlogCommandProvider {
    runtime: RuntimeComposition,
}

#[async_trait::async_trait]
impl CommandProvider for BlogCommandProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![
            CommandDescriptor::new(
                "blog",
                "publish-scheduled",
                "Publish blog posts whose scheduled date has arrived",
            )
            .with_dry_run(),
            CommandDescriptor::new(
                "blog",
                "cleanup-tokens",
                "Remove expired blog preview tokens",
            )
            .with_dry_run(),
        ]
    }

    async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        match (request.namespace.as_str(), request.name.as_str()) {
            ("blog", "publish-scheduled") => {
                publish_scheduled::execute(&self.runtime, request).await
            }
            ("blog", "cleanup-tokens") => cleanup_tokens::execute(&self.runtime, request).await,
            _ => Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            }),
        }
    }
}

pub fn command_provider(runtime: &RuntimeComposition) -> Box<dyn CommandProvider> {
    Box::new(BlogCommandProvider {
        runtime: runtime.clone(),
    })
}
