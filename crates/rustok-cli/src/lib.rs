use std::collections::BTreeSet;

use rustok_cli_core::{
    CliCoreError, CliCoreResult, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};
use rustok_runtime::RuntimeComposition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliExit {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CliExit {
    fn success(stdout: impl Into<String>) -> Self {
        Self {
            code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    fn failure(stderr: impl Into<String>) -> Self {
        Self {
            code: 2,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    fn from_outcome(outcome: CommandOutcome) -> Self {
        let mut stdout = outcome.message;
        if !stdout.is_empty() && !stdout.ends_with('\n') {
            stdout.push('\n');
        }
        if !outcome.data.is_null() {
            match serde_json::to_string_pretty(&outcome.data) {
                Ok(data) => {
                    stdout.push_str(&data);
                    stdout.push('\n');
                }
                Err(error) => {
                    return Self::failure(format!(
                        "Failed to render command outcome data: {error}"
                    ));
                }
            }
        }

        Self {
            code: outcome.exit_code,
            stdout,
            stderr: String::new(),
        }
    }
}

pub struct BuiltInProvider;

#[async_trait::async_trait]
impl CommandProvider for BuiltInProvider {
    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor::new(
            "core",
            "list",
            "List commands available in the selected CLI distribution",
        )]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRegistryError {
    DuplicateCommand { namespace: String, name: String },
}

impl std::fmt::Display for CommandRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateCommand { namespace, name } => {
                write!(
                    formatter,
                    "duplicate command registered: {namespace} {name}"
                )
            }
        }
    }
}

impl std::error::Error for CommandRegistryError {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ListOptions {
    json: bool,
    namespace: Option<String>,
}

impl ListOptions {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        let mut index = 0;

        while index < args.len() {
            let arg = &args[index];
            if arg == "--json" {
                options.json = true;
                index += 1;
                continue;
            }
            if arg == "--namespace" {
                let Some(namespace) = args.get(index + 1) else {
                    return Err("Missing value for --namespace".to_string());
                };
                options.namespace = Some(namespace.clone());
                index += 2;
                continue;
            }
            if let Some(namespace) = arg.strip_prefix("--namespace=") {
                if namespace.is_empty() {
                    return Err("Missing value for --namespace".to_string());
                }
                options.namespace = Some(namespace.to_string());
                index += 1;
                continue;
            }

            return Err(format!("Unknown list option: {arg}"));
        }

        Ok(options)
    }
}

pub struct CommandRegistry<'a> {
    commands: Vec<CommandDescriptor>,
    providers: Vec<&'a dyn CommandProvider>,
}

impl std::fmt::Debug for CommandRegistry<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CommandRegistry")
            .field("commands", &self.commands)
            .field("providers_len", &self.providers.len())
            .finish()
    }
}

impl<'a> CommandRegistry<'a> {
    pub fn from_providers(
        providers: &[&'a dyn CommandProvider],
    ) -> Result<Self, CommandRegistryError> {
        let commands = collect_commands(providers);
        let mut seen = BTreeSet::new();
        for command in &commands {
            if !seen.insert((command.namespace.clone(), command.name.clone())) {
                return Err(CommandRegistryError::DuplicateCommand {
                    namespace: command.namespace.clone(),
                    name: command.name.clone(),
                });
            }
        }

        Ok(Self {
            commands,
            providers: providers.to_vec(),
        })
    }

    pub fn commands(&self) -> &[CommandDescriptor] {
        &self.commands
    }

    pub async fn execute(&self, request: CommandRequest) -> CliCoreResult<CommandOutcome> {
        if !self
            .commands
            .iter()
            .any(|command| command.namespace == request.namespace && command.name == request.name)
        {
            return Err(CliCoreError::UnknownCommand {
                namespace: request.namespace,
                name: request.name,
            });
        }

        for provider in &self.providers {
            if provider.commands().iter().any(|command| {
                command.namespace == request.namespace && command.name == request.name
            }) {
                return provider.execute(request).await;
            }
        }

        Err(CliCoreError::UnknownCommand {
            namespace: request.namespace,
            name: request.name,
        })
    }
}

fn filter_commands(
    commands: &[CommandDescriptor],
    namespace: Option<&str>,
) -> Vec<CommandDescriptor> {
    commands
        .iter()
        .filter(|command| namespace.is_none_or(|namespace| command.namespace == namespace))
        .cloned()
        .collect()
}

pub fn collect_commands(providers: &[&dyn CommandProvider]) -> Vec<CommandDescriptor> {
    let mut commands = providers
        .iter()
        .flat_map(|provider| provider.commands())
        .collect::<Vec<_>>();
    commands.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then(left.name.cmp(&right.name))
    });
    commands
}

pub fn render_command_list(commands: &[CommandDescriptor]) -> String {
    if commands.is_empty() {
        return "No commands are registered for this CLI distribution.\n".to_string();
    }

    let mut lines = Vec::with_capacity(commands.len() + 1);
    lines.push("Available commands:".to_string());
    for command in commands {
        let dry_run = if command.supports_dry_run {
            " [dry-run]"
        } else {
            ""
        };
        lines.push(format!(
            "  {} {}{} - {}",
            command.namespace, command.name, dry_run, command.summary
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn render_command_list_json(
    commands: &[CommandDescriptor],
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(commands).map(|mut output| {
        output.push('\n');
        output
    })
}

pub fn parse_command_args(args: &[String]) -> Result<(serde_json::Value, bool), String> {
    let mut options = serde_json::Map::new();
    let mut positionals = Vec::new();
    let mut dry_run = false;
    let mut index = 0;
    let mut option_mode = true;

    while index < args.len() {
        let arg = &args[index];
        if option_mode && arg == "--" {
            option_mode = false;
            index += 1;
            continue;
        }

        if option_mode && arg.starts_with("--") {
            let raw = arg.trim_start_matches("--");
            if raw.is_empty() {
                return Err("Empty command option is not allowed".to_string());
            }

            let (key, value, consumed) = if raw == "dry-run" {
                (raw.to_string(), serde_json::Value::Bool(true), 1)
            } else if let Some((key, value)) = raw.split_once('=') {
                if key.is_empty() {
                    return Err("Empty command option is not allowed".to_string());
                }
                (
                    key.to_string(),
                    serde_json::Value::String(value.to_string()),
                    1,
                )
            } else if args
                .get(index + 1)
                .map(|next| !next.starts_with("--"))
                .unwrap_or(false)
            {
                (
                    raw.to_string(),
                    serde_json::Value::String(args[index + 1].clone()),
                    2,
                )
            } else {
                (raw.to_string(), serde_json::Value::Bool(true), 1)
            };

            let key = key.replace('-', "_");
            if key == "dry_run" {
                dry_run = value.as_bool().unwrap_or(true);
            }
            options.insert(key, value);
            index += consumed;
            continue;
        }

        positionals.push(serde_json::Value::String(arg.clone()));
        index += 1;
    }

    Ok((
        serde_json::json!({
            "options": options,
            "positionals": positionals,
        }),
        dry_run,
    ))
}

pub fn usage() -> &'static str {
    "Usage:\n  rustok-cli list [--json] [--namespace <name>]\n  rustok-cli <namespace> <command> [args...]\n  rustok-cli help\n\nCommands are provided by the selected distribution registry.\n"
}

pub async fn run_with_args<I, S>(args: I) -> CliExit
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    run_with_runtime(
        args,
        RuntimeComposition::without_database(serde_json::Value::Null),
    )
    .await
}

pub async fn run_with_environment<I, S>(args: I) -> CliExit
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    match RuntimeComposition::from_environment().await {
        Ok(runtime) => run_with_runtime(args, runtime).await,
        Err(error) => CliExit::failure(format!("Failed to initialize CLI runtime: {error}")),
    }
}

pub async fn run_with_runtime<I, S>(args: I, runtime: RuntimeComposition) -> CliExit
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let command = args.get(1).map(String::as_str).unwrap_or("help");
    let provider = BuiltInProvider;
    let distribution = rustok_cli_registry::selected_distribution_registry(&runtime);
    let mut providers = vec![&provider as &dyn CommandProvider];
    providers.extend(distribution.providers());
    let registry = match CommandRegistry::from_providers(&providers) {
        Ok(registry) => registry,
        Err(error) => return CliExit::failure(error.to_string()),
    };

    match command {
        "list" => render_list_command(&registry, &args[2..]),
        "core" if args.get(2).map(String::as_str) == Some("list") => {
            render_list_command(&registry, &args[3..])
        }
        "help" | "--help" | "-h" => CliExit::success(usage()),
        namespace if args.get(2).is_some() => {
            let command_name = args[2].clone();
            let (command_args, dry_run) = match parse_command_args(&args[3..]) {
                Ok(parsed) => parsed,
                Err(error) => return CliExit::failure(format!("{error}\n\n{}", usage())),
            };
            let request = CommandRequest {
                namespace: namespace.to_string(),
                name: command_name,
                args: command_args,
                dry_run,
            };
            match registry.execute(request).await {
                Ok(outcome) => CliExit::from_outcome(outcome),
                Err(error) => CliExit::failure(format!("{error}\n\n{}", usage())),
            }
        }
        unknown => CliExit::failure(format!(
            "Unknown rustok-cli command: {unknown}\n\n{}",
            usage()
        )),
    }
}

fn render_list_command(registry: &CommandRegistry<'_>, args: &[String]) -> CliExit {
    let options = match ListOptions::parse(args) {
        Ok(options) => options,
        Err(error) => return CliExit::failure(format!("{error}\n\n{}", usage())),
    };
    let commands = filter_commands(registry.commands(), options.namespace.as_deref());

    if options.json {
        return match render_command_list_json(&commands) {
            Ok(output) => CliExit::success(output),
            Err(error) => CliExit::failure(format!("Failed to render command list JSON: {error}")),
        };
    }

    CliExit::success(render_command_list(&commands))
}

#[cfg(test)]
mod tests {
    use super::{
        BuiltInProvider, CommandRegistry, collect_commands, render_command_list,
        render_command_list_json, run_with_args,
    };
    use rustok_cli_core::{CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest};

    #[test]
    fn built_in_provider_exposes_list_command() {
        let provider = BuiltInProvider;
        let commands = provider.commands();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].namespace, "core");
        assert_eq!(commands[0].name, "list");
    }

    #[test]
    fn command_args_parse_options_flags_and_positionals() {
        let (args, dry_run) = super::parse_command_args(&[
            "--tenant-id".to_string(),
            "tenant-1".to_string(),
            "--top-n=15".to_string(),
            "--dry-run".to_string(),
            "extra".to_string(),
            "--".to_string(),
            "--literal".to_string(),
        ])
        .unwrap();

        assert!(dry_run);
        assert_eq!(args["options"]["tenant_id"], "tenant-1");
        assert_eq!(args["options"]["top_n"], "15");
        assert_eq!(args["options"]["dry_run"], true);
        assert_eq!(args["positionals"][0], "extra");
        assert_eq!(args["positionals"][1], "--literal");
    }

    #[test]
    fn command_list_is_stably_sorted() {
        let provider = BuiltInProvider;
        let commands = collect_commands(&[&provider]);

        assert_eq!(commands[0].namespace, "core");
        assert_eq!(commands[0].name, "list");
    }

    #[tokio::test]
    async fn list_command_renders_available_commands() {
        let exit = run_with_args(["rustok-cli", "list"]).await;

        assert_eq!(exit.code, 0);
        assert!(exit.stderr.is_empty());
        assert!(exit.stdout.contains("core list"));
    }

    #[tokio::test]
    async fn list_command_can_render_json_inventory() {
        let exit = run_with_args(["rustok-cli", "list", "--json"]).await;

        assert_eq!(exit.code, 0);
        assert!(exit.stderr.is_empty());

        let commands: Vec<CommandDescriptor> = serde_json::from_str(&exit.stdout).unwrap();
        assert!(
            commands
                .iter()
                .any(|command| command.namespace == "core" && command.name == "list")
        );
    }

    #[tokio::test]
    async fn list_command_can_filter_by_namespace() {
        let exit = run_with_args(["rustok-cli", "list", "--namespace", "missing"]).await;

        assert_eq!(exit.code, 0);
        assert!(exit.stderr.is_empty());
        assert_eq!(
            exit.stdout,
            "No commands are registered for this CLI distribution.\n"
        );
    }

    #[tokio::test]
    async fn list_command_can_filter_json_by_namespace() {
        let exit = run_with_args(["rustok-cli", "list", "--namespace=core", "--json"]).await;

        assert_eq!(exit.code, 0);
        assert!(exit.stderr.is_empty());

        let commands: Vec<CommandDescriptor> = serde_json::from_str(&exit.stdout).unwrap();
        assert!(commands.iter().all(|command| command.namespace == "core"));
        assert!(commands.iter().any(|command| command.name == "list"));
    }

    #[tokio::test]
    async fn core_list_command_uses_namespace_dispatch_alias() {
        let exit = run_with_args(["rustok-cli", "core", "list", "--json"]).await;

        assert_eq!(exit.code, 0);
        assert!(exit.stderr.is_empty());

        let commands: Vec<CommandDescriptor> = serde_json::from_str(&exit.stdout).unwrap();
        assert_eq!(commands[0].namespace, "core");
        assert_eq!(commands[0].name, "list");
    }

    #[tokio::test]
    async fn core_version_command_uses_provider_execution() {
        let exit = run_with_args(["rustok-cli", "core", "version"]).await;

        assert_eq!(exit.code, 0);
        assert!(exit.stderr.is_empty());
        assert!(exit.stdout.contains(env!("CARGO_PKG_VERSION")));
        assert!(exit.stdout.contains("\"package\": \"rustok-cli-platform\""));
    }

    #[tokio::test]
    async fn list_command_rejects_unknown_options() {
        let exit = run_with_args(["rustok-cli", "list", "--unknown"]).await;

        assert_eq!(exit.code, 2);
        assert!(exit.stdout.is_empty());
        assert!(exit.stderr.contains("Unknown list option: --unknown"));
    }

    #[tokio::test]
    async fn unknown_command_fails_without_panicking() {
        let exit = run_with_args(["rustok-cli", "missing"]).await;

        assert_eq!(exit.code, 2);
        assert!(exit.stdout.is_empty());
        assert!(exit.stderr.contains("Unknown rustok-cli command"));
    }

    #[test]
    fn empty_command_list_has_explicit_output() {
        assert_eq!(
            render_command_list(&[]),
            "No commands are registered for this CLI distribution.\n"
        );
    }

    #[test]
    fn empty_command_list_json_is_explicit_array() {
        assert_eq!(render_command_list_json(&[]).unwrap(), "[]\n");
    }

    struct DuplicateProvider;

    #[async_trait::async_trait]
    impl CommandProvider for DuplicateProvider {
        fn commands(&self) -> Vec<CommandDescriptor> {
            vec![CommandDescriptor::new("core", "list", "duplicate")]
        }
    }

    #[test]
    fn registry_rejects_duplicate_provider_commands() {
        let built_in = BuiltInProvider;
        let duplicate = DuplicateProvider;
        let error = CommandRegistry::from_providers(&[&built_in, &duplicate]).unwrap_err();

        assert_eq!(error.to_string(), "duplicate command registered: core list");
    }

    struct ExecutingProvider;

    #[async_trait::async_trait]
    impl CommandProvider for ExecutingProvider {
        fn commands(&self) -> Vec<CommandDescriptor> {
            vec![CommandDescriptor::new("ops", "ping", "Ping ops command")]
        }

        async fn execute(
            &self,
            request: CommandRequest,
        ) -> rustok_cli_core::CliCoreResult<CommandOutcome> {
            assert_eq!(request.namespace, "ops");
            assert_eq!(request.name, "ping");
            Ok(CommandOutcome::success("pong"))
        }
    }

    #[tokio::test]
    async fn registry_dispatches_typed_command_execution() {
        let provider = ExecutingProvider;
        let registry = CommandRegistry::from_providers(&[&provider]).unwrap();
        let outcome = registry
            .execute(CommandRequest {
                namespace: "ops".to_string(),
                name: "ping".to_string(),
                args: serde_json::Value::Null,
                dry_run: false,
            })
            .await
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.message, "pong");
    }

    struct ArgsProvider;

    #[async_trait::async_trait]
    impl CommandProvider for ArgsProvider {
        fn commands(&self) -> Vec<CommandDescriptor> {
            vec![CommandDescriptor::new("ops", "args", "Inspect args")]
        }

        async fn execute(
            &self,
            request: CommandRequest,
        ) -> rustok_cli_core::CliCoreResult<CommandOutcome> {
            assert!(request.dry_run);
            assert_eq!(request.args["options"]["tenant_id"], "tenant-1");
            assert_eq!(request.args["positionals"][0], "positional");
            Ok(CommandOutcome::success("args ok"))
        }
    }

    #[tokio::test]
    async fn registry_dispatches_normalized_command_args() {
        let provider = ArgsProvider;
        let registry = CommandRegistry::from_providers(&[&provider]).unwrap();
        let (args, dry_run) = super::parse_command_args(&[
            "--tenant-id".to_string(),
            "tenant-1".to_string(),
            "--dry-run".to_string(),
            "positional".to_string(),
        ])
        .unwrap();
        let outcome = registry
            .execute(CommandRequest {
                namespace: "ops".to_string(),
                name: "args".to_string(),
                args,
                dry_run,
            })
            .await
            .unwrap();

        assert_eq!(outcome.message, "args ok");
    }
}
