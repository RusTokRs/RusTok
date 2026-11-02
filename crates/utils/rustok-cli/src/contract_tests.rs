use rustok_cli_core::{
    CliCoreError, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest,
};

use crate::{
    collect_commands, parse_command_args, render_command_list, render_command_list_json,
    run_with_args, usage, BuiltInProvider, CliExit, CommandRegistry, CommandRegistryError,
};

#[test]
fn test_cli_exit_constructors() {
    let success = CliExit::success("all good\n");
    assert_eq!(success.code, 0);
    assert_eq!(success.stdout, "all good\n");
    assert!(success.stderr.is_empty());

    let failure = CliExit::failure("something broke\n");
    assert_eq!(failure.code, 2);
    assert!(failure.stdout.is_empty());
    assert_eq!(failure.stderr, "something broke\n");

    let custom = CliExit::new(42, "out", "err");
    assert_eq!(custom.code, 42);
    assert_eq!(custom.stdout, "out");
    assert_eq!(custom.stderr, "err");

    // from_outcome with data
    let outcome = CommandOutcome::success("done")
        .with_data(serde_json::json!({ "status": "ok" }));
    let exit = CliExit::from_outcome(outcome);
    assert_eq!(exit.code, 0);
    assert!(exit.stdout.contains("done"));
    assert!(exit.stdout.contains("\"status\": \"ok\""));
}

#[test]
fn test_built_in_provider_contract() {
    let provider = BuiltInProvider::default();
    let copy = provider;
    assert_eq!(provider, copy);
    let debug = format!("{provider:?}");
    assert!(debug.contains("BuiltInProvider"));

    let commands = provider.commands();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].namespace, "core");
    assert_eq!(commands[0].name, "list");
    assert!(!commands[0].supports_dry_run);
}

#[test]
fn test_command_registry_contract() {
    let provider = BuiltInProvider;
    let registry = CommandRegistry::from_providers(&[&provider]).expect("valid registry");

    assert_eq!(registry.providers_count(), 1);
    assert_eq!(registry.commands().len(), 1);

    let debug = format!("{registry:?}");
    assert!(debug.contains("CommandRegistry"));
    assert!(debug.contains("commands"));
    assert!(debug.contains("providers_len: 1"));
}

#[test]
fn test_duplicate_command_error_display() {
    struct DupProvider;

    #[async_trait::async_trait]
    impl CommandProvider for DupProvider {
        fn commands(&self) -> Vec<CommandDescriptor> {
            vec![CommandDescriptor::new("core", "list", "duplicate")]
        }
    }

    let p1 = BuiltInProvider;
    let p2 = DupProvider;
    let err = CommandRegistry::from_providers(&[&p1, &p2]).unwrap_err();

    assert_eq!(
        err,
        CommandRegistryError::DuplicateCommand {
            namespace: "core".to_string(),
            name: "list".to_string(),
        }
    );
    assert!(err.to_string().contains("duplicate command registered: core list"));
}

#[test]
fn test_parse_command_args_edge_cases() {
    // Empty options rejected
    assert!(parse_command_args(&["--".to_string()]).is_ok()); // terminator is fine
    assert_eq!(
        parse_command_args(&["--".to_string(), "--".to_string()]).unwrap().0["positionals"][0],
        "--"
    );

    // Empty option key rejected
    assert!(parse_command_args(&["--=invalid".to_string()]).is_err());

    // Flag conversion: dashes to underscores
    let (parsed, dry_run) = parse_command_args(&[
        "--auth-token-id".to_string(),
        "tok-123".to_string(),
        "--dry-run".to_string(),
        "first_pos".to_string(),
        "--".to_string(),
        "--literal-flag".to_string(),
    ])
    .expect("parsed options");

    assert!(dry_run);
    assert_eq!(parsed["options"]["auth_token_id"], "tok-123");
    assert_eq!(parsed["options"]["dry_run"], true);
    assert_eq!(parsed["positionals"][0], "first_pos");
    assert_eq!(parsed["positionals"][1], "--literal-flag");
}

#[test]
fn test_usage_and_help_text() {
    let text = usage();
    assert!(text.contains("Usage:"));
    assert!(text.contains("rustok-cli list"));
    assert!(text.contains("rustok-cli <namespace> <command>"));
}

#[test]
fn test_render_command_lists() {
    let commands = vec![
        CommandDescriptor::new("core", "list", "List commands"),
        CommandDescriptor::new("platform", "migrate", "Run migrations").with_dry_run(),
    ];

    let rendered = render_command_list(&commands);
    assert!(rendered.contains("core list - List commands"));
    assert!(rendered.contains("platform migrate [dry-run] - Run migrations"));

    let json_rendered = render_command_list_json(&commands).expect("render json");
    assert!(json_rendered.contains("\"namespace\": \"core\""));
    assert!(json_rendered.contains("\"supports_dry_run\": true"));
}

#[tokio::test]
async fn test_cli_dry_run_unsupported_rejection() {
    struct NoDryRunProvider;

    #[async_trait::async_trait]
    impl CommandProvider for NoDryRunProvider {
        fn commands(&self) -> Vec<CommandDescriptor> {
            vec![CommandDescriptor::new("test", "run", "Test run")]
        }
    }

    let provider = NoDryRunProvider;
    let registry = CommandRegistry::from_providers(&[&provider]).unwrap();

    let err = registry
        .execute(CommandRequest {
            namespace: "test".to_string(),
            name: "run".to_string(),
            args: serde_json::Value::Null,
            dry_run: true,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        CliCoreError::DryRunNotSupported { namespace, name }
            if namespace == "test" && name == "run"
    ));
}

#[tokio::test]
async fn test_run_with_args_help() {
    let exit = run_with_args(["rustok-cli", "help"]).await;
    assert_eq!(exit.code, 0);
    assert!(exit.stdout.contains("Usage:"));
}

#[test]
fn test_collect_commands_contract() {
    let provider = BuiltInProvider;
    let list = collect_commands(&[&provider]);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].namespace, "core");
    assert_eq!(list[0].name, "list");
}
