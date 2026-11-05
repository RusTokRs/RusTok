use crate::{CliCoreError, CommandDescriptor, CommandOutcome, CommandProvider, CommandRequest};

#[test]
fn test_command_descriptor_contract() {
    let desc = CommandDescriptor::new("test_ns", "run_cmd", "Execute tests");
    assert_eq!(desc.namespace, "test_ns");
    assert_eq!(desc.name, "run_cmd");
    assert_eq!(desc.summary, "Execute tests");
    assert!(!desc.supports_dry_run);
    assert!(desc.matches("test_ns", "run_cmd"));
    assert!(!desc.matches("other_ns", "run_cmd"));

    let with_dry = desc.with_dry_run();
    assert!(with_dry.supports_dry_run);

    let json = serde_json::to_string(&with_dry).expect("serialize");
    let deserialized: CommandDescriptor = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(with_dry, deserialized);
}

#[test]
fn test_command_request_contract() {
    let req = CommandRequest::new(
        "auth",
        "login",
        serde_json::json!({ "user": "admin" }),
        true,
    );
    assert_eq!(req.namespace, "auth");
    assert_eq!(req.name, "login");
    assert_eq!(req.args["user"], "admin");
    assert!(req.dry_run);

    let json = serde_json::to_string(&req).expect("serialize");
    let deserialized: CommandRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req, deserialized);
}

#[test]
fn test_command_outcome_contract() {
    // Success
    let succ = CommandOutcome::success("operation completed");
    assert_eq!(succ.exit_code, 0);
    assert_eq!(succ.message, "operation completed");
    assert!(succ.is_success());
    assert!(succ.data.is_null());

    let succ_with_data = succ.with_data(serde_json::json!({ "items": [1, 2, 3] }));
    assert_eq!(succ_with_data.data["items"][1], 2);

    // Failure
    let fail = CommandOutcome::failure(2, "syntax error");
    assert_eq!(fail.exit_code, 2);
    assert_eq!(fail.message, "syntax error");
    assert!(!fail.is_success());

    // Custom
    let custom = CommandOutcome::new(1, "partial", serde_json::json!({"code": 1}));
    assert_eq!(custom.exit_code, 1);
    assert!(!custom.is_success());

    let json = serde_json::to_string(&custom).expect("serialize");
    let deserialized: CommandOutcome = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(custom, deserialized);
}

#[test]
fn test_cli_core_error_contract() {
    let err1 = CliCoreError::UnknownCommand {
        namespace: "sys".to_string(),
        name: "reboot".to_string(),
    };
    assert_eq!(err1.to_string(), "unknown command: sys reboot");
    assert_eq!(err1.clone(), err1);

    let err2 = CliCoreError::InvalidInput {
        message: "missing arg".to_string(),
    };
    assert_eq!(err2.to_string(), "invalid command input: missing arg");

    let err3 = CliCoreError::CommandFailed {
        message: "io error".to_string(),
    };
    assert_eq!(err3.to_string(), "command failed: io error");

    let err4 = CliCoreError::DryRunNotSupported {
        namespace: "sys".to_string(),
        name: "reboot".to_string(),
    };
    assert_eq!(
        err4.to_string(),
        "command sys reboot does not support --dry-run"
    );
}

#[tokio::test]
async fn test_command_provider_default_implementation() {
    struct CustomProvider;

    #[async_trait::async_trait]
    impl CommandProvider for CustomProvider {
        fn commands(&self) -> Vec<CommandDescriptor> {
            vec![CommandDescriptor::new("ns", "cmd", "summary")]
        }
    }

    let p = CustomProvider;
    let cmds = p.commands();
    assert_eq!(cmds.len(), 1);

    let req = CommandRequest::new("ns", "cmd", serde_json::Value::Null, false);
    let err = p.execute(req).await.unwrap_err();
    assert_eq!(
        err,
        CliCoreError::UnknownCommand {
            namespace: "ns".to_string(),
            name: "cmd".to_string(),
        }
    );
}
