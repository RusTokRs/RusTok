use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait,
};

use crate::error::{ScriptError, ScriptResult};
use crate::model::{
    AlloyImportedDraftCommand, AlloyImportedDraftResult, ReviewCommand, ReviewDecision,
    RhaiWorkspace, RustComponentCandidate, RustComponentCandidateBuild,
    RustComponentCandidateBuildExecution, RustComponentCandidateCommand,
    RustComponentCandidateReview, RustComponentCandidateReviewCommand, Script,
    ScriptDeletionCommand, ScriptEvidenceRetentionCommand, ScriptEvidenceRetentionState, ScriptId,
    ScriptSourceRevision, ScriptStatus, TestCommand, TestRun, TestRunClaim, TestRunCompletion,
};
use crate::storage::{ScriptPage, ScriptQuery, ScriptRegistry};

mod candidates;
pub mod entities;
mod mapping;
mod retention;
mod revisions;

#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use entities::{
    ActiveModel, Column, Entity, Model, component_candidate, component_candidate_build,
    component_candidate_build_execution, component_candidate_review, draft_purge_receipt,
    draft_retention_receipt, draft_review, draft_revision, draft_test_run, draft_tombstone,
    release_import,
};

#[derive(Clone)]
pub struct SeaOrmStorage {
    pub(crate) db: DatabaseConnection,
    pub(crate) tenant_id: Option<Uuid>,
}

impl SeaOrmStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            tenant_id: None,
        }
    }

    pub fn with_tenant(db: DatabaseConnection, tenant_id: Uuid) -> Self {
        Self {
            db,
            tenant_id: Some(tenant_id),
        }
    }

    pub fn for_tenant(&self, tenant_id: Uuid) -> Self {
        Self {
            db: self.db.clone(),
            tenant_id: Some(tenant_id),
        }
    }

    pub fn source_digest(workspace: &RhaiWorkspace) -> ScriptResult<String> {
        mapping::source_digest(workspace)
    }

    fn scoped_by_id(&self, id: ScriptId) -> sea_orm::Select<Entity> {
        let select = Entity::find_by_id(id);
        if let Some(tenant_id) = self.tenant_id {
            select.filter(Column::TenantId.eq(tenant_id))
        } else {
            select
        }
    }

    fn ensure_script_scope(&self, script: &Script) -> ScriptResult<()> {
        if self.tenant_id.is_some_and(|t| script.tenant_id != t) {
            return Err(ScriptError::NotFound {
                name: script.id.to_string(),
            });
        }
        Ok(())
    }

    async fn save_impl(&self, mut script: Script) -> ScriptResult<Script> {
        self.ensure_script_scope(&script)?;
        script.workspace.validate().map_err(ScriptError::from)?;
        script
            .source_provenance
            .validate()
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        mapping::validate_parent_release(&script.parent_release)?;
        let now = Utc::now();
        let (trigger_type, trigger_config) = mapping::trigger_to_parts(&script.trigger);
        let permissions_json = mapping::permissions_to_json(&script.permissions);

        if let Some(existing) = self
            .scoped_by_id(script.id)
            .one(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?
        {
            let previous = mapping::model_to_script(existing.clone())?;
            if previous.parent_release != script.parent_release {
                return Err(ScriptError::InvalidLineage(
                    "a draft cannot replace or remove its imported parent release".to_string(),
                ));
            }
            let expected_revision =
                i32::try_from(script.version).map_err(|_| ScriptError::RevisionConflict {
                    expected: script.version,
                })?;
            if expected_revision <= 0 || existing.version != expected_revision {
                return Err(ScriptError::RevisionConflict {
                    expected: script.version,
                });
            }
            let next_revision = expected_revision
                .checked_add(1)
                .ok_or_else(|| ScriptError::Storage("script version overflow".into()))?;
            script.version = next_revision as u32;
            script.updated_at = now;

            let transaction = self
                .db
                .begin()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            let mut update = Entity::update_many()
                .col_expr(Column::Name, Expr::value(script.name.clone()))
                .col_expr(Column::Description, Expr::value(script.description.clone()))
                .col_expr(
                    Column::Workspace,
                    Expr::value(mapping::workspace_to_json(&script.workspace)?),
                )
                .col_expr(Column::TriggerType, Expr::value(trigger_type))
                .col_expr(Column::TriggerConfig, Expr::value(trigger_config))
                .col_expr(Column::Status, Expr::value(script.status.as_str()))
                .col_expr(Column::Version, Expr::value(next_revision))
                .col_expr(Column::RunAsSystem, Expr::value(script.run_as_system))
                .col_expr(Column::Permissions, Expr::value(permissions_json))
                .col_expr(Column::AuthorId, Expr::value(script.author_id.clone()))
                .col_expr(
                    Column::SourceProvenance,
                    Expr::value(mapping::source_provenance_to_json(
                        &script.source_provenance,
                    )?),
                )
                .col_expr(Column::ErrorCount, Expr::value(script.error_count as i32))
                .col_expr(Column::LastErrorAt, Expr::value(script.last_error_at))
                .col_expr(Column::UpdatedAt, Expr::value(script.updated_at))
                .filter(Column::Id.eq(script.id))
                .filter(Column::Version.eq(expected_revision));
            if let Some(tenant_id) = self.tenant_id {
                update = update.filter(Column::TenantId.eq(tenant_id));
            }
            let result = update
                .exec(&transaction)
                .await
                .map_err(|err| ScriptError::Storage(err.to_string()))?;
            if result.rows_affected != 1 {
                return Err(ScriptError::RevisionConflict {
                    expected: script.version.saturating_sub(1),
                });
            }
            Self::ensure_revision_snapshot(&transaction, &previous).await?;
            Self::insert_revision_snapshot(&transaction, &script, Some(expected_revision)).await?;
            transaction
                .commit()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return self.get(script.id).await;
        }

        if draft_tombstone::Entity::find_by_id(script.id)
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .is_some()
        {
            return Err(ScriptError::InvalidLineage(
                "a deleted draft ID cannot be reused while immutable evidence is retained".into(),
            ));
        }

        script.version = 1;
        script.created_at = now;
        script.updated_at = now;

        let model = mapping::new_script_active_model(&script)?;

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        model
            .insert(&transaction)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;
        Self::insert_revision_snapshot(&transaction, &script, None).await?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;

        self.get(script.id).await
    }
}

#[async_trait::async_trait]
impl ScriptRegistry for SeaOrmStorage {
    async fn find(&self, query: ScriptQuery) -> ScriptResult<Vec<Script>> {
        let select = mapping::apply_query(Entity::find(), query, self.tenant_id);
        let models = select
            .order_by_asc(Column::Name)
            .all(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        models.into_iter().map(mapping::model_to_script).collect()
    }

    async fn find_paginated(
        &self,
        query: ScriptQuery,
        offset: u64,
        limit: u64,
    ) -> ScriptResult<ScriptPage> {
        let total = mapping::apply_query(Entity::find(), query.clone(), self.tenant_id)
            .order_by_asc(Column::Name)
            .count(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        let models = mapping::apply_query(Entity::find(), query, self.tenant_id)
            .order_by_asc(Column::Name)
            .offset(offset)
            .limit(limit)
            .all(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        let items: ScriptResult<Vec<Script>> =
            models.into_iter().map(mapping::model_to_script).collect();

        Ok(ScriptPage {
            items: items?,
            total,
        })
    }

    async fn get(&self, id: ScriptId) -> ScriptResult<Script> {
        let model = self
            .scoped_by_id(id)
            .one(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: id.to_string(),
            })?;

        mapping::model_to_script(model)
    }

    async fn get_source_revision(
        &self,
        id: ScriptId,
        revision: u32,
    ) -> ScriptResult<ScriptSourceRevision> {
        self.get_source_revision_impl(id, revision).await
    }

    async fn list_source_revisions(&self, id: ScriptId) -> ScriptResult<Vec<ScriptSourceRevision>> {
        self.list_source_revisions_impl(id).await
    }

    async fn review(&self, command: ReviewCommand) -> ScriptResult<ReviewDecision> {
        self.review_impl(command).await
    }

    async fn list_reviews(&self, id: ScriptId, revision: u32) -> ScriptResult<Vec<ReviewDecision>> {
        self.list_reviews_impl(id, revision).await
    }

    async fn claim_test_run(&self, command: TestCommand) -> ScriptResult<TestRunClaim> {
        self.claim_test_run_impl(command).await
    }

    async fn complete_test_run(
        &self,
        run_id: Uuid,
        lease_token: Uuid,
        completion: TestRunCompletion,
    ) -> ScriptResult<TestRun> {
        self.complete_test_run_impl(run_id, lease_token, completion)
            .await
    }

    async fn get_by_name(&self, name: &str) -> ScriptResult<Script> {
        let mut query = Entity::find().filter(Column::Name.eq(name));
        if let Some(tid) = self.tenant_id {
            query = query.filter(Column::TenantId.eq(tid));
        }
        let model = query
            .one(&self.db)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: name.to_string(),
            })?;

        mapping::model_to_script(model)
    }

    async fn import_published_release(
        &self,
        command: AlloyImportedDraftCommand,
    ) -> ScriptResult<AlloyImportedDraftResult> {
        self.import_published_release_impl(command).await
    }

    async fn create_component_candidate(
        &self,
        command: RustComponentCandidateCommand,
    ) -> ScriptResult<RustComponentCandidate> {
        self.create_component_candidate_impl(command).await
    }

    async fn get_component_candidate(&self, id: Uuid) -> ScriptResult<RustComponentCandidate> {
        self.get_component_candidate_impl(id).await
    }

    async fn review_component_candidate(
        &self,
        command: RustComponentCandidateReviewCommand,
    ) -> ScriptResult<RustComponentCandidateReview> {
        self.review_component_candidate_impl(command).await
    }

    async fn list_component_candidate_reviews(
        &self,
        candidate_id: Uuid,
    ) -> ScriptResult<Vec<RustComponentCandidateReview>> {
        self.list_component_candidate_reviews_impl(candidate_id)
            .await
    }

    async fn record_component_candidate_build(
        &self,
        build: RustComponentCandidateBuild,
    ) -> ScriptResult<RustComponentCandidateBuild> {
        self.record_component_candidate_build_impl(build).await
    }

    async fn get_component_candidate_build(
        &self,
        candidate_id: Uuid,
        idempotency_key: Uuid,
    ) -> ScriptResult<Option<RustComponentCandidateBuild>> {
        self.get_component_candidate_build_impl(candidate_id, idempotency_key)
            .await
    }

    async fn get_component_candidate_build_by_request(
        &self,
        candidate_id: Uuid,
        build_request_id: Uuid,
    ) -> ScriptResult<Option<RustComponentCandidateBuild>> {
        self.get_component_candidate_build_by_request_impl(candidate_id, build_request_id)
            .await
    }

    async fn record_component_candidate_build_execution(
        &self,
        execution: RustComponentCandidateBuildExecution,
    ) -> ScriptResult<RustComponentCandidateBuildExecution> {
        self.record_component_candidate_build_execution_impl(execution)
            .await
    }

    async fn get_component_candidate_build_execution(
        &self,
        candidate_id: Uuid,
        build_request_id: Uuid,
    ) -> ScriptResult<Option<RustComponentCandidateBuildExecution>> {
        self.get_component_candidate_build_execution_impl(candidate_id, build_request_id)
            .await
    }

    async fn save(&self, script: Script) -> ScriptResult<Script> {
        self.save_impl(script).await
    }

    async fn delete(&self, command: ScriptDeletionCommand) -> ScriptResult<()> {
        self.delete_impl(command).await
    }

    async fn get_deleted_evidence_retention(
        &self,
        id: ScriptId,
    ) -> ScriptResult<ScriptEvidenceRetentionState> {
        self.get_deleted_evidence_retention_impl(id).await
    }

    async fn update_deleted_evidence_retention(
        &self,
        command: ScriptEvidenceRetentionCommand,
    ) -> ScriptResult<ScriptEvidenceRetentionState> {
        self.update_deleted_evidence_retention_impl(command).await
    }

    async fn purge_expired_evidence(&self, now: DateTime<Utc>, limit: u16) -> ScriptResult<u64> {
        self.purge_expired_evidence_impl(now, limit).await
    }

    async fn set_status(&self, id: ScriptId, status: ScriptStatus) -> ScriptResult<()> {
        let mut script = self.get(id).await?;
        script.status = status;
        self.save(script).await?;
        Ok(())
    }

    async fn record_error(&self, id: ScriptId) -> ScriptResult<bool> {
        let mut script = self.get(id).await?;
        let should_disable = script.register_error();
        let status = if should_disable {
            ScriptStatus::Disabled
        } else {
            script.status
        };
        script.status = status;
        self.save(script).await?;

        Ok(should_disable)
    }

    async fn reset_errors(&self, id: ScriptId) -> ScriptResult<()> {
        let mut script = self.get(id).await?;
        script.reset_errors();
        self.save(script).await?;
        Ok(())
    }
}
