use std::sync::Arc;

use async_trait::async_trait;
use rustok_modules::ArtifactReleaseRef;

use crate::model::{
    AlloyImportError, AlloyImportedDraftCommand, AlloyImportedDraftResult,
    AlloyPublishedReleaseImportCommand, AlloyPublishedRhaiSource,
};
use crate::storage::ScriptRegistry;

/// Owner boundary for loading one exact eligible published Rhai release.
/// Implementations may use registry/CAS infrastructure, but Alloy receives
/// only immutable release metadata and canonical workspace bytes.
#[async_trait]
pub trait AlloyPublishedRhaiSourceProvider: Send + Sync {
    async fn load_published_rhai_source(
        &self,
        release: &ArtifactReleaseRef,
    ) -> Result<AlloyPublishedRhaiSource, AlloyImportError>;
}

/// Host-composed provider handle for immutable marketplace source import.
/// Alloy owns draft persistence and lineage, while the host injects the module
/// owner that resolves the exact released workspace from its canonical
/// projection and artifact store.
#[derive(Clone)]
pub struct AlloyPublishedRhaiSourceProviderHandle(pub Arc<dyn AlloyPublishedRhaiSourceProvider>);

pub struct AlloyReleaseImporter<R, P>
where
    R: ScriptRegistry,
    P: AlloyPublishedRhaiSourceProvider + ?Sized,
{
    registry: Arc<R>,
    source: Arc<P>,
}

impl<R, P> AlloyReleaseImporter<R, P>
where
    R: ScriptRegistry,
    P: AlloyPublishedRhaiSourceProvider + ?Sized,
{
    pub fn new(registry: Arc<R>, source: Arc<P>) -> Self {
        Self { registry, source }
    }

    pub async fn import(
        &self,
        command: AlloyPublishedReleaseImportCommand,
    ) -> Result<AlloyImportedDraftResult, AlloyImportError> {
        command.validate()?;
        let source = self
            .source
            .load_published_rhai_source(&command.release)
            .await?;
        let source_digest = source.validate_for(&command.release)?;
        let imported = AlloyImportedDraftCommand::from_source(&command, &source, &source_digest)?;
        self.registry
            .import_published_release(imported)
            .await
            .map_err(|error| match error {
                crate::ScriptError::ImportIdempotencyConflict => {
                    AlloyImportError::IdempotencyConflict
                }
                crate::ScriptError::ImportDraftNameConflict => AlloyImportError::DraftNameConflict,
                error => AlloyImportError::Storage(error.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::{
        InMemoryStorage, RhaiWorkspace, Script, ScriptRegistry, ScriptTrigger,
        stage_rhai_module_release,
    };

    #[derive(Clone)]
    struct FixedSourceProvider {
        source: AlloyPublishedRhaiSource,
    }

    #[async_trait]
    impl AlloyPublishedRhaiSourceProvider for FixedSourceProvider {
        async fn load_published_rhai_source(
            &self,
            release: &ArtifactReleaseRef,
        ) -> Result<AlloyPublishedRhaiSource, AlloyImportError> {
            if self.source.release.descriptor.release_ref() != *release {
                return Err(AlloyImportError::SourceUnavailable(
                    "release was not found".to_string(),
                ));
            }
            Ok(self.source.clone())
        }
    }

    fn published_source() -> AlloyPublishedRhaiSource {
        let script = Script::new(
            "published_tax_rule",
            RhaiWorkspace::single_source("40 + 2"),
            ScriptTrigger::Manual,
        );
        let release = stage_rhai_module_release("tax_rule", "1.0.0", &script, Vec::new())
            .expect("stage Rhai release")
            .publish(Utc::now())
            .expect("publish Rhai release");
        AlloyPublishedRhaiSource {
            release,
            workspace: script.workspace,
        }
    }

    #[tokio::test]
    async fn import_is_exactly_replayable_and_preserves_release_lineage() {
        let source = published_source();
        let release = source.release.descriptor.release_ref();
        let storage = Arc::new(InMemoryStorage::new());
        let importer =
            AlloyReleaseImporter::new(storage.clone(), Arc::new(FixedSourceProvider { source }));
        let command = AlloyPublishedReleaseImportCommand {
            tenant_id: uuid::Uuid::new_v4(),
            release: release.clone(),
            draft_name: "imported_tax_rule".to_string(),
            actor_id: "operator-42".to_string(),
            idempotency_key: uuid::Uuid::new_v4(),
        };

        let created = importer
            .import(command.clone())
            .await
            .expect("first import");
        let replay = importer.import(command).await.expect("exact replay");

        assert!(created.created);
        assert!(!replay.created);
        assert_eq!(replay.script.id, created.script.id);
        assert_eq!(created.script.parent_release, Some(release.clone()));
        let revision = storage
            .get_source_revision(created.script.id, 1)
            .await
            .expect("source revision");
        assert_eq!(revision.parent_release, Some(release));
        assert_eq!(
            revision.source_digest,
            created.script.workspace.digest().expect("workspace digest")
        );
    }

    #[tokio::test]
    async fn import_rejects_conflicting_idempotency_key_reuse() {
        let source = published_source();
        let storage = Arc::new(InMemoryStorage::new());
        let importer = AlloyReleaseImporter::new(
            storage,
            Arc::new(FixedSourceProvider {
                source: source.clone(),
            }),
        );
        let mut command = AlloyPublishedReleaseImportCommand {
            tenant_id: uuid::Uuid::new_v4(),
            release: source.release.descriptor.release_ref(),
            draft_name: "imported_tax_rule".to_string(),
            actor_id: "operator-42".to_string(),
            idempotency_key: uuid::Uuid::new_v4(),
        };
        importer
            .import(command.clone())
            .await
            .expect("first import");
        command.draft_name = "different_name".to_string();

        let error = importer
            .import(command)
            .await
            .expect_err("conflicting replay must fail");

        assert_eq!(error, AlloyImportError::IdempotencyConflict);
    }

    #[tokio::test]
    async fn import_rejects_a_duplicate_tenant_scoped_draft_name() {
        let source = published_source();
        let storage = Arc::new(InMemoryStorage::new());
        let importer = AlloyReleaseImporter::new(
            storage,
            Arc::new(FixedSourceProvider {
                source: source.clone(),
            }),
        );
        let command = AlloyPublishedReleaseImportCommand {
            tenant_id: uuid::Uuid::new_v4(),
            release: source.release.descriptor.release_ref(),
            draft_name: "imported_tax_rule".to_string(),
            actor_id: "operator-42".to_string(),
            idempotency_key: uuid::Uuid::new_v4(),
        };
        importer
            .import(command.clone())
            .await
            .expect("first import");
        let retry_as_new_command = AlloyPublishedReleaseImportCommand {
            idempotency_key: uuid::Uuid::new_v4(),
            ..command
        };

        let error = importer
            .import(retry_as_new_command)
            .await
            .expect_err("duplicate draft name must fail");

        assert_eq!(error, AlloyImportError::DraftNameConflict);
    }

    #[test]
    fn import_rejects_a_published_workspace_with_a_different_runtime_abi() {
        let mut source = published_source();
        source.release.descriptor.runtime_abi = "rustok:module/runtime@other".to_string();

        assert_eq!(
            source.validate_for(&source.release.descriptor.release_ref()),
            Err(AlloyImportError::IneligibleRelease)
        );
    }
}
