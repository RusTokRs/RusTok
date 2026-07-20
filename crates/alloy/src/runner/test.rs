use std::sync::Arc;

use crate::{
    ExecutionContext, ExecutionPhase, Script, ScriptRegistry, ScriptResult, ScriptTrigger,
    TestCommand, TestRun, TestRunClaim, TestRunCompletion,
};

use super::super::AlloyDraftRuntime;

/// Executes durable Alloy test commands. It claims a source-revision lease
/// before sandbox work and records a terminal result afterward, so duplicate
/// deliveries replay evidence instead of running a different workspace.
pub struct RevisionedTestRunner<R: ScriptRegistry> {
    runtime: AlloyDraftRuntime,
    registry: Arc<R>,
}

impl<R: ScriptRegistry> RevisionedTestRunner<R> {
    pub fn new(runtime: AlloyDraftRuntime, registry: Arc<R>) -> Self {
        Self { runtime, registry }
    }

    pub async fn execute(&self, command: TestCommand) -> ScriptResult<TestRun> {
        match self.registry.claim_test_run(command).await? {
            TestRunClaim::Replay(run) | TestRunClaim::InProgress(run) => Ok(run),
            TestRunClaim::Claimed(lease) => {
                let mut script = Script::new(
                    format!("test-run:{}", lease.source.script_id),
                    lease.source.workspace.clone(),
                    ScriptTrigger::Manual,
                );
                script.id = lease.source.script_id;
                script.tenant_id = lease.source.tenant_id;
                script.version = lease.source.revision;
                let context = ExecutionContext::new(ExecutionPhase::Manual)
                    .with_tenant(lease.source.tenant_id.to_string())
                    .with_user(lease.run.actor_id.clone());
                let completion = match self
                    .runtime
                    .execute_test(&script, &lease.run.test_path, &context)
                    .await
                {
                    Ok(true) => TestRunCompletion::passed(),
                    Ok(false) => TestRunCompletion::failed(Some(
                        "test entrypoint returned false".to_string(),
                    ))?,
                    Err(error) => {
                        TestRunCompletion::failed(Some(bounded_error(error.to_string())))?
                    }
                };
                self.registry
                    .complete_test_run(lease.run.id, lease.lease_token, completion)
                    .await
            }
        }
    }
}

fn bounded_error(error: String) -> String {
    let normalized = error
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let bounded = normalized
        .chars()
        .take(crate::MAX_TEST_ERROR_LENGTH)
        .collect::<String>();
    if bounded.trim().is_empty() {
        "sandbox test failed without an error message".to_string()
    } else {
        bounded.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::bounded_error;
    use crate::TEST_RUN_LEASE_SECONDS;

    #[test]
    fn terminal_test_errors_are_bounded_and_control_free() {
        assert_eq!(bounded_error("failed\nnow".into()), "failed now");
        assert_eq!(
            bounded_error("\n".into()),
            "sandbox test failed without an error message"
        );
        assert!(TEST_RUN_LEASE_SECONDS > 0);
    }
}
