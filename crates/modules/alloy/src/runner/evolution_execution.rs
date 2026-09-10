//! Owner-composed recording of completed Rust Component candidate builds.
//!
//! Alloy never accepts a build result from a transport or worker callback. It
//! reads the immutable completion pair from `rustok-modules`, binds it to the
//! durable candidate/build receipt, and retains only redacted evidence needed
//! for later review and release staging.

use std::sync::Arc;

use rustok_modules::{ModuleBuildProtocolError, ModuleBuildResultReader};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    RustComponentCandidateBuildExecution, RustComponentCandidateExecutionError, ScriptError,
    ScriptRegistry,
};

/// Resolves completed owner build results for approved Alloy Component
/// candidates. The build-result reader is injected by the host; this service
/// has no database connection, worker client, filesystem path, or Cargo path.
pub struct AlloyEvolutionExecutionService<R: ScriptRegistry, B: ModuleBuildResultReader> {
    registry: Arc<R>,
    build_results: Arc<B>,
}

impl<R: ScriptRegistry, B: ModuleBuildResultReader> AlloyEvolutionExecutionService<R, B> {
    pub fn new(registry: Arc<R>, build_results: Arc<B>) -> Self {
        Self {
            registry,
            build_results,
        }
    }

    /// Records the one immutable successful completion associated with a
    /// durable candidate-build receipt. Replays return the prior evidence and
    /// do not trust a caller-provided result. This proves only the owner
    /// completion contract; deployment-level hardened OCI-job evidence remains
    /// a separately operated requirement.
    pub async fn record_completed_build(
        &self,
        candidate_id: Uuid,
        build_request_id: Uuid,
    ) -> Result<RustComponentCandidateBuildExecution, AlloyEvolutionExecutionError> {
        let candidate = self.registry.get_component_candidate(candidate_id).await?;
        let candidate_build = self
            .registry
            .get_component_candidate_build_by_request(candidate.id, build_request_id)
            .await?
            .ok_or(AlloyEvolutionExecutionError::CandidateBuildNotFound)?;
        if let Some(existing) = self
            .registry
            .get_component_candidate_build_execution(candidate.id, candidate_build.build_request_id)
            .await?
        {
            return Ok(existing);
        }
        let completed = self
            .build_results
            .load_completed(candidate.tenant_id, candidate_build.build_request_id)
            .await?;
        let execution = RustComponentCandidateBuildExecution::from_completed_build(
            &candidate,
            &candidate_build,
            &completed,
        )?;
        Ok(self
            .registry
            .record_component_candidate_build_execution(execution)
            .await?)
    }
}

#[derive(Debug, Error)]
pub enum AlloyEvolutionExecutionError {
    #[error("the Rust Component candidate build receipt was not found")]
    CandidateBuildNotFound,
    #[error(transparent)]
    Script(#[from] ScriptError),
    #[error(transparent)]
    CandidateExecution(#[from] RustComponentCandidateExecutionError),
    #[error(transparent)]
    ModuleBuild(#[from] ModuleBuildProtocolError),
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use rustok_modules::{
        ArtifactReleaseRef, MODULE_BUILD_COMPONENT_TARGET, MODULE_BUILD_PROTOCOL_VERSION,
        MODULE_BUILD_RUNTIME_ABI, MODULE_BUILD_SANDBOX_SCENARIO_PATH, MODULE_BUILD_WIT_VERSION,
        MODULE_BUILD_WIT_WORLD, ModuleAuthoringBuildSubmission, ModuleBuildAuthoring,
        ModuleBuildCompletedResult, ModuleBuildComponentInterface, ModuleBuildDependencyPolicy,
        ModuleBuildEvidence, ModuleBuildLimits, ModuleBuildMetrics, ModuleBuildNetworkPolicy,
        ModuleBuildNextAction, ModuleBuildOutcome, ModuleBuildProtocolError,
        ModuleBuildPublicationReceipt, ModuleBuildRequest, ModuleBuildResult,
        ModuleBuildResultReader, ModuleBuildScenario, ModuleBuildSignatureAuthority,
        ModuleBuildSource, ModuleBuildToolchain, ModuleBuildValidationOutcome,
        ModuleBuildValidationProfile, ModuleBuildValidationResult, ModuleBuildWitContract,
        ModuleCommandContext, OciArtifactReference,
    };
    use rustok_sandbox::{LocalSandboxScenarioComparison, LocalSandboxScenarioResult};
    use uuid::Uuid;

    use super::*;
    use crate::{
        InMemoryStorage, ReviewCommand, ReviewStatus, RhaiWorkspace, RustComponentCandidateBuild,
        RustComponentCandidateBuildCommand, RustComponentCandidateCommand,
        RustComponentCandidateReviewCommand, RustComponentSourceFile, RustComponentWorkspace,
        Script, ScriptRegistry, ScriptTrigger,
    };

    #[derive(Clone)]
    struct RecordingBuildResultReader {
        completed: Arc<Mutex<ModuleBuildCompletedResult>>,
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl ModuleBuildResultReader for RecordingBuildResultReader {
        async fn load_completed(
            &self,
            tenant_id: Uuid,
            request_id: Uuid,
        ) -> Result<ModuleBuildCompletedResult, ModuleBuildProtocolError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let completed = self
                .completed
                .lock()
                .expect("completed result lock")
                .clone();
            if completed.request.context.tenant_id != Some(tenant_id)
                || completed.request.request_id != request_id
            {
                return Err(ModuleBuildProtocolError::UnknownRequest);
            }
            Ok(completed)
        }
    }

    fn digest(marker: char) -> String {
        format!("sha256:{}", marker.to_string().repeat(64))
    }

    fn workspace() -> RustComponentWorkspace {
        let rendered =
            rustok_module_template::render(&rustok_module_template::ModuleTemplateInput {
                slug: "sample_module".to_string(),
                version: "1.1.0".to_string(),
                display_name: "Sample Module".to_string(),
            })
            .expect("rendered Component template");
        let mut files = rendered
            .files()
            .iter()
            .map(|file| RustComponentSourceFile {
                path: file.path.to_string(),
                contents: String::from_utf8(file.contents.clone()).expect("UTF-8 template file"),
            })
            .collect::<Vec<_>>();
        files.push(RustComponentSourceFile {
            path: "Cargo.lock".to_string(),
            contents: "# This file is automatically generated by Cargo.\nversion = 4\n".to_string(),
        });
        RustComponentWorkspace { files }
    }

    async fn candidate_build(
        storage: &Arc<InMemoryStorage>,
    ) -> (
        crate::RustComponentCandidate,
        RustComponentCandidateBuild,
        ModuleCommandContext,
    ) {
        let tenant_id = Uuid::new_v4();
        let parent_release = ArtifactReleaseRef {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: digest('a'),
        };
        let mut script = Script::new(
            "component_candidate_parent",
            RhaiWorkspace::single_source("40 + 2"),
            ScriptTrigger::Manual,
        );
        script.tenant_id = tenant_id;
        script.parent_release = Some(parent_release);
        let script = storage.save(script).await.expect("parent draft");
        storage
            .review(ReviewCommand {
                script_id: script.id,
                expected_revision: script.version,
                status: ReviewStatus::Approved,
                policy_revision: "policy:parent".to_string(),
                actor_id: "operator:parent-reviewer".to_string(),
                reason: None,
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("approved parent");
        let candidate = storage
            .create_component_candidate(RustComponentCandidateCommand {
                script_id: script.id,
                expected_revision: script.version,
                workspace: workspace(),
                actor_id: "operator:evolution".to_string(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("candidate");
        storage
            .review_component_candidate(RustComponentCandidateReviewCommand {
                candidate_id: candidate.id,
                status: ReviewStatus::Approved,
                policy_revision: "policy:candidate".to_string(),
                actor_id: "operator:candidate-reviewer".to_string(),
                reason: None,
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("approved candidate");
        let context = ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id: Some(tenant_id),
            trace_id: "trace:alloy-component-candidate".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        };
        let command = RustComponentCandidateBuildCommand {
            candidate_id: candidate.id,
            context: context.clone(),
            project_id: "alloy-component-candidate".to_string(),
            rust_toolchain: "1.90.0".to_string(),
            sdk_version: "0.1.0".to_string(),
            template_version: "0.1.0".to_string(),
            dependency_lock_digest: digest('b'),
        };
        let submission = ModuleAuthoringBuildSubmission {
            request_id: Uuid::new_v4(),
            build_created: true,
            source_created: true,
            source_reference: format!("cas://{}", digest('c')),
            source_digest: digest('c'),
            archive_bytes: 1_024,
            source_bytes: 512,
            entries: 8,
        };
        let build = RustComponentCandidateBuild::from_submission(&candidate, &command, &submission)
            .expect("candidate build receipt");
        let build = storage
            .record_component_candidate_build(build)
            .await
            .expect("durable candidate build receipt");
        (candidate, build, context)
    }

    fn completed_result(
        candidate: &crate::RustComponentCandidate,
        build: &RustComponentCandidateBuild,
        context: ModuleCommandContext,
    ) -> ModuleBuildCompletedResult {
        let manifest = candidate
            .workspace
            .source_manifest()
            .expect("candidate manifest");
        let request = ModuleBuildRequest {
            protocol_version: MODULE_BUILD_PROTOCOL_VERSION,
            request_id: build.build_request_id,
            context,
            project_id: "alloy-component-candidate".to_string(),
            source: ModuleBuildSource {
                digest: build.archive_source_digest.clone(),
                reference: build.source_reference.clone(),
            },
            scenario: ModuleBuildScenario {
                source_path: MODULE_BUILD_SANDBOX_SCENARIO_PATH.to_string(),
                digest: candidate.scenario_digest.clone(),
            },
            expected_module_slug: manifest.slug().to_string(),
            expected_version: manifest.version().to_string(),
            parent_release: Some(candidate.parent_release.clone()),
            runtime_abi: MODULE_BUILD_RUNTIME_ABI.to_string(),
            wit: ModuleBuildWitContract {
                world: MODULE_BUILD_WIT_WORLD.to_string(),
                version: MODULE_BUILD_WIT_VERSION.to_string(),
            },
            toolchain: ModuleBuildToolchain {
                rust_toolchain: "1.90.0".to_string(),
                component_target: MODULE_BUILD_COMPONENT_TARGET.to_string(),
            },
            authoring: ModuleBuildAuthoring {
                sdk_version: "0.1.0".to_string(),
                template_version: "0.1.0".to_string(),
            },
            dependency_policy: ModuleBuildDependencyPolicy {
                lock_digest: digest('b'),
                allowed_registries: vec!["https://crates.io".to_string()],
                allow_git_dependencies: false,
                allow_build_scripts: false,
                allow_native_links: false,
            },
            limits: ModuleBuildLimits {
                cpu_cores: 2,
                memory_bytes: 512 * 1024 * 1024,
                disk_bytes: 2 * 1024 * 1024 * 1024,
                process_limit: 32,
                output_bytes: 1024 * 1024,
                wall_clock_ms: 300_000,
            },
            network_policy: ModuleBuildNetworkPolicy::Denied,
            validation_profiles: vec![
                ModuleBuildValidationProfile::Check,
                ModuleBuildValidationProfile::Test,
            ],
            attempt: 1,
        };
        request.validate().expect("valid immutable build request");
        let result = ModuleBuildResult {
            protocol_version: request.protocol_version,
            request_id: request.request_id,
            tenant_id: request.context.tenant_id.expect("tenant"),
            attempt: request.attempt,
            outcome: ModuleBuildOutcome::Succeeded,
            source_digest: request.source.digest.clone(),
            dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
            toolchain_digest: request.toolchain.protocol_digest(),
            wit_digest: request.wit.protocol_digest(),
            component_digest: Some(digest('d')),
            sbom_digest: Some(digest('e')),
            provenance_digest: Some(digest('f')),
            component_interface: Some(ModuleBuildComponentInterface {
                exports: vec!["run".to_string()],
                imports: Vec::new(),
            }),
            evidence: ModuleBuildEvidence {
                log_reference: "cas://logs/1".to_string(),
                policy_report_reference: "cas://reports/1".to_string(),
                validation_results: vec![
                    ModuleBuildValidationResult {
                        profile: ModuleBuildValidationProfile::Check,
                        outcome: ModuleBuildValidationOutcome::Passed,
                    },
                    ModuleBuildValidationResult {
                        profile: ModuleBuildValidationProfile::Test,
                        outcome: ModuleBuildValidationOutcome::Passed,
                    },
                ],
                scenario_comparison: Some(LocalSandboxScenarioComparison {
                    scenario_digest: request.scenario.digest.clone(),
                    result: LocalSandboxScenarioResult::Success,
                }),
                diagnostics: Vec::new(),
            },
            publication: Some(ModuleBuildPublicationReceipt {
                artifact: OciArtifactReference {
                    registry: "registry.example".to_string(),
                    repository: "modules/sample-module".to_string(),
                    digest: digest('1'),
                },
                signature_manifest: OciArtifactReference {
                    registry: "registry.example".to_string(),
                    repository: "modules/sample-module".to_string(),
                    digest: digest('2'),
                },
                signature_authority: ModuleBuildSignatureAuthority::BuildService,
            }),
            metrics: ModuleBuildMetrics {
                duration_ms: 1,
                peak_memory_bytes: 1,
                output_bytes: 1,
            },
            retryable: false,
            next_action: ModuleBuildNextAction::AdmitArtifact,
        };
        result
            .validate_against(&request)
            .expect("valid completed build result");
        ModuleBuildCompletedResult {
            request,
            result,
            revision: 3,
        }
    }

    #[tokio::test]
    async fn completed_owner_result_is_bound_and_recorded_idempotently() {
        let storage = Arc::new(InMemoryStorage::new());
        let (candidate, build, context) = candidate_build(&storage).await;
        let reader = Arc::new(RecordingBuildResultReader {
            completed: Arc::new(Mutex::new(completed_result(&candidate, &build, context))),
            calls: Arc::new(AtomicUsize::new(0)),
        });
        let service = AlloyEvolutionExecutionService::new(storage.clone(), reader.clone());

        let execution = service
            .record_completed_build(candidate.id, build.build_request_id)
            .await
            .expect("recorded execution evidence");
        assert_eq!(execution.candidate_build_id, build.id);
        assert_eq!(execution.candidate_source_digest, candidate.source_digest);
        assert_eq!(execution.scenario_digest, candidate.scenario_digest);
        assert_eq!(execution.archive_source_digest, build.archive_source_digest);
        assert_eq!(reader.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            service
                .record_completed_build(candidate.id, build.build_request_id)
                .await
                .expect("idempotent execution evidence replay"),
            execution
        );
        assert_eq!(reader.calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            storage
                .get_component_candidate_build_execution(candidate.id, build.build_request_id)
                .await
                .expect("execution evidence read"),
            Some(execution)
        );
    }

    #[tokio::test]
    async fn substituted_owner_scenario_is_rejected_without_durable_evidence() {
        let storage = Arc::new(InMemoryStorage::new());
        let (candidate, build, context) = candidate_build(&storage).await;
        let mut completed = completed_result(&candidate, &build, context);
        completed.request.scenario.digest = digest('9');
        let reader = Arc::new(RecordingBuildResultReader {
            completed: Arc::new(Mutex::new(completed)),
            calls: Arc::new(AtomicUsize::new(0)),
        });
        let service = AlloyEvolutionExecutionService::new(storage.clone(), reader);

        assert!(matches!(
            service
                .record_completed_build(candidate.id, build.build_request_id)
                .await,
            Err(AlloyEvolutionExecutionError::CandidateExecution(
                RustComponentCandidateExecutionError::InvalidEvidence
            ))
        ));
        assert_eq!(
            storage
                .get_component_candidate_build_execution(candidate.id, build.build_request_id)
                .await
                .expect("execution evidence read"),
            None
        );
    }
}
