//! Owner-composed dispatch of approved Alloy Rust Component candidates.
//!
//! This boundary accepts only a durable candidate identifier and command
//! evidence. It materializes the reviewed data-only source below a host-owned
//! work root, creates the deterministic archive, and hands that archive to the
//! module build owner. No caller-provided filesystem path crosses this API.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use rustok_modules::{
    ModuleAuthoringBuildCommand, ModuleAuthoringBuildControl, ModuleAuthoringBuildError,
    ModuleAuthoringSourceArchiveBuilder, ModuleBuildScenario,
};
use thiserror::Error;

use crate::{
    RustComponentCandidateBuild, RustComponentCandidateBuildCommand,
    RustComponentCandidateBuildError, ScriptError, ScriptRegistry,
    validate_candidate_parent_release,
};

/// Dispatches approved Component candidates through the module build owner.
///
/// `working_root` is selected by the host at composition time. It must already
/// exist as a non-symlink directory; callers cannot select or override it.
pub struct AlloyEvolutionBuildService<R: ScriptRegistry, B: ModuleAuthoringBuildControl> {
    registry: Arc<R>,
    build_control: Arc<B>,
    working_root: PathBuf,
}

impl<R: ScriptRegistry, B: ModuleAuthoringBuildControl> AlloyEvolutionBuildService<R, B> {
    pub fn new(
        registry: Arc<R>,
        build_control: Arc<B>,
        working_root: PathBuf,
    ) -> Result<Self, AlloyEvolutionBuildError> {
        validate_working_root(&working_root)?;
        Ok(Self {
            registry,
            build_control,
            working_root,
        })
    }

    /// Idempotently materializes, archives, submits, and records one approved
    /// candidate. A completed receipt is returned before any filesystem work;
    /// reusing its idempotency key for different command evidence is rejected.
    pub async fn submit(
        &self,
        command: RustComponentCandidateBuildCommand,
    ) -> Result<RustComponentCandidateBuild, AlloyEvolutionBuildError> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        if let Some(receipt) = self
            .registry
            .get_component_candidate_build(command.candidate_id, command.context.idempotency_key)
            .await?
        {
            if receipt.request_digest != request_digest {
                return Err(RustComponentCandidateBuildError::IdempotencyConflict.into());
            }
            return Ok(receipt);
        }

        let candidate = self
            .registry
            .get_component_candidate(command.candidate_id)
            .await?;
        if command.context.tenant_id != Some(candidate.tenant_id) {
            return Err(RustComponentCandidateBuildError::InvalidCommand.into());
        }
        validate_candidate_parent_release(&candidate.workspace, &candidate.parent_release)
            .map_err(ScriptError::from)?;
        let approved = self
            .registry
            .list_component_candidate_reviews(candidate.id)
            .await?
            .last()
            .is_some_and(|review| {
                review.status == crate::ReviewStatus::Approved
                    && review.source_digest == candidate.source_digest
                    && review.scenario_digest == candidate.scenario_digest
            });
        if !approved {
            return Err(RustComponentCandidateBuildError::CandidateNotApproved.into());
        }

        let operation_root = self.operation_root(&command)?;
        fs::create_dir(&operation_root)?;
        let submission = self
            .prepare_and_submit(&candidate, &command, &operation_root)
            .await;
        let cleanup = fs::remove_dir_all(&operation_root);
        match (submission, cleanup) {
            (Ok(receipt), Ok(())) => Ok(receipt),
            (Ok(_), Err(error)) => Err(error.into()),
            (Err(error), _) => Err(error),
        }
    }

    async fn prepare_and_submit(
        &self,
        candidate: &crate::RustComponentCandidate,
        command: &RustComponentCandidateBuildCommand,
        operation_root: &Path,
    ) -> Result<RustComponentCandidateBuild, AlloyEvolutionBuildError> {
        let source_root = operation_root.join("source");
        let archive_path = operation_root.join("source.tar.gz");
        let archive_builder = ModuleAuthoringSourceArchiveBuilder::new()?;
        archive_builder.materialize(&candidate.workspace.source_files()?, &source_root)?;
        let archive = archive_builder.prepare(&source_root, &archive_path)?;
        let manifest = candidate.workspace.source_manifest()?;
        let build_command = ModuleAuthoringBuildCommand {
            context: command.context.clone(),
            project_id: command.project_id.clone(),
            source_digest: archive.source_digest().to_string(),
            scenario: ModuleBuildScenario {
                source_path: "tests/sandbox-scenario.json".to_string(),
                digest: candidate.scenario_digest.clone(),
            },
            expected_module_slug: manifest.slug().to_string(),
            expected_version: manifest.version().to_string(),
            parent_release: Some(candidate.parent_release.clone()),
            rust_toolchain: command.rust_toolchain.clone(),
            sdk_version: command.sdk_version.clone(),
            template_version: command.template_version.clone(),
            dependency_lock_digest: command.dependency_lock_digest.clone(),
        };
        let submission = self
            .build_control
            .submit_build(build_command, archive)
            .await?;
        let receipt =
            RustComponentCandidateBuild::from_submission(candidate, command, &submission)?;
        Ok(self
            .registry
            .record_component_candidate_build(receipt)
            .await?)
    }

    fn operation_root(
        &self,
        command: &RustComponentCandidateBuildCommand,
    ) -> Result<PathBuf, AlloyEvolutionBuildError> {
        let root = self.working_root.join(format!(
            "alloy-component-candidate-{}",
            command.context.correlation_id.simple()
        ));
        if root.exists() {
            return Err(AlloyEvolutionBuildError::OperationAlreadyExists);
        }
        Ok(root)
    }
}

fn validate_working_root(working_root: &Path) -> Result<(), AlloyEvolutionBuildError> {
    let metadata = fs::symlink_metadata(working_root)?;
    if !working_root.is_absolute() || !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(AlloyEvolutionBuildError::InvalidWorkingRoot);
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum AlloyEvolutionBuildError {
    #[error("the Alloy Component build work root is invalid")]
    InvalidWorkingRoot,
    #[error("the Component build operation work directory already exists")]
    OperationAlreadyExists,
    #[error("the Component build work directory operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Script(#[from] ScriptError),
    #[error(transparent)]
    CandidateBuild(#[from] RustComponentCandidateBuildError),
    #[error(transparent)]
    ModuleBuild(#[from] ModuleAuthoringBuildError),
    #[error("the approved Component source is invalid: {0}")]
    Workspace(#[from] crate::RustComponentWorkspaceError),
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::Arc};

    use async_trait::async_trait;
    use parking_lot::Mutex;
    use rustok_modules::{
        ArtifactReleaseRef, ModuleAuthoringBuildCommand, ModuleAuthoringBuildSubmission,
        PreparedModuleSourceArchive,
    };
    use uuid::Uuid;

    use super::*;
    use crate::{
        InMemoryStorage, ReviewCommand, ReviewStatus, RhaiWorkspace, RustComponentCandidateCommand,
        RustComponentCandidateReviewCommand, RustComponentSourceFile, RustComponentWorkspace,
        Script, ScriptRegistry, ScriptTrigger,
    };

    #[derive(Default)]
    struct RecordingBuildControl {
        commands: Mutex<Vec<ModuleAuthoringBuildCommand>>,
    }

    #[async_trait]
    impl ModuleAuthoringBuildControl for RecordingBuildControl {
        async fn submit_build(
            &self,
            command: ModuleAuthoringBuildCommand,
            archive: PreparedModuleSourceArchive,
        ) -> Result<ModuleAuthoringBuildSubmission, ModuleAuthoringBuildError> {
            assert_eq!(command.source_digest, archive.source_digest());
            command.validate()?;
            self.commands.lock().push(command.clone());
            Ok(ModuleAuthoringBuildSubmission {
                request_id: Uuid::new_v4(),
                build_created: true,
                source_created: true,
                source_reference: format!("cas://{}", command.source_digest),
                source_digest: command.source_digest,
                archive_bytes: 1_024,
                source_bytes: 512,
                entries: 8,
            })
        }
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("alloy-evolution-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("test work root");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn component_workspace() -> RustComponentWorkspace {
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

    #[tokio::test]
    async fn approved_candidate_is_materialized_in_host_root_and_submitted_once() {
        let storage = Arc::new(InMemoryStorage::new());
        let tenant_id = Uuid::new_v4();
        let parent_release = ArtifactReleaseRef {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
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
                workspace: component_workspace(),
                actor_id: "operator:evolution".to_string(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("durable candidate");
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

        let work_root = TestDirectory::new();
        let build_control = Arc::new(RecordingBuildControl::default());
        let service = AlloyEvolutionBuildService::new(
            storage.clone(),
            build_control.clone(),
            work_root.0.clone(),
        )
        .expect("host-owned build service");
        let command = RustComponentCandidateBuildCommand {
            candidate_id: candidate.id,
            context: rustok_modules::ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: Some(tenant_id),
                trace_id: "trace:alloy-component-candidate".to_string(),
                correlation_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
            },
            project_id: "alloy-component-candidate".to_string(),
            rust_toolchain: "1.90.0".to_string(),
            sdk_version: "0.1.0".to_string(),
            template_version: "0.1.0".to_string(),
            dependency_lock_digest: format!("sha256:{}", "b".repeat(64)),
        };
        let receipt = service
            .submit(command.clone())
            .await
            .expect("build submission");
        assert_eq!(receipt.candidate_id, candidate.id);
        assert_eq!(build_control.commands.lock().len(), 1);
        assert!(
            fs::read_dir(&work_root.0)
                .expect("work root entries")
                .next()
                .is_none()
        );
        assert_eq!(
            service.submit(command).await.expect("build replay"),
            receipt
        );
        assert_eq!(build_control.commands.lock().len(), 1);
    }
}
