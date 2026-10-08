use chrono::{DateTime, Utc};
use rustok_core::RetentionPolicy;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{ActiveValue, EntityTrait, QueryFilter, QueryOrder, QuerySelect, TransactionTrait};

use crate::error::{ScriptError, ScriptResult};
use crate::model::{
    ScriptDeletionCommand, ScriptEvidenceRetentionCommand, ScriptEvidenceRetentionError,
    ScriptEvidenceRetentionState, ScriptId, deleted_evidence_retention,
};
use crate::storage::ScriptRegistry;

use super::SeaOrmStorage;
use super::entities::{
    Column, Entity, component_candidate, component_candidate_build,
    component_candidate_build_execution, component_candidate_review, draft_purge_receipt,
    draft_retention_receipt, draft_review, draft_revision, draft_test_run, draft_tombstone,
};
use super::mapping::{
    replay_deleted_command, retention_state_from_receipt, retention_state_from_tombstone,
};

impl SeaOrmStorage {
    pub(crate) async fn deletion_receipt(
        &self,
        id: ScriptId,
    ) -> ScriptResult<Option<draft_tombstone::Model>> {
        let mut query = draft_tombstone::Entity::find_by_id(id);
        if let Some(tenant_id) = self.tenant_id {
            query = query.filter(draft_tombstone::Column::TenantId.eq(tenant_id));
        }
        query
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))
    }

    pub(crate) async fn retention_receipt(
        &self,
        id: ScriptId,
        deletion_request_digest: &str,
        idempotency_key: Uuid,
    ) -> ScriptResult<Option<draft_retention_receipt::Model>> {
        let mut query = draft_retention_receipt::Entity::find()
            .filter(draft_retention_receipt::Column::ScriptId.eq(id))
            .filter(
                draft_retention_receipt::Column::DeletionRequestDigest.eq(deletion_request_digest),
            )
            .filter(draft_retention_receipt::Column::IdempotencyKey.eq(idempotency_key));
        if let Some(tenant_id) = self.tenant_id {
            query = query.filter(draft_retention_receipt::Column::TenantId.eq(tenant_id));
        }
        query
            .one(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))
    }

    pub(crate) async fn delete_impl(&self, command: ScriptDeletionCommand) -> ScriptResult<()> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let id = command.script_id;
        if let Some(existing) = self.deletion_receipt(id).await? {
            return replay_deleted_command(&existing, &command, &request_digest);
        }
        let current = self.get(id).await?;
        if current.version != command.expected_revision {
            return Err(ScriptError::RevisionConflict {
                expected: command.expected_revision,
            });
        }
        let deleted_at = Utc::now();
        let (retention_policy, retain_until) = deleted_evidence_retention(deleted_at);

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let mut delete = Entity::delete_many().filter(Column::Id.eq(id));
        if let Some(tenant_id) = self.tenant_id {
            delete = delete.filter(Column::TenantId.eq(tenant_id));
        }
        delete = delete.filter(Column::Version.eq(
            i32::try_from(command.expected_revision).map_err(|_| {
                ScriptError::RevisionConflict {
                    expected: command.expected_revision,
                }
            })?,
        ));
        let result = delete
            .exec(&transaction)
            .await
            .map_err(|err| ScriptError::Storage(err.to_string()))?;

        if result.rows_affected == 0 {
            transaction
                .rollback()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            if let Some(existing) = self.deletion_receipt(id).await? {
                return replay_deleted_command(&existing, &command, &request_digest);
            }
            return match self.get(id).await {
                Ok(_current) => Err(ScriptError::RevisionConflict {
                    expected: command.expected_revision,
                }),
                Err(ScriptError::NotFound { .. }) => Err(ScriptError::NotFound {
                    name: id.to_string(),
                }),
                Err(error) => Err(error),
            };
        }

        draft_tombstone::Entity::insert(draft_tombstone::ActiveModel {
            id: ActiveValue::Set(id),
            tenant_id: ActiveValue::Set(current.tenant_id),
            deleted_at: ActiveValue::Set(deleted_at),
            deleted_by: ActiveValue::Set(command.actor_id),
            delete_reason: ActiveValue::Set(command.reason),
            idempotency_key: ActiveValue::Set(command.idempotency_key),
            request_digest: ActiveValue::Set(request_digest),
            retention_policy: ActiveValue::Set(retention_policy.as_str().to_string()),
            retain_until: ActiveValue::Set(Some(retain_until)),
            retention_revision: ActiveValue::Set(1),
        })
        .exec_without_returning(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;

        Ok(())
    }

    pub(crate) async fn get_deleted_evidence_retention_impl(
        &self,
        id: ScriptId,
    ) -> ScriptResult<ScriptEvidenceRetentionState> {
        let tombstone = self
            .deletion_receipt(id)
            .await?
            .ok_or_else(|| ScriptError::NotFound {
                name: id.to_string(),
            })?;
        retention_state_from_tombstone(&tombstone)
    }

    pub(crate) async fn update_deleted_evidence_retention_impl(
        &self,
        command: ScriptEvidenceRetentionCommand,
    ) -> ScriptResult<ScriptEvidenceRetentionState> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let tombstone = self.deletion_receipt(command.script_id).await?;
        let Some(tombstone) = tombstone else {
            return match self
                .retention_receipt(
                    command.script_id,
                    &command.deletion_request_digest,
                    command.idempotency_key,
                )
                .await?
            {
                Some(receipt) if receipt.request_digest == request_digest => {
                    retention_state_from_receipt(&receipt)
                }
                _ => Err(ScriptError::NotFound {
                    name: command.script_id.to_string(),
                }),
            };
        };
        let current = retention_state_from_tombstone(&tombstone)?;
        if current.deletion_request_digest != command.deletion_request_digest {
            return Err(ScriptError::NotFound {
                name: command.script_id.to_string(),
            });
        }
        if let Some(receipt) = self
            .retention_receipt(
                command.script_id,
                &command.deletion_request_digest,
                command.idempotency_key,
            )
            .await?
        {
            return if receipt.request_digest == request_digest {
                retention_state_from_receipt(&receipt)
            } else {
                Err(ScriptEvidenceRetentionError::IdempotencyConflict.into())
            };
        }
        if current.retention_revision != command.expected_retention_revision {
            return Err(ScriptError::RetentionRevisionConflict {
                expected: command.expected_retention_revision,
            });
        }
        let updated = current.transition(command.action, Utc::now())?;
        let expected_revision = i32::try_from(current.retention_revision)
            .map_err(|_| ScriptEvidenceRetentionError::InvalidStoredState)?;
        let updated_revision = i32::try_from(updated.retention_revision)
            .map_err(|_| ScriptEvidenceRetentionError::InvalidStoredState)?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let changed = draft_tombstone::Entity::update_many()
            .col_expr(
                draft_tombstone::Column::RetentionPolicy,
                Expr::value(updated.policy.as_str()),
            )
            .col_expr(
                draft_tombstone::Column::RetainUntil,
                Expr::value(updated.retain_until),
            )
            .col_expr(
                draft_tombstone::Column::RetentionRevision,
                Expr::value(updated_revision),
            )
            .filter(draft_tombstone::Column::Id.eq(command.script_id))
            .filter(draft_tombstone::Column::TenantId.eq(current.tenant_id))
            .filter(draft_tombstone::Column::RetentionPolicy.eq(current.policy.as_str()))
            .filter(draft_tombstone::Column::RetentionRevision.eq(expected_revision))
            .exec(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        if changed.rows_affected != 1 {
            transaction
                .rollback()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            if let Some(receipt) = self
                .retention_receipt(
                    command.script_id,
                    &command.deletion_request_digest,
                    command.idempotency_key,
                )
                .await?
            {
                return if receipt.request_digest == request_digest {
                    retention_state_from_receipt(&receipt)
                } else {
                    Err(ScriptEvidenceRetentionError::IdempotencyConflict.into())
                };
            }
            return Err(ScriptError::RetentionRevisionConflict {
                expected: command.expected_retention_revision,
            });
        }
        draft_retention_receipt::Entity::insert(draft_retention_receipt::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            script_id: ActiveValue::Set(command.script_id),
            tenant_id: ActiveValue::Set(current.tenant_id),
            action: ActiveValue::Set(command.action.as_str().to_string()),
            actor_id: ActiveValue::Set(command.actor_id),
            idempotency_key: ActiveValue::Set(command.idempotency_key),
            request_digest: ActiveValue::Set(request_digest),
            deletion_request_digest: ActiveValue::Set(command.deletion_request_digest),
            retention_policy: ActiveValue::Set(updated.policy.as_str().to_string()),
            retain_until: ActiveValue::Set(updated.retain_until),
            retention_revision: ActiveValue::Set(updated_revision),
            recorded_at: ActiveValue::Set(Utc::now()),
        })
        .exec_without_returning(&transaction)
        .await
        .map_err(|error| ScriptError::Storage(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        Ok(updated)
    }

    pub(crate) async fn purge_expired_evidence_impl(
        &self,
        now: DateTime<Utc>,
        limit: u16,
    ) -> ScriptResult<u64> {
        if self.tenant_id.is_some() {
            return Err(ScriptError::Storage(
                "Alloy evidence retention must run through the unscoped owner storage".into(),
            ));
        }
        if limit == 0 {
            return Ok(0);
        }

        let candidates = draft_tombstone::Entity::find()
            .filter(
                draft_tombstone::Column::RetentionPolicy.eq(RetentionPolicy::RetainUntil.as_str()),
            )
            .filter(draft_tombstone::Column::RetainUntil.lte(now))
            .order_by_asc(draft_tombstone::Column::RetainUntil)
            .order_by_asc(draft_tombstone::Column::Id)
            .limit(u64::from(limit))
            .all(&self.db)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
        let mut purged = 0_u64;

        for tombstone in candidates {
            let retain_until = tombstone.retain_until.ok_or_else(|| {
                ScriptError::Storage("retain_until evidence has no deadline".into())
            })?;
            let transaction = self
                .db
                .begin()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;

            let claimed = draft_tombstone::Entity::delete_many()
                .filter(draft_tombstone::Column::Id.eq(tombstone.id))
                .filter(draft_tombstone::Column::TenantId.eq(tombstone.tenant_id))
                .filter(
                    draft_tombstone::Column::RetentionPolicy
                        .eq(RetentionPolicy::RetainUntil.as_str()),
                )
                .filter(draft_tombstone::Column::RetainUntil.lte(now))
                .exec(&transaction)
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            if claimed.rows_affected != 1 {
                transaction
                    .rollback()
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                continue;
            }

            let source_revision_count = i32::try_from(
                draft_revision::Entity::delete_many()
                    .filter(draft_revision::Column::ScriptId.eq(tombstone.id))
                    .filter(draft_revision::Column::TenantId.eq(tombstone.tenant_id))
                    .exec(&transaction)
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?
                    .rows_affected,
            )
            .map_err(|_| ScriptError::Storage("source revision count exceeds i32".into()))?;
            let review_count = i32::try_from(
                draft_review::Entity::delete_many()
                    .filter(draft_review::Column::ScriptId.eq(tombstone.id))
                    .filter(draft_review::Column::TenantId.eq(tombstone.tenant_id))
                    .exec(&transaction)
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?
                    .rows_affected,
            )
            .map_err(|_| ScriptError::Storage("review count exceeds i32".into()))?;
            let test_run_count = i32::try_from(
                draft_test_run::Entity::delete_many()
                    .filter(draft_test_run::Column::ScriptId.eq(tombstone.id))
                    .filter(draft_test_run::Column::TenantId.eq(tombstone.tenant_id))
                    .exec(&transaction)
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?
                    .rows_affected,
            )
            .map_err(|_| ScriptError::Storage("test run count exceeds i32".into()))?;
            let candidate_ids = component_candidate::Entity::find()
                .filter(component_candidate::Column::ScriptId.eq(tombstone.id))
                .filter(component_candidate::Column::TenantId.eq(tombstone.tenant_id))
                .all(&transaction)
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?
                .into_iter()
                .map(|candidate| candidate.id)
                .collect::<Vec<_>>();
            if !candidate_ids.is_empty() {
                component_candidate_review::Entity::delete_many()
                    .filter(component_candidate_review::Column::TenantId.eq(tombstone.tenant_id))
                    .filter(
                        component_candidate_review::Column::CandidateId
                            .is_in(candidate_ids.clone()),
                    )
                    .exec(&transaction)
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                component_candidate_build_execution::Entity::delete_many()
                    .filter(
                        component_candidate_build_execution::Column::TenantId
                            .eq(tombstone.tenant_id),
                    )
                    .filter(
                        component_candidate_build_execution::Column::CandidateId
                            .is_in(candidate_ids.clone()),
                    )
                    .exec(&transaction)
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
                component_candidate_build::Entity::delete_many()
                    .filter(component_candidate_build::Column::TenantId.eq(tombstone.tenant_id))
                    .filter(component_candidate_build::Column::CandidateId.is_in(candidate_ids))
                    .exec(&transaction)
                    .await
                    .map_err(|error| ScriptError::Storage(error.to_string()))?;
            }
            component_candidate::Entity::delete_many()
                .filter(component_candidate::Column::ScriptId.eq(tombstone.id))
                .filter(component_candidate::Column::TenantId.eq(tombstone.tenant_id))
                .exec(&transaction)
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;

            draft_purge_receipt::Entity::insert(draft_purge_receipt::ActiveModel {
                id: ActiveValue::Set(Uuid::new_v4()),
                script_id: ActiveValue::Set(tombstone.id),
                tenant_id: ActiveValue::Set(tombstone.tenant_id),
                retention_policy: ActiveValue::Set(tombstone.retention_policy),
                retain_until: ActiveValue::Set(retain_until),
                purged_at: ActiveValue::Set(now),
                source_revision_count: ActiveValue::Set(source_revision_count),
                review_count: ActiveValue::Set(review_count),
                test_run_count: ActiveValue::Set(test_run_count),
                deletion_request_digest: ActiveValue::Set(tombstone.request_digest),
            })
            .exec_without_returning(&transaction)
            .await
            .map_err(|error| ScriptError::Storage(error.to_string()))?;
            transaction
                .commit()
                .await
                .map_err(|error| ScriptError::Storage(error.to_string()))?;
            purged = purged.saturating_add(1);
        }

        Ok(purged)
    }
}
