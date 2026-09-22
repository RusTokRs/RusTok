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
                script.parent_release = lease.source.parent_release.clone();
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
        .take(crate::model::MAX_TEST_ERROR_LENGTH)
        .collect::<String>();
    if bounded.trim().is_empty() {
        "sandbox test failed without an error message".to_string()
    } else {
        bounded.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_sandbox::SandboxPolicy;
    use uuid::Uuid;

    use super::{RevisionedTestRunner, bounded_error};
    use crate::{
        AlloyImportedDraftPolicyError, AlloyImportedDraftPolicyProvider,
        AlloyImportedDraftPolicyProviderHandle, InMemoryStorage, RhaiWorkspace, RhaiWorkspaceFile,
        RhaiWorkspaceFileKind, Script, ScriptRegistry, ScriptTrigger, TestCommand, TestRunStatus,
        model::TEST_RUN_LEASE_SECONDS,
    };

    #[derive(Clone)]
    struct UnavailableImportedDraftPolicyProvider;

    #[async_trait]
    impl AlloyImportedDraftPolicyProvider for UnavailableImportedDraftPolicyProvider {
        async fn resolve_policy(
            &self,
            _tenant_id: Uuid,
            _parent_release: &rustok_modules::ArtifactReleaseRef,
        ) -> Result<SandboxPolicy, AlloyImportedDraftPolicyError> {
            Err(AlloyImportedDraftPolicyError::Unavailable)
        }
    }

    #[test]
    fn terminal_test_errors_are_bounded_and_control_free() {
        assert_eq!(bounded_error("failed\nnow".into()), "failed now");
        assert_eq!(
            bounded_error("\n".into()),
            "sandbox test failed without an error message"
        );
        const { assert!(TEST_RUN_LEASE_SECONDS > 0) };
    }

    #[tokio::test]
    async fn revisioned_test_carries_imported_parent_lineage_to_the_policy_gate() {
        let tenant_id = Uuid::new_v4();
        let mut script = Script::new(
            "imported_tax_rule",
            RhaiWorkspace::single_source("42"),
            ScriptTrigger::Manual,
        );
        script.tenant_id = tenant_id;
        script.parent_release = Some(rustok_modules::ArtifactReleaseRef {
            slug: "tax_rule".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        });
        script.workspace.files.push(RhaiWorkspaceFile {
            path: "tests/smoke.rhai".to_string(),
            kind: RhaiWorkspaceFileKind::Test,
            contents: "true".to_string(),
        });
        let storage = Arc::new(InMemoryStorage::new());
        let saved = storage.save(script).await.expect("save imported draft");
        let runtime = crate::create_test_alloy_draft_runtime().with_imported_draft_policy_provider(
            AlloyImportedDraftPolicyProviderHandle(Arc::new(
                UnavailableImportedDraftPolicyProvider,
            )),
        );

        let run = RevisionedTestRunner::new(runtime, storage)
            .execute(TestCommand {
                script_id: saved.id,
                expected_revision: saved.version,
                test_path: "tests/smoke.rhai".to_string(),
                actor_id: "operator:42".to_string(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("terminal failed test evidence");

        assert_eq!(run.status, TestRunStatus::Failed);
        assert_eq!(run.passed, Some(false));
        assert_eq!(
            run.error.as_deref(),
            Some("Runtime error: imported Alloy draft parent policy is unavailable")
        );
    }
}
