use rustok_cli_core::{CliCoreError, CommandRequest};
use rustok_runtime::RuntimeComposition;

use crate::command_provider;

#[test]
fn provider_describes_blog_commands() {
    let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
    let provider = command_provider(&runtime);
    let commands = provider.commands();

    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].namespace, "blog");
    assert_eq!(commands[0].name, "publish-scheduled");
    assert!(commands[0].supports_dry_run);

    assert_eq!(commands[1].namespace, "blog");
    assert_eq!(commands[1].name, "cleanup-tokens");
    assert!(commands[1].supports_dry_run);
}

#[tokio::test]
async fn provider_dispatches_both_commands_and_preserves_typed_failures() {
    let runtime = RuntimeComposition::without_database(serde_json::Value::Null);
    let provider = command_provider(&runtime);

    for name in ["publish-scheduled", "cleanup-tokens"] {
        let request = CommandRequest {
            namespace: "blog".to_string(),
            name: name.to_string(),
            args: serde_json::json!({}),
            dry_run: true,
        };
        let error = provider.execute(request).await.unwrap_err();
        match error {
            CliCoreError::InvalidInput { message } => {
                assert_eq!(
                    message,
                    format!("blog {name} expects normalized command options")
                );
            }
            other => panic!("expected input error for {name}, got {other:?}"),
        }

        let request = CommandRequest {
            namespace: "blog".to_string(),
            name: name.to_string(),
            args: serde_json::json!({ "options": {} }),
            dry_run: true,
        };
        let error = provider.execute(request).await.unwrap_err();
        match error {
            CliCoreError::CommandFailed { message } => {
                assert!(message.starts_with("blog CLI requires a database runtime:"));
            }
            other => panic!("expected runtime error for {name}, got {other:?}"),
        }
    }
}
