use chrono::Utc;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ActiveValue, EntityTrait, QueryFilter, QueryOrder, TransactionTrait};

use crate::error::{ScriptError, ScriptResult};
use crate::model::{
    validate_candidate_parent_release, validate_transition, AlloyImportedDraftCommand,
    AlloyImportedDraftResult, ReviewStatus, RustComponentCandidate, RustComponentCandidateBuild,
    RustComponentCandidateBuildError, RustComponentCandidateBuildExecution,
    RustComponentCandidateCommand, RustComponentCandidateError,
    RustComponentCandidateExecutionError, RustComponentCandidateReview,
    RustComponentCandidateReviewCommand,
};
use crate::storage::ScriptRegistry;

use super::entities::{
    component_candidate, component_candidate_build, component_candidate_build_execution,
    component_candidate_review, draft_revision, draft_review, draft_tombstone, release_import,
    Column, Entity,
};
use super::mapping::{
    model_to_component_candidate, model_to_component_candidate_build,
    model_to_component_candidate_build_execution, model_to_component_candidate_review,
    model_to_review_decision, model_to_script, model_to_source_revision, new_script_active_model,
};
use super::SeaOrmStorage;

impl SeaOrmStorage {
    pub(crate) async fn import_published_release_impl(
        &self,
        mut command: AlloyImportedDraftCommand,
    ) -> ScriptResult<AlloyImportedDraftResult> {
        command
            .validate()
            .map_err(|error| ScriptError::InvalidLineage(error.to_string()))?;
        self.ensure_script_scope(&command.script)?;
        let parent_release = command
            .script
            .parent_release
            .clone()
            .ok_or_else(|| {
                ScriptError::InvalidLineage(
                    "validated imported draft must have a parent release".into(),
                )
            })?;
        let now = Utc::now();
        command.script.version = 1;
        command.script.created_at = now;
        command.script.updated_at = now;

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if draft_tombstone::Entity::find_by_id(command.script.id)
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .is_some()
        {
            return Err(ScriptError::InvalidLineage(
                "a deleted draft ID cannot be reused while immutable evidence is retained".into(),
            ));
        }
        let receipt_id = Uuid::new_v4();
        release_import::Entity::insert(release_import::ActiveModel {
            id: ActiveValue::Set(receipt_id),
            tenant_id: ActiveValue::Set(command.script.tenant_id),
            idempotency_key: ActiveValue::Set(command.idempotency_key),
            request_digest: ActiveValue::Set(command.request_digest.clone()),
            script_id: ActiveValue::Set(command.script.id),
            parent_release_slug: ActiveValue::Set(parent_release.slug.clone()),
            parent_release_version: ActiveValue::Set(parent_release.version.clone()),
            parent_release_digest: ActiveValue::Set(parent_release.digest.clone()),
            created_at: ActiveValue::Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                release_import::Column::TenantId,
                release_import::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;

        let receipt = release_import::Entity::find()
            .filter(release_import::Column::TenantId.eq(command.script.tenant_id))
            .filter(release_import::Column::IdempotencyKey.eq(command.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| {
                ScriptError::Storage(
                    "Alloy release import admission completed without a receipt".to_string(),
                )
            })?;

        if receipt.id != receipt_id {
            if receipt.request_digest != command.request_digest
                || receipt.parent_release_slug != parent_release.slug
                || receipt.parent_release_version != parent_release.version
                || receipt.parent_release_digest != parent_release.digest
            {
                transaction
                    .rollback()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Err(ScriptError::ImportIdempotencyConflict);
            }
            let model = Entity::find_by_id(receipt.script_id)
                .filter(Column::TenantId.eq(command.script.tenant_id))
                .one(&transaction)
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?
                .ok_or_else(|| {
                    ScriptError::Storage(
                        "Alloy release import receipt references a missing draft".to_string(),
                    )
                })?;
            let script = model_to_script(model)?;
            transaction
                .commit()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Ok(AlloyImportedDraftResult {
                script,
                created: false,
            });
        }

        Entity::insert(new_script_active_model(&command.script)?)
            .on_conflict(
                OnConflict::columns([Column::TenantId, Column::Name])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let inserted = Entity::find_by_id(command.script.id)
            .filter(Column::TenantId.eq(command.script.tenant_id))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if inserted.is_none() {
            transaction
                .rollback()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Err(ScriptError::ImportDraftNameConflict);
        }
        Self::insert_revision_snapshot(&transaction, &command.script, None).await?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(AlloyImportedDraftResult {
            script: command.script,
            created: true,
        })
    }

    pub(crate) async fn create_component_candidate_impl(
        &self,
        command: RustComponentCandidateCommand,
    ) -> ScriptResult<RustComponentCandidate> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let source_digest = command
            .workspace
            .source_digest()
            .map_err(RustComponentCandidateError::Workspace)?;
        let scenario = command
            .workspace
            .scenario()
            .map_err(RustComponentCandidateError::Workspace)?;
        let scenario_digest = scenario.canonical_digest().map_err(|error| {
            ScriptError::Storage(format!("candidate scenario digest is unavailable: {error}"))
        })?;
        let candidate_id = Uuid::new_v4();
        let now = Utc::now();
        let revision = i32::try_from(command.expected_revision).map_err(|_| {
            ScriptError::RevisionConflict {
                expected: command.expected_revision,
            }
        })?;
        let workspace_json = serde_json::to_value(&command.workspace).map_err(|error| {
            ScriptError::Storage(format!(
                "candidate workspace cannot be stored as durable JSON: {error}"
            ))
        })?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let mut script_query = Entity::find_by_id(command.script_id);
        if let Some(tenant_id) = self.tenant_id {
            script_query = script_query.filter(Column::TenantId.eq(tenant_id));
        }
        let script = script_query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_script)
            .transpose()?
            .ok_or_else(|| ScriptError::NotFound {
                name: command.script_id.to_string(),
            })?;
        if script.version != command.expected_revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }
        let parent_release = script
            .parent_release
            .clone()
            .ok_or(RustComponentCandidateError::ParentReleaseMissing)?;
        validate_candidate_parent_release(&command.workspace, &parent_release)?;
        let parent_source = draft_revision::Entity::find()
            .filter(draft_revision::Column::ScriptId.eq(script.id))
            .filter(draft_revision::Column::TenantId.eq(script.tenant_id))
            .filter(draft_revision::Column::Revision.eq(revision))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_source_revision)
            .transpose()?
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{}@{}", script.id, command.expected_revision),
            })?;
        if parent_source.parent_release.as_ref() != Some(&parent_release) {
            return Err(ScriptError::Storage(
                "component candidate parent source release does not match the draft release".into(),
            ));
        }
        let latest_review = draft_review::Entity::find()
            .filter(draft_review::Column::ScriptId.eq(script.id))
            .filter(draft_review::Column::TenantId.eq(script.tenant_id))
            .filter(draft_review::Column::Revision.eq(revision))
            .order_by_desc(draft_review::Column::CreatedAt)
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_review_decision)
            .transpose()?;
        if !latest_review.is_some_and(|review| {
            review.status == ReviewStatus::Approved
                && review.source_digest == parent_source.source_digest
        }) {
            return Err(RustComponentCandidateError::ParentReviewNotApproved.into());
        }
        component_candidate::Entity::insert(component_candidate::ActiveModel {
            id: ActiveValue::Set(candidate_id),
            tenant_id: ActiveValue::Set(script.tenant_id),
            idempotency_key: ActiveValue::Set(command.idempotency_key),
            request_digest: ActiveValue::Set(request_digest.clone()),
            script_id: ActiveValue::Set(command.script_id),
            parent_revision: ActiveValue::Set(revision),
            parent_source_digest: ActiveValue::Set(parent_source.source_digest.clone()),
            parent_release_slug: ActiveValue::Set(parent_release.slug.clone()),
            parent_release_version: ActiveValue::Set(parent_release.version.clone()),
            parent_release_digest: ActiveValue::Set(parent_release.digest.clone()),
            workspace: ActiveValue::Set(workspace_json),
            source_digest: ActiveValue::Set(source_digest.clone()),
            scenario_digest: ActiveValue::Set(scenario_digest.clone()),
            actor_id: ActiveValue::Set(command.actor_id.clone()),
            created_at: ActiveValue::Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                component_candidate::Column::TenantId,
                component_candidate::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let stored = component_candidate::Entity::find()
            .filter(component_candidate::Column::TenantId.eq(script.tenant_id))
            .filter(component_candidate::Column::IdempotencyKey.eq(command.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| {
                ScriptError::Storage(
                    "component candidate admission completed without a durable record".to_string(),
                )
            })?;
        let candidate = model_to_component_candidate(stored)?;
        if candidate.request_digest != request_digest
            || candidate.script_id != command.script_id
            || candidate.parent_revision != command.expected_revision
            || candidate.parent_release != parent_release
            || candidate.parent_source_digest != parent_source.source_digest
            || candidate.source_digest != source_digest
            || candidate.scenario_digest != scenario_digest
        {
            transaction
                .rollback()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Err(RustComponentCandidateError::IdempotencyConflict.into());
        }
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(candidate)
    }

    pub(crate) async fn get_component_candidate_impl(
        &self,
        id: Uuid,
    ) -> ScriptResult<RustComponentCandidate> {
        let mut query = component_candidate::Entity::find_by_id(id);
        if let Some(tenant_id) = self.tenant_id {
            query = query.filter(component_candidate::Column::TenantId.eq(tenant_id));
        }
        let candidate = query
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate)
            .transpose()?
            .ok_or_else(|| ScriptError::NotFound {
                name: id.to_string(),
            })?;
        let owner = self.get(candidate.script_id).await?;
        (owner.tenant_id == candidate.tenant_id)
            .then_some(candidate)
            .ok_or_else(|| {
                ScriptError::Storage("component candidate tenant lineage is invalid".to_string())
            })
    }

    pub(crate) async fn review_component_candidate_impl(
        &self,
        command: RustComponentCandidateReviewCommand,
    ) -> ScriptResult<RustComponentCandidateReview> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let mut candidate_query = component_candidate::Entity::find_by_id(command.candidate_id);
        if let Some(tenant_id) = self.tenant_id {
            candidate_query =
                candidate_query.filter(component_candidate::Column::TenantId.eq(tenant_id));
        }
        let candidate = candidate_query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate)
            .transpose()?
            .ok_or_else(|| ScriptError::NotFound {
                name: command.candidate_id.to_string(),
            })?;
        Entity::find_by_id(candidate.script_id)
            .filter(Column::TenantId.eq(candidate.tenant_id))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: command.candidate_id.to_string(),
            })?;
        let existing = component_candidate_review::Entity::find()
            .filter(component_candidate_review::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_review::Column::IdempotencyKey.eq(command.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if let Some(existing) = existing {
            let review = model_to_component_candidate_review(existing)?;
            if review.request_digest == request_digest
                && review.tenant_id == candidate.tenant_id
                && review.source_digest == candidate.source_digest
                && review.scenario_digest == candidate.scenario_digest
            {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Ok(review);
            }
            return Err(crate::model::ReviewError::IdempotencyConflict.into());
        }
        let current = component_candidate_review::Entity::find()
            .filter(component_candidate_review::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_review::Column::TenantId.eq(candidate.tenant_id))
            .order_by_desc(component_candidate_review::Column::CreatedAt)
            .order_by_desc(component_candidate_review::Column::Id)
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate_review)
            .transpose()?;
        validate_transition(current.map(|review| review.status), command.status)?;
        let review_id = Uuid::new_v4();
        let now = Utc::now();
        component_candidate_review::Entity::insert(component_candidate_review::ActiveModel {
            id: ActiveValue::Set(review_id),
            candidate_id: ActiveValue::Set(candidate.id),
            tenant_id: ActiveValue::Set(candidate.tenant_id),
            source_digest: ActiveValue::Set(candidate.source_digest.clone()),
            scenario_digest: ActiveValue::Set(candidate.scenario_digest.clone()),
            status: ActiveValue::Set(command.status.as_str().to_string()),
            policy_revision: ActiveValue::Set(command.policy_revision.clone()),
            actor_id: ActiveValue::Set(command.actor_id.clone()),
            reason: ActiveValue::Set(command.reason.clone()),
            idempotency_key: ActiveValue::Set(command.idempotency_key),
            request_digest: ActiveValue::Set(request_digest.clone()),
            created_at: ActiveValue::Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                component_candidate_review::Column::CandidateId,
                component_candidate_review::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let stored = component_candidate_review::Entity::find()
            .filter(component_candidate_review::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_review::Column::IdempotencyKey.eq(command.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| {
                ScriptError::Storage(
                    "component candidate review admission completed without a durable decision"
                        .to_string(),
                )
            })?;
        let review = model_to_component_candidate_review(stored)?;
        if review.request_digest != request_digest
            || review.tenant_id != candidate.tenant_id
            || review.source_digest != candidate.source_digest
            || review.scenario_digest != candidate.scenario_digest
        {
            transaction
                .rollback()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Err(crate::model::ReviewError::IdempotencyConflict.into());
        }
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(review)
    }

    pub(crate) async fn list_component_candidate_reviews_impl(
        &self,
        candidate_id: Uuid,
    ) -> ScriptResult<Vec<RustComponentCandidateReview>> {
        let candidate = self.get_component_candidate_impl(candidate_id).await?;
        component_candidate_review::Entity::find()
            .filter(component_candidate_review::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_review::Column::TenantId.eq(candidate.tenant_id))
            .order_by_asc(component_candidate_review::Column::CreatedAt)
            .order_by_asc(component_candidate_review::Column::Id)
            .all(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .into_iter()
            .map(model_to_component_candidate_review)
            .collect()
    }

    pub(crate) async fn record_component_candidate_build_impl(
        &self,
        build: RustComponentCandidateBuild,
    ) -> ScriptResult<RustComponentCandidateBuild> {
        let candidate = self.get_component_candidate_impl(build.candidate_id).await?;
        build.validate_against(&candidate)?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let script_exists = Entity::find_by_id(candidate.script_id)
            .filter(Column::TenantId.eq(candidate.tenant_id))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .is_some();
        if !script_exists {
            return Err(ScriptError::NotFound {
                name: candidate.script_id.to_string(),
            });
        }
        let latest_review = component_candidate_review::Entity::find()
            .filter(component_candidate_review::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_review::Column::TenantId.eq(candidate.tenant_id))
            .order_by_desc(component_candidate_review::Column::CreatedAt)
            .order_by_desc(component_candidate_review::Column::Id)
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate_review)
            .transpose()?;
        if !latest_review.is_some_and(|review| {
            review.status == ReviewStatus::Approved
                && review.source_digest == candidate.source_digest
                && review.scenario_digest == candidate.scenario_digest
        }) {
            return Err(RustComponentCandidateBuildError::CandidateNotApproved.into());
        }
        component_candidate_build::Entity::insert(component_candidate_build::ActiveModel {
            id: ActiveValue::Set(build.id),
            candidate_id: ActiveValue::Set(candidate.id),
            tenant_id: ActiveValue::Set(candidate.tenant_id),
            candidate_source_digest: ActiveValue::Set(build.candidate_source_digest.clone()),
            scenario_digest: ActiveValue::Set(build.scenario_digest.clone()),
            archive_source_digest: ActiveValue::Set(build.archive_source_digest.clone()),
            build_request_id: ActiveValue::Set(build.build_request_id),
            source_reference: ActiveValue::Set(build.source_reference.clone()),
            actor_id: ActiveValue::Set(build.actor_id),
            idempotency_key: ActiveValue::Set(build.idempotency_key),
            request_digest: ActiveValue::Set(build.request_digest.clone()),
            created_at: ActiveValue::Set(build.created_at),
        })
        .on_conflict(
            OnConflict::columns([
                component_candidate_build::Column::CandidateId,
                component_candidate_build::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let stored = component_candidate_build::Entity::find()
            .filter(component_candidate_build::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_build::Column::IdempotencyKey.eq(build.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate_build)
            .transpose()?
            .ok_or_else(|| {
                ScriptError::Storage(
                    "component candidate build admission completed without a durable receipt"
                        .to_string(),
                )
            })?;
        if stored.request_digest != build.request_digest
            || stored.tenant_id != candidate.tenant_id
            || stored.candidate_source_digest != candidate.source_digest
            || stored.scenario_digest != candidate.scenario_digest
            || stored.archive_source_digest != build.archive_source_digest
            || stored.build_request_id != build.build_request_id
            || stored.source_reference != build.source_reference
        {
            transaction
                .rollback()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Err(RustComponentCandidateBuildError::IdempotencyConflict.into());
        }
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(stored)
    }

    pub(crate) async fn get_component_candidate_build_impl(
        &self,
        candidate_id: Uuid,
        idempotency_key: Uuid,
    ) -> ScriptResult<Option<RustComponentCandidateBuild>> {
        let candidate = self.get_component_candidate_impl(candidate_id).await?;
        component_candidate_build::Entity::find()
            .filter(component_candidate_build::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_build::Column::TenantId.eq(candidate.tenant_id))
            .filter(component_candidate_build::Column::IdempotencyKey.eq(idempotency_key))
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate_build)
            .transpose()
    }

    pub(crate) async fn get_component_candidate_build_by_request_impl(
        &self,
        candidate_id: Uuid,
        build_request_id: Uuid,
    ) -> ScriptResult<Option<RustComponentCandidateBuild>> {
        let candidate = self.get_component_candidate_impl(candidate_id).await?;
        component_candidate_build::Entity::find()
            .filter(component_candidate_build::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_build::Column::TenantId.eq(candidate.tenant_id))
            .filter(component_candidate_build::Column::BuildRequestId.eq(build_request_id))
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate_build)
            .transpose()
    }

    pub(crate) async fn record_component_candidate_build_execution_impl(
        &self,
        execution: RustComponentCandidateBuildExecution,
    ) -> ScriptResult<RustComponentCandidateBuildExecution> {
        let candidate = self.get_component_candidate_impl(execution.candidate_id).await?;
        let build = self
            .get_component_candidate_build_by_request_impl(candidate.id, execution.build_request_id)
            .await?
            .ok_or_else(|| ScriptError::NotFound {
                name: execution.build_request_id.to_string(),
            })?;
        execution.validate_against(&candidate, &build)?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let candidate_visible = component_candidate::Entity::find_by_id(candidate.id)
            .filter(component_candidate::Column::TenantId.eq(candidate.tenant_id))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .is_some();
        let build_visible = component_candidate_build::Entity::find_by_id(build.id)
            .filter(component_candidate_build::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_build::Column::TenantId.eq(candidate.tenant_id))
            .filter(component_candidate_build::Column::BuildRequestId.eq(build.build_request_id))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .is_some();
        if !candidate_visible || !build_visible {
            transaction
                .rollback()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Err(ScriptError::NotFound {
                name: execution.build_request_id.to_string(),
            });
        }
        let publication = serde_json::to_value(&execution.publication)
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let scenario_comparison = serde_json::to_value(&execution.scenario_comparison)
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let build_result_revision =
            i64::try_from(execution.build_result_revision).map_err(|_| {
                ScriptError::Storage(
                    "component candidate build execution revision exceeds i64".to_string(),
                )
            })?;
        component_candidate_build_execution::Entity::insert(
            component_candidate_build_execution::ActiveModel {
                candidate_build_id: ActiveValue::Set(build.id),
                candidate_id: ActiveValue::Set(candidate.id),
                tenant_id: ActiveValue::Set(candidate.tenant_id),
                candidate_source_digest: ActiveValue::Set(
                    execution.candidate_source_digest.clone(),
                ),
                scenario_digest: ActiveValue::Set(execution.scenario_digest.clone()),
                archive_source_digest: ActiveValue::Set(execution.archive_source_digest.clone()),
                build_request_id: ActiveValue::Set(execution.build_request_id),
                source_reference: ActiveValue::Set(execution.source_reference.clone()),
                build_result_revision: ActiveValue::Set(build_result_revision),
                component_digest: ActiveValue::Set(execution.component_digest.clone()),
                sbom_digest: ActiveValue::Set(execution.sbom_digest.clone()),
                provenance_digest: ActiveValue::Set(execution.provenance_digest.clone()),
                publication: ActiveValue::Set(publication),
                scenario_comparison: ActiveValue::Set(scenario_comparison),
                created_at: ActiveValue::Set(execution.created_at),
            },
        )
        .on_conflict(
            OnConflict::column(component_candidate_build_execution::Column::CandidateBuildId)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let stored = component_candidate_build_execution::Entity::find_by_id(build.id)
            .filter(component_candidate_build_execution::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_build_execution::Column::TenantId.eq(candidate.tenant_id))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate_build_execution)
            .transpose()?
            .ok_or_else(|| {
                ScriptError::Storage(
                    "component candidate build execution completed without durable evidence"
                        .to_string(),
                )
            })?;
        stored.validate_against(&candidate, &build)?;
        if !stored.matches_evidence(&execution) {
            transaction
                .rollback()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Err(RustComponentCandidateExecutionError::EvidenceConflict.into());
        }
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(stored)
    }

    pub(crate) async fn get_component_candidate_build_execution_impl(
        &self,
        candidate_id: Uuid,
        build_request_id: Uuid,
    ) -> ScriptResult<Option<RustComponentCandidateBuildExecution>> {
        let candidate = self.get_component_candidate_impl(candidate_id).await?;
        let Some(build) = self
            .get_component_candidate_build_by_request_impl(candidate.id, build_request_id)
            .await?
        else {
            return Ok(None);
        };
        let execution = component_candidate_build_execution::Entity::find_by_id(build.id)
            .filter(component_candidate_build_execution::Column::CandidateId.eq(candidate.id))
            .filter(component_candidate_build_execution::Column::TenantId.eq(candidate.tenant_id))
            .filter(
                component_candidate_build_execution::Column::BuildRequestId
                    .eq(build.build_request_id),
            )
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_component_candidate_build_execution)
            .transpose()?;
        if let Some(execution) = &execution {
            execution.validate_against(&candidate, &build)?;
        }
        Ok(execution)
    }
}
