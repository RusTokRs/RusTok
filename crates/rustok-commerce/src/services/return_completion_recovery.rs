use std::collections::HashMap;

use chrono::Utc;
use rustok_core::generate_id;
use rustok_order::OrderService;
use rustok_order::dto::OrderReturnResponse;
use rustok_order::error::OrderError;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::entities::{return_completion_command, return_completion_operation};

use super::post_order::{PostOrderOrchestrationError, PostOrderOrchestrationResult};
use super::return_completion_operation::{
    ReturnCompletionOperationStage, ReturnCompletionOperationStatus,
};
use super::return_completion_orchestration as core;

#[derive(Debug, Clone)]
pub struct ListReturnCompletionOperationsInput {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReturnCompletionOperationResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub return_id: Uuid,
    pub request_hash: String,
    pub status: String,
    pub stage: String,
    pub refund_id: Option<Uuid>,
    pub order_change_id: Option<Uuid>,
    pub attempt_count: i32,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<chrono::DateTime<Utc>>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub requested_by_actor_id: Option<Uuid>,
    pub retry_count: i32,
    pub last_retry_actor_id: Option<Uuid>,
    pub last_retry_at: Option<chrono::DateTime<Utc>>,
    pub can_retry: bool,
    pub requires_reconciliation: bool,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
    pub completed_at: Option<chrono::DateTime<Utc>>,
}

/// Durable return-completion facade.
///
/// The immutable command inbox and pending execution operation are committed in
/// one database transaction before the core orchestration may claim a lease or
/// execute provider/owner effects.
pub struct ReturnCompletionOrchestrationService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    payment_provider_registry: PaymentProviderRegistry,
}

impl ReturnCompletionOrchestrationService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            db,
            event_bus,
            payment_provider_registry: PaymentProviderRegistry::with_manual_provider(),
        }
    }

    pub fn with_payment_provider_registry(
        mut self,
        payment_provider_registry: PaymentProviderRegistry,
    ) -> Self {
        self.payment_provider_registry = payment_provider_registry;
        self
    }

    pub async fn complete_return(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        return_id: Uuid,
        input: core::CompleteReturnResolutionInput,
    ) -> PostOrderOrchestrationResult<OrderReturnResponse> {
        validate_completion_shape(&input)?;
        OrderService::new(self.db.clone(), self.event_bus.clone())
            .get_return(tenant_id, return_id)
            .await?;
        let request_payload = completion_request_payload(&input);
        let request_hash = completion_request_hash(&request_payload)?;
        self.admit_command_and_operation(
            tenant_id,
            actor_id,
            return_id,
            request_hash.as_str(),
            request_payload,
        )
        .await?;

        self.core_service()
            .complete_return(tenant_id, actor_id, return_id, input)
            .await
    }

    pub async fn retry_operation(
        &self,
        tenant_id: Uuid,
        retry_actor_id: Uuid,
        operation_id: Uuid,
    ) -> PostOrderOrchestrationResult<OrderReturnResponse> {
        let operation = return_completion_operation::Entity::find_by_id(operation_id)
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                PostOrderOrchestrationError::Validation(format!(
                    "return completion operation {operation_id} was not found"
                ))
            })?;

        ensure_operator_retry_allowed(&operation)?;
        let command = return_completion_command::Entity::find()
            .filter(return_completion_command::Column::TenantId.eq(tenant_id))
            .filter(return_completion_command::Column::ReturnId.eq(operation.return_id))
            .one(&self.db)
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                PostOrderOrchestrationError::Validation(format!(
                    "return completion operation {operation_id} has no durable command snapshot"
                ))
            })?;
        if command.request_hash != operation.request_hash {
            return Err(PostOrderOrchestrationError::Validation(format!(
                "return completion operation {operation_id} command hash does not match its execution journal"
            )));
        }

        let input: core::CompleteReturnResolutionInput = serde_json::from_value(
            command.request_payload.clone(),
        )
        .map_err(|error| {
            PostOrderOrchestrationError::Validation(format!(
                "return completion operation {operation_id} command snapshot is invalid: {error}"
            ))
        })?;
        validate_completion_shape(&input)?;
        self.record_retry(tenant_id, command.id, retry_actor_id)
            .await?;

        self.core_service()
            .complete_return(
                tenant_id,
                command.requested_by_actor_id,
                operation.return_id,
                input,
            )
            .await
    }

    pub async fn get_operation(
        &self,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> PostOrderOrchestrationResult<ReturnCompletionOperationResponse> {
        let operation = return_completion_operation::Entity::find_by_id(operation_id)
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                PostOrderOrchestrationError::Validation(format!(
                    "return completion operation {operation_id} was not found"
                ))
            })?;
        let command = return_completion_command::Entity::find()
            .filter(return_completion_command::Column::TenantId.eq(tenant_id))
            .filter(return_completion_command::Column::ReturnId.eq(operation.return_id))
            .one(&self.db)
            .await
            .map_err(storage_error)?;
        Ok(map_operation(operation, command.as_ref()))
    }

    pub async fn list_operations(
        &self,
        tenant_id: Uuid,
        input: ListReturnCompletionOperationsInput,
    ) -> PostOrderOrchestrationResult<(Vec<ReturnCompletionOperationResponse>, u64)> {
        let page = input.page.max(1);
        let per_page = input.per_page.clamp(1, 100);
        let mut query = return_completion_operation::Entity::find()
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id));
        if let Some(status) = input.status {
            let status = normalize_status_filter(status.as_str())?;
            query = query.filter(return_completion_operation::Column::Status.eq(status));
        }

        let total = query.clone().count(&self.db).await.map_err(storage_error)?;
        let operations = query
            .order_by_desc(return_completion_operation::Column::UpdatedAt)
            .offset((page - 1) * per_page)
            .limit(per_page)
            .all(&self.db)
            .await
            .map_err(storage_error)?;
        let return_ids = operations
            .iter()
            .map(|operation| operation.return_id)
            .collect::<Vec<_>>();
        let commands = if return_ids.is_empty() {
            Vec::new()
        } else {
            return_completion_command::Entity::find()
                .filter(return_completion_command::Column::TenantId.eq(tenant_id))
                .filter(return_completion_command::Column::ReturnId.is_in(return_ids))
                .all(&self.db)
                .await
                .map_err(storage_error)?
        };
        let commands = commands
            .into_iter()
            .map(|command| (command.return_id, command))
            .collect::<HashMap<_, _>>();

        Ok((
            operations
                .into_iter()
                .map(|operation| {
                    let command = commands.get(&operation.return_id);
                    map_operation(operation, command)
                })
                .collect(),
            total,
        ))
    }

    fn core_service(&self) -> core::ReturnCompletionOrchestrationService {
        core::ReturnCompletionOrchestrationService::new(self.db.clone(), self.event_bus.clone())
            .with_payment_provider_registry(self.payment_provider_registry.clone())
    }

    async fn admit_command_and_operation(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        return_id: Uuid,
        request_hash: &str,
        request_payload: Value,
    ) -> PostOrderOrchestrationResult<(
        return_completion_command::Model,
        return_completion_operation::Model,
    )> {
        let txn = self.db.begin().await.map_err(storage_error)?;
        let existing_command = return_completion_command::Entity::find()
            .filter(return_completion_command::Column::TenantId.eq(tenant_id))
            .filter(return_completion_command::Column::ReturnId.eq(return_id))
            .one(&txn)
            .await
            .map_err(storage_error)?;
        let existing_operation = return_completion_operation::Entity::find()
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .filter(return_completion_operation::Column::ReturnId.eq(return_id))
            .one(&txn)
            .await
            .map_err(storage_error)?;

        if let Some(existing) = existing_command.as_ref() {
            ensure_same_command(existing, request_hash, &request_payload)?;
        }
        if let Some(existing) = existing_operation.as_ref() {
            ensure_same_operation(existing, request_hash)?;
        }

        let now = Utc::now();
        let command = match existing_command {
            Some(existing) => existing,
            None => {
                let inserted = return_completion_command::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    return_id: Set(return_id),
                    request_hash: Set(request_hash.to_string()),
                    request_payload: Set(request_payload.clone()),
                    requested_by_actor_id: Set(actor_id),
                    retry_count: Set(0),
                    last_retry_actor_id: Set(None),
                    last_retry_at: Set(None),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await;
                match inserted {
                    Ok(model) => model,
                    Err(error) if is_unique_constraint(&error) => {
                        txn.rollback().await.map_err(storage_error)?;
                        return self
                            .load_existing_admission(
                                tenant_id,
                                return_id,
                                request_hash,
                                &request_payload,
                            )
                            .await;
                    }
                    Err(error) => return Err(storage_error(error)),
                }
            }
        };

        let operation = match existing_operation {
            Some(existing) => existing,
            None => {
                let inserted = return_completion_operation::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    return_id: Set(return_id),
                    request_hash: Set(request_hash.to_string()),
                    status: Set(ReturnCompletionOperationStatus::Pending
                        .as_str()
                        .to_string()),
                    stage: Set(ReturnCompletionOperationStage::Created.as_str().to_string()),
                    refund_id: Set(None),
                    order_change_id: Set(None),
                    attempt_count: Set(0),
                    lease_owner: Set(None),
                    lease_expires_at: Set(None),
                    last_error_code: Set(None),
                    last_error_message: Set(None),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                    completed_at: Set(None),
                }
                .insert(&txn)
                .await;
                match inserted {
                    Ok(model) => model,
                    Err(error) if is_unique_constraint(&error) => {
                        txn.rollback().await.map_err(storage_error)?;
                        return self
                            .load_existing_admission(
                                tenant_id,
                                return_id,
                                request_hash,
                                &request_payload,
                            )
                            .await;
                    }
                    Err(error) => return Err(storage_error(error)),
                }
            }
        };

        txn.commit().await.map_err(storage_error)?;
        Ok((command, operation))
    }

    async fn load_existing_admission(
        &self,
        tenant_id: Uuid,
        return_id: Uuid,
        request_hash: &str,
        request_payload: &Value,
    ) -> PostOrderOrchestrationResult<(
        return_completion_command::Model,
        return_completion_operation::Model,
    )> {
        let command = return_completion_command::Entity::find()
            .filter(return_completion_command::Column::TenantId.eq(tenant_id))
            .filter(return_completion_command::Column::ReturnId.eq(return_id))
            .one(&self.db)
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                PostOrderOrchestrationError::Validation(format!(
                    "return {return_id} command admission raced but no command was committed"
                ))
            })?;
        let operation = return_completion_operation::Entity::find()
            .filter(return_completion_operation::Column::TenantId.eq(tenant_id))
            .filter(return_completion_operation::Column::ReturnId.eq(return_id))
            .one(&self.db)
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                PostOrderOrchestrationError::Validation(format!(
                    "return {return_id} command admission raced but no operation was committed"
                ))
            })?;
        ensure_same_command(&command, request_hash, request_payload)?;
        ensure_same_operation(&operation, request_hash)?;
        Ok((command, operation))
    }

    async fn record_retry(
        &self,
        tenant_id: Uuid,
        command_id: Uuid,
        retry_actor_id: Uuid,
    ) -> PostOrderOrchestrationResult<()> {
        let now = Utc::now().fixed_offset();
        let result = return_completion_command::Entity::update_many()
            .col_expr(
                return_completion_command::Column::RetryCount,
                Expr::col(return_completion_command::Column::RetryCount).add(1),
            )
            .col_expr(
                return_completion_command::Column::LastRetryActorId,
                Expr::value(Some(retry_actor_id)),
            )
            .col_expr(
                return_completion_command::Column::LastRetryAt,
                Expr::value(Some(now)),
            )
            .col_expr(
                return_completion_command::Column::UpdatedAt,
                Expr::current_timestamp().into(),
            )
            .filter(return_completion_command::Column::TenantId.eq(tenant_id))
            .filter(return_completion_command::Column::Id.eq(command_id))
            .exec(&self.db)
            .await
            .map_err(storage_error)?;
        if result.rows_affected != 1 {
            return Err(PostOrderOrchestrationError::Validation(format!(
                "return completion command {command_id} disappeared before retry"
            )));
        }
        Ok(())
    }
}

fn ensure_operator_retry_allowed(
    operation: &return_completion_operation::Model,
) -> PostOrderOrchestrationResult<()> {
    match operation.status.as_str() {
        "pending" | "retryable_error" => Ok(()),
        "executing"
            if operation
                .lease_expires_at
                .map(|expires_at| expires_at <= Utc::now().fixed_offset())
                .unwrap_or(true) =>
        {
            Ok(())
        }
        "completed" => Err(PostOrderOrchestrationError::Validation(format!(
            "return completion operation {} is already completed",
            operation.id
        ))),
        "reconciliation_required" => Err(PostOrderOrchestrationError::Validation(format!(
            "return completion operation {} requires reconciliation and cannot be retried automatically",
            operation.id
        ))),
        "failed" => Err(PostOrderOrchestrationError::Validation(format!(
            "return completion operation {} is terminally failed",
            operation.id
        ))),
        _ => Err(PostOrderOrchestrationError::Validation(format!(
            "return completion operation {} is currently leased and cannot be retried",
            operation.id
        ))),
    }
}

fn map_operation(
    operation: return_completion_operation::Model,
    command: Option<&return_completion_command::Model>,
) -> ReturnCompletionOperationResponse {
    let now = Utc::now().fixed_offset();
    let can_retry = command.is_some()
        && (matches!(operation.status.as_str(), "pending" | "retryable_error")
            || (operation.status == "executing"
                && operation
                    .lease_expires_at
                    .map(|expires_at| expires_at <= now)
                    .unwrap_or(true)));
    ReturnCompletionOperationResponse {
        id: operation.id,
        tenant_id: operation.tenant_id,
        return_id: operation.return_id,
        request_hash: operation.request_hash,
        status: operation.status.clone(),
        stage: operation.stage,
        refund_id: operation.refund_id,
        order_change_id: operation.order_change_id,
        attempt_count: operation.attempt_count,
        lease_owner: operation.lease_owner,
        lease_expires_at: operation
            .lease_expires_at
            .map(|value| value.with_timezone(&Utc)),
        last_error_code: operation.last_error_code,
        last_error_message: operation.last_error_message,
        requested_by_actor_id: command.map(|value| value.requested_by_actor_id),
        retry_count: command.map(|value| value.retry_count).unwrap_or(0),
        last_retry_actor_id: command.and_then(|value| value.last_retry_actor_id),
        last_retry_at: command
            .and_then(|value| value.last_retry_at)
            .map(|value| value.with_timezone(&Utc)),
        can_retry,
        requires_reconciliation: operation.status == "reconciliation_required",
        created_at: operation.created_at.with_timezone(&Utc),
        updated_at: operation.updated_at.with_timezone(&Utc),
        completed_at: operation
            .completed_at
            .map(|value| value.with_timezone(&Utc)),
    }
}

fn ensure_same_command(
    existing: &return_completion_command::Model,
    request_hash: &str,
    request_payload: &Value,
) -> PostOrderOrchestrationResult<()> {
    if existing.request_hash != request_hash || &existing.request_payload != request_payload {
        return Err(PostOrderOrchestrationError::Validation(format!(
            "return {} already has a different completion command",
            existing.return_id
        )));
    }
    Ok(())
}

fn ensure_same_operation(
    existing: &return_completion_operation::Model,
    request_hash: &str,
) -> PostOrderOrchestrationResult<()> {
    if existing.request_hash != request_hash {
        return Err(PostOrderOrchestrationError::Validation(format!(
            "return {} execution journal is already bound to another command",
            existing.return_id
        )));
    }
    Ok(())
}

fn normalize_status_filter(value: &str) -> PostOrderOrchestrationResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if matches!(
        value.as_str(),
        "pending"
            | "executing"
            | "retryable_error"
            | "reconciliation_required"
            | "completed"
            | "failed"
    ) {
        return Ok(value);
    }
    Err(PostOrderOrchestrationError::Validation(
        "invalid return completion operation status filter".to_string(),
    ))
}

fn validate_completion_shape(
    input: &core::CompleteReturnResolutionInput,
) -> PostOrderOrchestrationResult<()> {
    let helpers = usize::from(input.refund.is_some())
        + usize::from(input.exchange.is_some())
        + usize::from(input.claim.is_some());
    if helpers > 1 {
        return Err(PostOrderOrchestrationError::Validation(
            "refund, exchange, and claim helpers are mutually exclusive".to_string(),
        ));
    }
    if helpers > 0 && (input.refund_id.is_some() || input.order_change_id.is_some()) {
        return Err(PostOrderOrchestrationError::Validation(
            "resolution helpers cannot be combined with explicit refund_id or order_change_id"
                .to_string(),
        ));
    }
    let expected = if input.refund.is_some() {
        Some("refund")
    } else if input.exchange.is_some() {
        Some("exchange")
    } else if input.claim.is_some() {
        Some("claim")
    } else {
        None
    };
    if let (Some(expected), Some(actual)) = (expected, input.resolution_type.as_deref()) {
        if !actual.trim().eq_ignore_ascii_case(expected) {
            return Err(PostOrderOrchestrationError::Validation(format!(
                "{expected} helper requires resolution_type to be omitted or `{expected}`"
            )));
        }
    }
    Ok(())
}

fn completion_request_payload(input: &core::CompleteReturnResolutionInput) -> Value {
    serde_json::json!({
        "version": 1,
        "resolution_type": input.resolution_type.as_ref().map(|value| value.trim().to_ascii_lowercase()),
        "refund_id": input.refund_id,
        "order_change_id": input.order_change_id,
        "refund": input.refund.as_ref().map(|refund| serde_json::json!({
            "payment_collection_id": refund.payment_collection_id,
            "amount": refund.amount.normalize().to_string(),
            "reason": refund.reason,
            "metadata": canonical_json(&refund.metadata),
            "complete": refund.complete,
        })),
        "exchange": input.exchange.as_ref().map(|exchange| serde_json::json!({
            "description": exchange.description,
            "preview": canonical_json(&exchange.preview),
            "metadata": canonical_json(&exchange.metadata),
        })),
        "claim": input.claim.as_ref().map(|claim| serde_json::json!({
            "description": claim.description,
            "preview": canonical_json(&claim.preview),
            "metadata": canonical_json(&claim.metadata),
        })),
        "metadata": canonical_json(&input.metadata),
    })
}

fn completion_request_hash(payload: &Value) -> PostOrderOrchestrationResult<String> {
    let encoded = serde_json::to_vec(payload).map_err(|error| {
        PostOrderOrchestrationError::Validation(format!(
            "failed to hash return completion command: {error}"
        ))
    })?;
    Ok(Sha256::digest(encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        Value::Object(values) => {
            let mut keys = values.keys().collect::<Vec<_>>();
            keys.sort();
            let mut canonical = serde_json::Map::new();
            for key in keys {
                canonical.insert(key.clone(), canonical_json(&values[key]));
            }
            Value::Object(canonical)
        }
        value => value.clone(),
    }
}

fn storage_error(error: sea_orm::DbErr) -> PostOrderOrchestrationError {
    OrderError::Database(error).into()
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;

    fn input() -> core::CompleteReturnResolutionInput {
        core::CompleteReturnResolutionInput {
            resolution_type: None,
            refund_id: None,
            order_change_id: None,
            refund: None,
            exchange: None,
            claim: None,
            metadata: Value::Null,
        }
    }

    #[test]
    fn command_payload_round_trips() {
        let mut value = input();
        value.refund = Some(core::CompleteReturnRefundInput {
            payment_collection_id: None,
            amount: Decimal::ONE,
            reason: Some("customer request".to_string()),
            metadata: serde_json::json!({"b": 2, "a": 1}),
            complete: true,
        });
        let payload = completion_request_payload(&value);
        let decoded: core::CompleteReturnResolutionInput =
            serde_json::from_value(payload.clone()).unwrap();
        assert_eq!(completion_request_payload(&decoded), payload);
    }

    #[test]
    fn command_hash_is_stable_across_metadata_key_order() {
        let mut left = input();
        left.metadata = serde_json::json!({"b": 2, "a": 1});
        let mut right = input();
        right.metadata = serde_json::json!({"a": 1, "b": 2});
        assert_eq!(
            completion_request_hash(&completion_request_payload(&left)).unwrap(),
            completion_request_hash(&completion_request_payload(&right)).unwrap()
        );
    }
}
