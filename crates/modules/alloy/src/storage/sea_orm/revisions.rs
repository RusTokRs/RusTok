use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, EntityTrait, QueryFilter, QueryOrder, TransactionTrait,
};

use crate::error::{ScriptError, ScriptResult};
use crate::model::{
    validate_transition, ReviewCommand, ReviewDecision, Script, ScriptId, ScriptSourceRevision,
    TestCommand, TestRun, TestRunClaim, TestRunCompletion, TestRunLease, TestRunStatus,
};
use crate::storage::ScriptRegistry;

use super::entities::{draft_review, draft_revision, draft_test_run, Column, Entity};
use super::mapping::{
    model_to_review_decision, model_to_source_revision, model_to_test_run, source_digest,
    source_provenance_to_json, workspace_to_json,
};
use super::SeaOrmStorage;

impl SeaOrmStorage {
    pub(crate) async fn source_for_test_run(
        transaction: &sea_orm::DatabaseTransaction,
        script_id: ScriptId,
        tenant_id: Uuid,
        revision: i32,
        test_path: &str,
    ) -> ScriptResult<ScriptSourceRevision> {
        let model = draft_revision::Entity::find()
            .filter(draft_revision::Column::ScriptId.eq(script_id))
            .filter(draft_revision::Column::TenantId.eq(tenant_id))
            .filter(draft_revision::Column::Revision.eq(revision))
            .one(transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{script_id}@{revision}"),
            })?;
        let source = model_to_source_revision(model)?;
        source.workspace.validate_rhai_test(test_path)?;
        Ok(source)
    }

    pub(crate) async fn insert_revision_snapshot(
        transaction: &sea_orm::DatabaseTransaction,
        script: &Script,
        parent_revision: Option<i32>,
    ) -> ScriptResult<()> {
        let revision = i32::try_from(script.version).map_err(|_| {
            ScriptError::Storage("script revision exceeds the durable revision range".into())
        })?;
        draft_revision::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            script_id: ActiveValue::Set(script.id),
            tenant_id: ActiveValue::Set(script.tenant_id),
            revision: ActiveValue::Set(revision),
            parent_revision: ActiveValue::Set(parent_revision),
            source_digest: ActiveValue::Set(source_digest(&script.workspace)?),
            workspace: ActiveValue::Set(workspace_to_json(&script.workspace)?),
            author_id: ActiveValue::Set(script.author_id.clone()),
            source_provenance: ActiveValue::Set(source_provenance_to_json(
                &script.source_provenance,
            )?),
            parent_release_slug: ActiveValue::Set(
                script
                    .parent_release
                    .as_ref()
                    .map(|release| release.slug.clone()),
            ),
            parent_release_version: ActiveValue::Set(
                script
                    .parent_release
                    .as_ref()
                    .map(|release| release.version.clone()),
            ),
            parent_release_digest: ActiveValue::Set(
                script
                    .parent_release
                    .as_ref()
                    .map(|release| release.digest.clone()),
            ),
            created_at: ActiveValue::Set(script.updated_at),
        }
        .insert(transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(())
    }

    pub(crate) async fn ensure_revision_snapshot(
        transaction: &sea_orm::DatabaseTransaction,
        script: &Script,
    ) -> ScriptResult<()> {
        let revision = i32::try_from(script.version).map_err(|_| {
            ScriptError::Storage("script revision exceeds the durable revision range".into())
        })?;
        let existing = draft_revision::Entity::find()
            .filter(draft_revision::Column::ScriptId.eq(script.id))
            .filter(draft_revision::Column::Revision.eq(revision))
            .one(transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if existing.is_none() {
            Self::insert_revision_snapshot(
                transaction,
                script,
                revision.checked_sub(1).filter(|parent| *parent > 0),
            )
            .await?;
        }
        Ok(())
    }

    pub(crate) async fn get_source_revision_impl(
        &self,
        id: ScriptId,
        revision: u32,
    ) -> ScriptResult<ScriptSourceRevision> {
        let revision = i32::try_from(revision).map_err(|_| {
            ScriptError::Storage("requested source revision is outside the durable range".into())
        })?;
        let mut query = draft_revision::Entity::find()
            .inner_join(Entity)
            .filter(draft_revision::Column::ScriptId.eq(id))
            .filter(Column::Id.eq(id))
            .filter(draft_revision::Column::Revision.eq(revision));
        if let Some(tenant_id) = self.tenant_id {
            query = query
                .filter(draft_revision::Column::TenantId.eq(tenant_id))
                .filter(Column::TenantId.eq(tenant_id));
        }
        let model = query
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{id}@{revision}"),
            })?;

        model_to_source_revision(model)
    }

    pub(crate) async fn list_source_revisions_impl(
        &self,
        id: ScriptId,
    ) -> ScriptResult<Vec<ScriptSourceRevision>> {
        let mut query = draft_revision::Entity::find()
            .inner_join(Entity)
            .filter(draft_revision::Column::ScriptId.eq(id))
            .filter(Column::Id.eq(id));
        if let Some(tenant_id) = self.tenant_id {
            query = query
                .filter(draft_revision::Column::TenantId.eq(tenant_id))
                .filter(Column::TenantId.eq(tenant_id));
        }
        let models = query
            .order_by_asc(draft_revision::Column::Revision)
            .all(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;

        if models.is_empty() {
            self.get(id).await?;
        }

        models.into_iter().map(model_to_source_revision).collect()
    }

    pub(crate) async fn review_impl(&self, command: ReviewCommand) -> ScriptResult<ReviewDecision> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let revision = i32::try_from(command.expected_revision).map_err(|_| {
            ScriptError::RevisionConflict {
                expected: command.expected_revision,
            }
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
        let script_model = script_query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: command.script_id.to_string(),
            })?;
        if script_model.version != revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }

        let source_revision = draft_revision::Entity::find()
            .filter(draft_revision::Column::ScriptId.eq(command.script_id))
            .filter(draft_revision::Column::Revision.eq(revision))
            .filter(draft_revision::Column::TenantId.eq(script_model.tenant_id))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: format!("{}@{}", command.script_id, command.expected_revision),
            })?;

        #[allow(clippy::useless_conversion)]
        let mut assert_current = Entity::update_many()
            .col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt).into())
            .filter(Column::Id.eq(command.script_id))
            .filter(Column::Version.eq(revision));
        if let Some(tenant_id) = self.tenant_id {
            assert_current = assert_current.filter(Column::TenantId.eq(tenant_id));
        }
        let asserted = assert_current
            .exec(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if asserted.rows_affected != 1 {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }

        let existing = draft_review::Entity::find()
            .filter(draft_review::Column::ScriptId.eq(command.script_id))
            .filter(draft_review::Column::Revision.eq(revision))
            .filter(draft_review::Column::TenantId.eq(script_model.tenant_id))
            .filter(draft_review::Column::IdempotencyKey.eq(command.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if let Some(existing) = existing {
            let existing = model_to_review_decision(existing)?;
            if existing.request_digest == request_digest {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Ok(existing);
            }
            return Err(crate::model::ReviewError::IdempotencyConflict.into());
        }

        let current = draft_review::Entity::find()
            .filter(draft_review::Column::ScriptId.eq(command.script_id))
            .filter(draft_review::Column::Revision.eq(revision))
            .filter(draft_review::Column::TenantId.eq(script_model.tenant_id))
            .order_by_desc(draft_review::Column::CreatedAt)
            .order_by_desc(draft_review::Column::Id)
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .map(model_to_review_decision)
            .transpose()?;
        validate_transition(current.map(|decision| decision.status), command.status)?;

        let decision = ReviewDecision {
            id: Uuid::new_v4(),
            script_id: command.script_id,
            tenant_id: script_model.tenant_id,
            revision: command.expected_revision,
            source_digest: source_revision.source_digest,
            status: command.status,
            policy_revision: command.policy_revision,
            actor_id: command.actor_id,
            reason: command.reason,
            idempotency_key: command.idempotency_key,
            request_digest,
            created_at: Utc::now(),
        };
        draft_review::ActiveModel {
            id: ActiveValue::Set(decision.id),
            script_id: ActiveValue::Set(decision.script_id),
            tenant_id: ActiveValue::Set(decision.tenant_id),
            revision: ActiveValue::Set(revision),
            source_digest: ActiveValue::Set(decision.source_digest.clone()),
            status: ActiveValue::Set(decision.status.as_str().to_string()),
            policy_revision: ActiveValue::Set(decision.policy_revision.clone()),
            actor_id: ActiveValue::Set(decision.actor_id.clone()),
            reason: ActiveValue::Set(decision.reason.clone()),
            idempotency_key: ActiveValue::Set(decision.idempotency_key),
            request_digest: ActiveValue::Set(decision.request_digest.clone()),
            created_at: ActiveValue::Set(decision.created_at),
        }
        .insert(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(decision)
    }

    pub(crate) async fn list_reviews_impl(
        &self,
        id: ScriptId,
        revision: u32,
    ) -> ScriptResult<Vec<ReviewDecision>> {
        let revision = i32::try_from(revision).map_err(|_| {
            ScriptError::Storage("requested review revision is outside the durable range".into())
        })?;
        let mut query = draft_review::Entity::find()
            .inner_join(Entity)
            .filter(draft_review::Column::ScriptId.eq(id))
            .filter(Column::Id.eq(id))
            .filter(draft_review::Column::Revision.eq(revision));
        if let Some(tenant_id) = self.tenant_id {
            query = query
                .filter(draft_review::Column::TenantId.eq(tenant_id))
                .filter(Column::TenantId.eq(tenant_id));
        }
        let models = query
            .order_by_asc(draft_review::Column::CreatedAt)
            .order_by_asc(draft_review::Column::Id)
            .all(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if models.is_empty() {
            self.get(id).await?;
        }
        models.into_iter().map(model_to_review_decision).collect()
    }

    pub(crate) async fn claim_test_run_impl(
        &self,
        command: TestCommand,
    ) -> ScriptResult<TestRunClaim> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let revision = i32::try_from(command.expected_revision).map_err(|_| {
            ScriptError::RevisionConflict {
                expected: command.expected_revision,
            }
        })?;
        let now = Utc::now();
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
            .ok_or_else(|| ScriptError::NotFound {
                name: command.script_id.to_string(),
            })?;
        if script.version != revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }

        #[allow(clippy::useless_conversion)]
        let mut assert_current = Entity::update_many()
            .col_expr(Column::UpdatedAt, Expr::col(Column::UpdatedAt).into())
            .filter(Column::Id.eq(command.script_id))
            .filter(Column::Version.eq(revision));
        if let Some(tenant_id) = self.tenant_id {
            assert_current = assert_current.filter(Column::TenantId.eq(tenant_id));
        }
        if assert_current
            .exec(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .rows_affected
            != 1
        {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }
        let mut existing_query = draft_test_run::Entity::find()
            .filter(draft_test_run::Column::ScriptId.eq(command.script_id))
            .filter(draft_test_run::Column::Revision.eq(revision))
            .filter(draft_test_run::Column::IdempotencyKey.eq(command.idempotency_key));
        if let Some(tenant_id) = self.tenant_id {
            existing_query = existing_query.filter(draft_test_run::Column::TenantId.eq(tenant_id));
        }
        if let Some(existing) = existing_query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
        {
            let existing_run = model_to_test_run(existing.clone())?;
            if existing_run.request_digest != request_digest {
                return Err(crate::model::TestRunError::IdempotencyConflict.into());
            }
            if existing_run.status.is_terminal() {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Ok(TestRunClaim::Replay(existing_run));
            }
            if existing
                .lease_expires_at
                .is_some_and(|expires_at| expires_at > now)
            {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                return Ok(TestRunClaim::InProgress(existing_run));
            }
            let lease_token = Uuid::new_v4();
            let recovered = draft_test_run::Entity::update_many()
                .col_expr(
                    draft_test_run::Column::LeaseToken,
                    Expr::value(Some(lease_token)),
                )
                .col_expr(
                    draft_test_run::Column::LeaseExpiresAt,
                    Expr::value(Some(crate::model::test_run_lease_expires_at(now))),
                )
                .filter(draft_test_run::Column::Id.eq(existing.id))
                .filter(draft_test_run::Column::Status.eq(TestRunStatus::Pending.as_str()))
                .filter(draft_test_run::Column::LeaseExpiresAt.lte(now))
                .exec(&transaction)
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            if recovered.rows_affected != 1 {
                return Err(crate::model::TestRunError::LeaseLost.into());
            }
            let source = Self::source_for_test_run(
                &transaction,
                command.script_id,
                existing.tenant_id,
                revision,
                &command.test_path,
            )
            .await?;
            if source.source_digest != existing_run.source_digest {
                return Err(ScriptError::Storage(
                    "test run source digest does not match its immutable revision".into(),
                ));
            }
            transaction
                .commit()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            return Ok(TestRunClaim::Claimed(TestRunLease {
                run: existing_run,
                lease_token,
                source,
            }));
        }

        let source = Self::source_for_test_run(
            &transaction,
            command.script_id,
            script.tenant_id,
            revision,
            &command.test_path,
        )
        .await?;

        let existing = draft_test_run::Entity::find()
            .filter(draft_test_run::Column::ScriptId.eq(command.script_id))
            .filter(draft_test_run::Column::TenantId.eq(script.tenant_id))
            .filter(draft_test_run::Column::Revision.eq(revision))
            .filter(draft_test_run::Column::IdempotencyKey.eq(command.idempotency_key))
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if existing.is_some() {
            return Err(crate::model::TestRunError::LeaseLost.into());
        }

        let lease_token = Uuid::new_v4();
        let run = TestRun {
            id: Uuid::new_v4(),
            script_id: command.script_id,
            tenant_id: script.tenant_id,
            revision: command.expected_revision,
            source_digest: source.source_digest.clone(),
            test_path: command.test_path,
            actor_id: command.actor_id,
            idempotency_key: command.idempotency_key,
            request_digest,
            status: TestRunStatus::Pending,
            passed: None,
            error: None,
            created_at: now,
            completed_at: None,
        };
        draft_test_run::ActiveModel {
            id: ActiveValue::Set(run.id),
            script_id: ActiveValue::Set(run.script_id),
            tenant_id: ActiveValue::Set(run.tenant_id),
            revision: ActiveValue::Set(revision),
            source_digest: ActiveValue::Set(run.source_digest.clone()),
            test_path: ActiveValue::Set(run.test_path.clone()),
            actor_id: ActiveValue::Set(run.actor_id.clone()),
            idempotency_key: ActiveValue::Set(run.idempotency_key),
            request_digest: ActiveValue::Set(run.request_digest.clone()),
            status: ActiveValue::Set(TestRunStatus::Pending.as_str().to_string()),
            passed: ActiveValue::Set(None),
            error: ActiveValue::Set(None),
            lease_token: ActiveValue::Set(Some(lease_token)),
            lease_expires_at: ActiveValue::Set(Some(crate::model::test_run_lease_expires_at(now))),
            created_at: ActiveValue::Set(now),
            completed_at: ActiveValue::Set(None),
        }
        .insert(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(TestRunClaim::Claimed(TestRunLease {
            run,
            lease_token,
            source,
        }))
    }

    pub(crate) async fn complete_test_run_impl(
        &self,
        run_id: Uuid,
        lease_token: Uuid,
        completion: TestRunCompletion,
    ) -> ScriptResult<TestRun> {
        completion.validate()?;
        let now = Utc::now();
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let mut query = draft_test_run::Entity::find_by_id(run_id);
        if let Some(tenant_id) = self.tenant_id {
            query = query.filter(draft_test_run::Column::TenantId.eq(tenant_id));
        }
        let model = query
            .one(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?
            .ok_or_else(|| ScriptError::NotFound {
                name: run_id.to_string(),
            })?;
        let existing = model_to_test_run(model.clone())?;
        if existing.status.is_terminal() {
            transaction
                .commit()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            self.get(existing.script_id).await?;
            return Ok(existing);
        }
        if model.lease_token != Some(lease_token)
            || model
                .lease_expires_at
                .is_none_or(|expires_at| expires_at <= now)
        {
            return Err(crate::model::TestRunError::LeaseLost.into());
        }
        let status = if completion.passed {
            TestRunStatus::Passed
        } else {
            TestRunStatus::Failed
        };
        let updated = draft_test_run::Entity::update_many()
            .col_expr(draft_test_run::Column::Status, Expr::value(status.as_str()))
            .col_expr(
                draft_test_run::Column::Passed,
                Expr::value(Some(completion.passed)),
            )
            .col_expr(
                draft_test_run::Column::Error,
                Expr::value(completion.error.clone()),
            )
            .col_expr(
                draft_test_run::Column::LeaseToken,
                Expr::value(None::<Uuid>),
            )
            .col_expr(
                draft_test_run::Column::LeaseExpiresAt,
                Expr::value(None::<DateTime<Utc>>),
            )
            .col_expr(draft_test_run::Column::CompletedAt, Expr::value(Some(now)))
            .filter(draft_test_run::Column::Id.eq(run_id))
            .filter(draft_test_run::Column::Status.eq(TestRunStatus::Pending.as_str()))
            .filter(draft_test_run::Column::LeaseToken.eq(lease_token))
            .filter(draft_test_run::Column::LeaseExpiresAt.gt(now))
            .exec(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if updated.rows_affected != 1 {
            return Err(crate::model::TestRunError::LeaseLost.into());
        }
        let run = TestRun {
            status,
            passed: Some(completion.passed),
            error: completion.error,
            completed_at: Some(now),
            ..existing
        };
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        self.get(run.script_id).await?;
        Ok(run)
    }
}
