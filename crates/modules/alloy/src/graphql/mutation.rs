use std::collections::HashMap;

use async_graphql::{Context, Json, Object, Result};
use chrono::Utc;
use rustok_api::RuntimeLocale;
use uuid::Uuid;

use crate::{
    AlloyAuthoringService, AlloyImportError, AlloyPublishedReleaseImportCommand,
    AlloyReleaseImporter, AlloyReleaseStageCommand, AuthoringOrigin, CreateAlloyScriptCommand,
    RevisionedReleaseStager, RevisionedTestRunner, ScriptEvidenceRetentionCommand, ScriptRegistry,
    TestCommand, UpdateAlloyScriptCommand, alloy_release_command_context,
    model::{ReviewCommand, Script, ScriptDeletionCommand, ScriptStatus},
    runner::ExecutionOutcome,
    utils::{dynamic_to_json, json_to_dynamic},
};

use super::{
    CreateScriptInput, DeleteScriptInput, GqlDeletedEvidenceRetention, GqlExecutionResult,
    GqlImportedDraft, GqlReviewDecision, GqlScript, GqlScriptStatus, GqlStageRelease, GqlTestRun,
    ImportPublishedReleaseInput, ReviewScriptInput, RunScriptInput, RunWorkspaceTestInput,
    StageReleaseInput, UpdateDeletedEvidenceRetentionInput, UpdateScriptInput,
    published_rhai_source_from_graphql_ctx, release_governance_from_graphql_ctx, require_admin,
    require_release_admin, runtime_from_graphql_ctx,
};

fn parse_runtime_locale(locale: Option<String>) -> Result<Option<RuntimeLocale>> {
    locale
        .map(|locale| {
            RuntimeLocale::new(&locale).map_err(|_| {
                async_graphql::Error::new(
                    "descriptionLocale must be a concrete normalized source locale",
                )
            })
        })
        .transpose()
}

fn ensure_expected_revision(script: &Script, expected_version: u32) -> Result<()> {
    if script.version != expected_version {
        return Err(async_graphql::Error::new(format!(
            "Script revision conflict: expected version {expected_version}"
        )));
    }
    Ok(())
}

fn import_error(error: AlloyImportError) -> async_graphql::Error {
    match error {
        AlloyImportError::SourceUnavailable(_) => {
            async_graphql::Error::new("The canonical published Rhai workspace is unavailable")
        }
        AlloyImportError::InvalidCommand
        | AlloyImportError::IneligibleRelease
        | AlloyImportError::InvalidSource => async_graphql::Error::new(
            "The published release cannot be imported as an Alloy Rhai workspace",
        ),
        AlloyImportError::IdempotencyConflict => async_graphql::Error::new(
            "Alloy import idempotency key was reused for a different release command",
        ),
        AlloyImportError::DraftNameConflict => async_graphql::Error::new(
            "An Alloy draft with the requested tenant-scoped name already exists",
        ),
        AlloyImportError::Storage(_) => async_graphql::Error::new("Alloy release import failed"),
    }
}

#[derive(Default)]
pub struct AlloyMutation;

#[Object]
impl AlloyMutation {
    async fn create_script(
        &self,
        ctx: &Context<'_>,
        input: CreateScriptInput,
    ) -> Result<GqlScript> {
        let auth = require_admin(ctx).await?;
        if matches!(input.status, Some(status) if status != GqlScriptStatus::Draft) {
            return Err(async_graphql::Error::new(
                "Alloy createScript creates a draft; use a lifecycle mutation after creation",
            ));
        }
        let description_locale = parse_runtime_locale(input.description_locale)?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let service = AlloyAuthoringService::from_scoped(runtime.clone());
        let actor_id = auth.user_id.to_string();
        let saved = service
            .create_script_from(
                AuthoringOrigin::Graphql,
                &actor_id,
                CreateAlloyScriptCommand {
                    name: input.name,
                    description: input.description,
                    description_locale,
                    workspace: input.workspace.0,
                    trigger: input.trigger.into(),
                    permissions: input.permissions,
                    run_as_system: input.run_as_system,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let script = runtime
            .storage
            .get(saved.id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(script.into())
    }

    async fn update_script(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: UpdateScriptInput,
    ) -> Result<GqlScript> {
        let auth = require_admin(ctx).await?;
        let description_locale = parse_runtime_locale(input.description_locale)?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let service = AlloyAuthoringService::from_scoped(runtime.clone());
        let actor_id = auth.user_id.to_string();
        let saved = service
            .update_script_from(
                AuthoringOrigin::Graphql,
                &actor_id,
                UpdateAlloyScriptCommand {
                    script_id: id,
                    expected_version: input.expected_version,
                    name: input.name,
                    description: input.description,
                    description_locale,
                    expected_description_copy_revision: input.expected_description_copy_revision,
                    workspace: input.workspace.map(|workspace| workspace.0),
                    trigger: input.trigger.map(Into::into),
                    status: input.status.map(Into::into),
                    run_as_system: input.run_as_system,
                    permissions: input.permissions,
                },
            )
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        let script = runtime
            .storage
            .get(saved.id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(script.into())
    }

    async fn delete_script(&self, ctx: &Context<'_>, input: DeleteScriptInput) -> Result<bool> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let script = match runtime.storage.get(input.id).await {
            Ok(script) => {
                ensure_expected_revision(&script, input.expected_version)?;
                Some(script)
            }
            Err(crate::ScriptError::NotFound { .. }) => None,
            Err(error) => return Err(async_graphql::Error::new(error.to_string())),
        };
        runtime
            .storage
            .delete(ScriptDeletionCommand {
                script_id: input.id,
                expected_revision: input.expected_version,
                actor_id: auth.user_id.to_string(),
                reason: input.reason,
                idempotency_key: input.idempotency_key,
            })
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        if let Some(script) = script {
            runtime.engine.invalidate(&script.name);
        }

        Ok(true)
    }

    async fn update_deleted_evidence_retention(
        &self,
        ctx: &Context<'_>,
        input: UpdateDeletedEvidenceRetentionInput,
    ) -> Result<GqlDeletedEvidenceRetention> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let retention = runtime
            .storage
            .update_deleted_evidence_retention(ScriptEvidenceRetentionCommand {
                script_id: input.script_id,
                deletion_request_digest: input.deletion_request_digest,
                expected_retention_revision: input.expected_retention_revision,
                action: input.action.into(),
                actor_id: auth.user_id.to_string(),
                reason: input.reason,
                idempotency_key: input.idempotency_key,
            })
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(retention.into())
    }

    async fn run_script(
        &self,
        ctx: &Context<'_>,
        input: RunScriptInput,
    ) -> Result<GqlExecutionResult> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let user_id = Some(auth.user_id.to_string());

        let params = input
            .params
            .map(|params| -> Result<HashMap<String, rhai::Dynamic>> {
                let object = params
                    .0
                    .as_object()
                    .ok_or_else(|| async_graphql::Error::new("params must be a JSON object"))?;
                Ok(object
                    .iter()
                    .map(|(key, value)| (key.clone(), json_to_dynamic(value.clone())))
                    .collect())
            })
            .transpose()?
            .unwrap_or_default();

        let script = runtime
            .storage
            .get_by_name(&input.script_name)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        if script.version != input.expected_version {
            return Err(async_graphql::Error::new(
                crate::ScriptError::RevisionConflict {
                    expected: input.expected_version,
                }
                .to_string(),
            ));
        }
        let result = runtime
            .orchestrator
            .run_manual_snapshot(&script, params, None, user_id)
            .await;

        let (success, error, return_value, changes) = match result.outcome {
            ExecutionOutcome::Success {
                ref return_value,
                ref entity_changes,
            } => (
                true,
                None,
                return_value.clone().map(dynamic_to_json),
                Some(serde_json::Value::Object(
                    entity_changes
                        .iter()
                        .map(|(key, value)| (key.clone(), dynamic_to_json(value.clone())))
                        .collect(),
                )),
            ),
            ExecutionOutcome::Aborted { ref reason } => (false, Some(reason.clone()), None, None),
            ExecutionOutcome::Failed { ref error } => (false, Some(error.to_string()), None, None),
        };

        Ok(GqlExecutionResult {
            execution_id: result.execution_id,
            success,
            duration_ms: result.duration_ms(),
            error,
            return_value: return_value.map(Json),
            changes: changes.map(Json),
        })
    }

    async fn review_script(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: ReviewScriptInput,
    ) -> Result<GqlReviewDecision> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let decision = runtime
            .storage
            .review(ReviewCommand {
                script_id: id,
                expected_revision: input.expected_version,
                status: input.status.into(),
                policy_revision: input.policy_revision,
                actor_id: auth.user_id.to_string(),
                reason: input.reason,
                idempotency_key: input.idempotency_key,
            })
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(decision.into())
    }

    async fn run_workspace_test(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: RunWorkspaceTestInput,
    ) -> Result<GqlTestRun> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let run = RevisionedTestRunner::new(runtime.sandbox.clone(), runtime.storage.clone())
            .execute(TestCommand {
                script_id: id,
                expected_revision: input.expected_version,
                test_path: input.test_path,
                actor_id: auth.user_id.to_string(),
                idempotency_key: input.idempotency_key,
            })
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(run.into())
    }

    async fn stage_release(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        input: StageReleaseInput,
    ) -> Result<GqlStageRelease> {
        let auth = require_release_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let governance = release_governance_from_graphql_ctx(ctx)?;
        let result = RevisionedReleaseStager::new(
            runtime.sandbox.clone(),
            runtime.storage.clone(),
            governance.0,
        )
        .stage(AlloyReleaseStageCommand {
            script_id: id,
            expected_revision: input.expected_version,
            publish_request_id: input.publish_request_id,
            expected_publish_request_revision: input.expected_publish_request_revision,
            artifact_digest: input.artifact_digest,
            context: alloy_release_command_context(
                auth.tenant_id,
                auth.user_id,
                input.idempotency_key,
            ),
            // `require_release_admin` established the host permission; the
            // registry owner rechecks it against current durable state.
            actor_can_manage_modules: true,
        })
        .await
        .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        Ok(GqlStageRelease {
            staging_id: result.staging_id,
            created: result.created,
            request_revision: result.request_revision,
        })
    }

    async fn import_published_release(
        &self,
        ctx: &Context<'_>,
        input: ImportPublishedReleaseInput,
    ) -> Result<GqlImportedDraft> {
        let auth = require_release_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let source = published_rhai_source_from_graphql_ctx(ctx)?;
        let result = AlloyReleaseImporter::new(runtime.storage.clone(), source.0)
            .import(AlloyPublishedReleaseImportCommand {
                tenant_id: runtime.tenant_id,
                release: rustok_modules::ArtifactReleaseRef {
                    slug: input.release.slug,
                    version: input.release.version,
                    digest: input.release.digest,
                },
                draft_name: input.draft_name,
                actor_id: auth.user_id.to_string(),
                idempotency_key: input.idempotency_key,
            })
            .await
            .map_err(import_error)?;
        Ok(GqlImportedDraft {
            script: result.script.into(),
            created: result.created,
        })
    }

    async fn activate_script(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        expected_version: u32,
    ) -> Result<GqlScript> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        ensure_expected_revision(&script, expected_version)?;

        script.activate();
        script.author_id = Some(auth.user_id.to_string());
        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn pause_script(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        expected_version: u32,
    ) -> Result<GqlScript> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        ensure_expected_revision(&script, expected_version)?;

        script.status = ScriptStatus::Paused;
        script.updated_at = Utc::now();
        script.author_id = Some(auth.user_id.to_string());

        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn disable_script(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        expected_version: u32,
    ) -> Result<GqlScript> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        ensure_expected_revision(&script, expected_version)?;

        script.disable();
        script.author_id = Some(auth.user_id.to_string());
        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn archive_script(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        expected_version: u32,
    ) -> Result<GqlScript> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        ensure_expected_revision(&script, expected_version)?;

        script.archive();
        script.author_id = Some(auth.user_id.to_string());
        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }

    async fn reset_script_errors(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        expected_version: u32,
    ) -> Result<GqlScript> {
        let auth = require_admin(ctx).await?;
        let runtime = runtime_from_graphql_ctx(ctx)?;
        let mut script = runtime
            .storage
            .get(id)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
        ensure_expected_revision(&script, expected_version)?;

        script.reset_errors();
        script.updated_at = Utc::now();
        script.author_id = Some(auth.user_id.to_string());

        let saved = runtime
            .storage
            .save(script)
            .await
            .map_err(|error| async_graphql::Error::new(error.to_string()))?;

        Ok(saved.into())
    }
}
