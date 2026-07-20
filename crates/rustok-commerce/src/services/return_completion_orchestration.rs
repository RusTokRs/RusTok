use rust_decimal::Decimal;
use rustok_core::generate_id;
use rustok_order::OrderService;
use rustok_order::dto::{
    CompleteOrderReturnInput, CreateOrderChangeInput, ListOrderChangesInput, OrderChangeResponse,
    OrderReturnResponse,
};
use rustok_order::error::OrderError;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::PaymentService;
use rustok_payment::dto::{CompleteRefundInput, CreateRefundInput, RefundResponse};
use rustok_payment::error::PaymentError;
use rustok_payment::providers::PaymentProviderRegistry;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use utoipa::ToSchema;
use uuid::Uuid;

use super::payment_orchestration::{PaymentOrchestrationError, PaymentOrchestrationService};
use super::post_order::{PostOrderOrchestrationError, PostOrderOrchestrationResult};
use super::return_completion_operation::{
    BeginReturnCompletionOperation, DEFAULT_RETURN_COMPLETION_LEASE_SECONDS,
    ReturnCompletionOperationCheckpoint, ReturnCompletionOperationJournal,
    ReturnCompletionOperationStage,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteReturnResolutionInput {
    pub resolution_type: Option<String>,
    pub refund_id: Option<Uuid>,
    pub order_change_id: Option<Uuid>,
    pub refund: Option<CompleteReturnRefundInput>,
    pub exchange: Option<CompleteReturnExchangeInput>,
    pub claim: Option<CompleteReturnClaimInput>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteReturnRefundInput {
    pub payment_collection_id: Option<Uuid>,
    pub amount: Decimal,
    pub reason: Option<String>,
    pub metadata: Value,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteReturnExchangeInput {
    pub description: Option<String>,
    pub preview: Value,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CompleteReturnClaimInput {
    pub description: Option<String>,
    pub preview: Value,
    pub metadata: Value,
}

/// Coordinates completion of an existing return with an optional refund,
/// exchange, or claim resolution.
///
/// The complete command is validated and durably admitted before any provider
/// or owner side effect. Retries adopt previously created refund/order-change
/// identities and a completed owner return.
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
        input: CompleteReturnResolutionInput,
    ) -> PostOrderOrchestrationResult<OrderReturnResponse> {
        validate_completion_shape(&input)?;
        let request_hash = completion_request_hash(&input)?;
        let journal = ReturnCompletionOperationJournal::new(self.db.clone());
        let operation = journal
            .begin(BeginReturnCompletionOperation {
                tenant_id,
                return_id,
                request_hash,
            })
            .await
            .map_err(map_journal_error)?;
        let order_service = OrderService::new(self.db.clone(), self.event_bus.clone());

        match operation.status.as_str() {
            "completed" => {
                return order_service
                    .get_return(tenant_id, return_id)
                    .await
                    .map_err(Into::into);
            }
            "reconciliation_required" => {
                return Err(PostOrderOrchestrationError::Validation(format!(
                    "return completion operation {} requires reconciliation",
                    operation.id
                )));
            }
            "failed" => {
                return Err(PostOrderOrchestrationError::Validation(format!(
                    "return completion operation {} is terminally failed",
                    operation.id
                )));
            }
            _ => {}
        }

        let lease_owner = format!("return-completion:{}:{}", return_id, generate_id());
        let claimed = journal
            .claim_execution(
                tenant_id,
                operation.id,
                lease_owner.clone(),
                DEFAULT_RETURN_COMPLETION_LEASE_SECONDS,
            )
            .await
            .map_err(map_journal_error)?;
        let Some(claimed) = claimed else {
            let current = journal
                .get(tenant_id, operation.id)
                .await
                .map_err(map_journal_error)?;
            if current.status == "completed" {
                return order_service
                    .get_return(tenant_id, return_id)
                    .await
                    .map_err(Into::into);
            }
            return Err(PostOrderOrchestrationError::Validation(format!(
                "return completion operation {} is already executing or requires operator action",
                operation.id
            )));
        };

        let result = self
            .execute_claimed(
                &journal,
                &order_service,
                tenant_id,
                actor_id,
                return_id,
                claimed,
                lease_owner.as_str(),
                input,
            )
            .await;

        match result {
            Ok(order_return) => {
                journal
                    .mark_completed(tenant_id, operation.id, lease_owner)
                    .await
                    .map_err(map_journal_error)?;
                Ok(order_return)
            }
            Err(error) => {
                self.record_failure(
                    &journal,
                    tenant_id,
                    operation.id,
                    lease_owner.as_str(),
                    &error,
                )
                .await?;
                Err(error)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn execute_claimed(
        &self,
        journal: &ReturnCompletionOperationJournal,
        order_service: &OrderService,
        tenant_id: Uuid,
        actor_id: Uuid,
        return_id: Uuid,
        mut operation: crate::entities::return_completion_operation::Model,
        lease_owner: &str,
        input: CompleteReturnResolutionInput,
    ) -> PostOrderOrchestrationResult<OrderReturnResponse> {
        let mut current_return = order_service.get_return(tenant_id, return_id).await?;
        if current_return.status == "completed" {
            return Ok(current_return);
        }

        let CompleteReturnResolutionInput {
            resolution_type,
            refund_id,
            order_change_id,
            refund,
            exchange,
            claim,
            metadata,
        } = input;
        let mut stage = ReturnCompletionOperationStage::from_str(operation.stage.as_str())
            .map_err(map_journal_error)?;
        self.validate_explicit_resolution_links(
            order_service,
            tenant_id,
            &current_return,
            refund_id,
            order_change_id,
        )
        .await?;
        let mut owner_input = CompleteOrderReturnInput {
            resolution_type,
            refund_id,
            order_change_id,
            metadata: normalize_object_or_empty(metadata, "metadata")?,
        };

        if let Some(refund_input) = refund {
            let refund = self
                .resolve_refund(
                    journal,
                    tenant_id,
                    return_id,
                    &current_return,
                    &mut operation,
                    lease_owner,
                    stage,
                    refund_input,
                )
                .await?;
            stage = ReturnCompletionOperationStage::from_str(operation.stage.as_str())
                .map_err(map_journal_error)?;
            owner_input.resolution_type = Some("refund".to_string());
            owner_input.refund_id = Some(refund.id);
            owner_input.order_change_id = None;
        } else if let Some(exchange_input) = exchange {
            let order_change = self
                .resolve_order_change(
                    journal,
                    order_service,
                    tenant_id,
                    actor_id,
                    return_id,
                    &current_return,
                    &mut operation,
                    lease_owner,
                    stage,
                    "exchange",
                    exchange_input.description,
                    exchange_input.preview,
                    exchange_input.metadata,
                )
                .await?;
            stage = ReturnCompletionOperationStage::from_str(operation.stage.as_str())
                .map_err(map_journal_error)?;
            owner_input.resolution_type = Some("exchange".to_string());
            owner_input.refund_id = None;
            owner_input.order_change_id = Some(order_change.id);
        } else if let Some(claim_input) = claim {
            let order_change = self
                .resolve_order_change(
                    journal,
                    order_service,
                    tenant_id,
                    actor_id,
                    return_id,
                    &current_return,
                    &mut operation,
                    lease_owner,
                    stage,
                    "claim",
                    claim_input.description,
                    claim_input.preview,
                    claim_input.metadata,
                )
                .await?;
            stage = ReturnCompletionOperationStage::from_str(operation.stage.as_str())
                .map_err(map_journal_error)?;
            owner_input.resolution_type = Some("claim".to_string());
            owner_input.refund_id = None;
            owner_input.order_change_id = Some(order_change.id);
        }

        current_return = order_service.get_return(tenant_id, return_id).await?;
        let completed = if current_return.status == "completed" {
            current_return
        } else {
            match order_service
                .complete_return(tenant_id, return_id, owner_input)
                .await
            {
                Ok(value) => value,
                Err(OrderError::InvalidTransition { .. }) => {
                    let adopted = order_service.get_return(tenant_id, return_id).await?;
                    if adopted.status == "completed" {
                        adopted
                    } else {
                        return Err(OrderError::InvalidTransition {
                            from: adopted.status,
                            to: "completed".to_string(),
                        }
                        .into());
                    }
                }
                Err(error) => return Err(error.into()),
            }
        };

        if stage != ReturnCompletionOperationStage::ReturnCompleted {
            journal
                .checkpoint(ReturnCompletionOperationCheckpoint {
                    tenant_id,
                    operation_id: operation.id,
                    lease_owner: lease_owner.to_string(),
                    expected_stage: stage,
                    next_stage: ReturnCompletionOperationStage::ReturnCompleted,
                    refund_id: None,
                    order_change_id: None,
                    lease_seconds: DEFAULT_RETURN_COMPLETION_LEASE_SECONDS,
                })
                .await
                .map_err(map_journal_error)?;
        }

        Ok(completed)
    }

    async fn validate_explicit_resolution_links(
        &self,
        order_service: &OrderService,
        tenant_id: Uuid,
        order_return: &OrderReturnResponse,
        refund_id: Option<Uuid>,
        order_change_id: Option<Uuid>,
    ) -> PostOrderOrchestrationResult<()> {
        if let Some(refund_id) = refund_id {
            let payment_service = PaymentService::new(self.db.clone());
            let refund = payment_service.get_refund(tenant_id, refund_id).await?;
            let collection = payment_service
                .get_collection(tenant_id, refund.payment_collection_id)
                .await?;
            if collection.order_id != Some(order_return.order_id) {
                return Err(PostOrderOrchestrationError::Validation(format!(
                    "refund {refund_id} is not attached to order {}",
                    order_return.order_id
                )));
            }
        }
        if let Some(order_change_id) = order_change_id {
            let order_change = order_service
                .get_order_change(tenant_id, order_change_id)
                .await?;
            if order_change.order_id != order_return.order_id {
                return Err(PostOrderOrchestrationError::Validation(format!(
                    "order change {order_change_id} is not attached to order {}",
                    order_return.order_id
                )));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn resolve_refund(
        &self,
        journal: &ReturnCompletionOperationJournal,
        tenant_id: Uuid,
        return_id: Uuid,
        order_return: &OrderReturnResponse,
        operation: &mut crate::entities::return_completion_operation::Model,
        lease_owner: &str,
        stage: ReturnCompletionOperationStage,
        input: CompleteReturnRefundInput,
    ) -> PostOrderOrchestrationResult<RefundResponse> {
        let payment_service = PaymentService::new(self.db.clone());
        let payment_orchestration = PaymentOrchestrationService::new(self.db.clone())
            .with_provider_registry(self.payment_provider_registry.clone());

        let mut refund = if let Some(refund_id) = operation.refund_id {
            payment_service.get_refund(tenant_id, refund_id).await?
        } else {
            if stage != ReturnCompletionOperationStage::Created {
                return Err(PostOrderOrchestrationError::Validation(format!(
                    "return completion operation {} reached `{}` without a refund identity",
                    operation.id, operation.stage
                )));
            }
            let collection_id = self
                .resolve_payment_collection(
                    tenant_id,
                    order_return.order_id,
                    input.payment_collection_id,
                )
                .await?;
            let created = payment_orchestration
                .create_refund_idempotent(
                    tenant_id,
                    collection_id,
                    format!("order_return:{return_id}:refund"),
                    CreateRefundInput {
                        amount: input.amount,
                        reason: input.reason,
                        metadata: normalize_object_or_empty(input.metadata, "refund.metadata")?,
                    },
                )
                .await?;
            *operation = journal
                .checkpoint(ReturnCompletionOperationCheckpoint {
                    tenant_id,
                    operation_id: operation.id,
                    lease_owner: lease_owner.to_string(),
                    expected_stage: ReturnCompletionOperationStage::Created,
                    next_stage: ReturnCompletionOperationStage::ResolutionCreated,
                    refund_id: Some(created.id),
                    order_change_id: None,
                    lease_seconds: DEFAULT_RETURN_COMPLETION_LEASE_SECONDS,
                })
                .await
                .map_err(map_journal_error)?;
            created
        };

        if input.complete && refund.status != "refunded" {
            refund = payment_orchestration
                .complete_refund(
                    tenant_id,
                    refund.id,
                    CompleteRefundInput {
                        metadata: serde_json::json!({
                            "source": "order_return_completion",
                            "return_id": return_id,
                            "return_completion_operation_id": operation.id,
                        }),
                    },
                )
                .await?;
        }
        Ok(refund)
    }

    #[allow(clippy::too_many_arguments)]
    async fn resolve_order_change(
        &self,
        journal: &ReturnCompletionOperationJournal,
        order_service: &OrderService,
        tenant_id: Uuid,
        actor_id: Uuid,
        return_id: Uuid,
        order_return: &OrderReturnResponse,
        operation: &mut crate::entities::return_completion_operation::Model,
        lease_owner: &str,
        stage: ReturnCompletionOperationStage,
        change_type: &str,
        description: Option<String>,
        preview: Value,
        metadata: Value,
    ) -> PostOrderOrchestrationResult<OrderChangeResponse> {
        let order_change = if let Some(order_change_id) = operation.order_change_id {
            order_service
                .get_order_change(tenant_id, order_change_id)
                .await?
        } else {
            if stage != ReturnCompletionOperationStage::Created {
                return Err(PostOrderOrchestrationError::Validation(format!(
                    "return completion operation {} reached `{}` without an order-change identity",
                    operation.id, operation.stage
                )));
            }
            if let Some(existing) = self
                .find_resolution_order_change(
                    order_service,
                    tenant_id,
                    order_return.order_id,
                    operation.id,
                    change_type,
                )
                .await?
            {
                existing
            } else {
                order_service
                    .create_order_change(
                        tenant_id,
                        actor_id,
                        order_return.order_id,
                        build_resolution_order_change(
                            change_type,
                            description,
                            preview,
                            metadata,
                            return_id,
                            operation.id,
                        )?,
                    )
                    .await?
            }
        };

        if stage == ReturnCompletionOperationStage::Created {
            *operation = journal
                .checkpoint(ReturnCompletionOperationCheckpoint {
                    tenant_id,
                    operation_id: operation.id,
                    lease_owner: lease_owner.to_string(),
                    expected_stage: ReturnCompletionOperationStage::Created,
                    next_stage: ReturnCompletionOperationStage::ResolutionCreated,
                    refund_id: None,
                    order_change_id: Some(order_change.id),
                    lease_seconds: DEFAULT_RETURN_COMPLETION_LEASE_SECONDS,
                })
                .await
                .map_err(map_journal_error)?;
        }
        Ok(order_change)
    }

    async fn find_resolution_order_change(
        &self,
        order_service: &OrderService,
        tenant_id: Uuid,
        order_id: Uuid,
        operation_id: Uuid,
        change_type: &str,
    ) -> PostOrderOrchestrationResult<Option<OrderChangeResponse>> {
        let (changes, _) = order_service
            .list_order_changes(
                tenant_id,
                ListOrderChangesInput {
                    page: 1,
                    per_page: 100,
                    order_id: Some(order_id),
                    status: None,
                    change_type: Some(change_type.to_string()),
                },
            )
            .await?;
        let operation_id = operation_id.to_string();
        Ok(changes.into_iter().find(|change| {
            change
                .metadata
                .get("return_completion_operation_id")
                .and_then(Value::as_str)
                == Some(operation_id.as_str())
        }))
    }

    async fn resolve_payment_collection(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
        explicit_collection_id: Option<Uuid>,
    ) -> PostOrderOrchestrationResult<Uuid> {
        let payment_service = PaymentService::new(self.db.clone());
        if let Some(collection_id) = explicit_collection_id {
            let collection = payment_service
                .get_collection(tenant_id, collection_id)
                .await?;
            if collection.order_id != Some(order_id) {
                return Err(PostOrderOrchestrationError::Validation(format!(
                    "payment collection {collection_id} is not attached to order {order_id}"
                )));
            }
            return Ok(collection_id);
        }

        payment_service
            .find_latest_collection_by_order(tenant_id, order_id)
            .await?
            .map(|collection| collection.id)
            .ok_or_else(|| {
                PostOrderOrchestrationError::Validation(format!(
                    "order {order_id} has no payment collection for return refund"
                ))
            })
    }

    async fn record_failure(
        &self,
        journal: &ReturnCompletionOperationJournal,
        tenant_id: Uuid,
        operation_id: Uuid,
        lease_owner: &str,
        error: &PostOrderOrchestrationError,
    ) -> PostOrderOrchestrationResult<()> {
        let message = error.to_string();
        match failure_disposition(error) {
            FailureDisposition::Retryable => journal
                .mark_retryable(
                    tenant_id,
                    operation_id,
                    lease_owner,
                    "return_completion_retryable",
                    message,
                )
                .await
                .map_err(map_journal_error)?,
            FailureDisposition::Reconciliation => journal
                .mark_reconciliation_required(
                    tenant_id,
                    operation_id,
                    lease_owner,
                    "return_completion_reconciliation_required",
                    message,
                )
                .await
                .map_err(map_journal_error)?,
            FailureDisposition::Failed => journal
                .mark_failed(
                    tenant_id,
                    operation_id,
                    lease_owner,
                    "return_completion_failed",
                    message,
                )
                .await
                .map_err(map_journal_error)?,
        };
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum FailureDisposition {
    Retryable,
    Reconciliation,
    Failed,
}

fn failure_disposition(error: &PostOrderOrchestrationError) -> FailureDisposition {
    match error {
        PostOrderOrchestrationError::Payment(error) => payment_failure_disposition(error),
        PostOrderOrchestrationError::PaymentOrchestration(error) => match error {
            PaymentOrchestrationError::ProviderAfterRefundReservation { source, .. }
            | PaymentOrchestrationError::Provider(source)
            | PaymentOrchestrationError::Payment(source) => payment_failure_disposition(source),
        },
        PostOrderOrchestrationError::Order(OrderError::Database(_) | OrderError::Core(_)) => {
            FailureDisposition::Retryable
        }
        PostOrderOrchestrationError::Order(_) | PostOrderOrchestrationError::Validation(_) => {
            FailureDisposition::Failed
        }
    }
}

fn payment_failure_disposition(error: &PaymentError) -> FailureDisposition {
    if error.requires_provider_reconciliation() {
        FailureDisposition::Reconciliation
    } else if error.is_provider_retryable()
        || matches!(
            error,
            PaymentError::Database(_) | PaymentError::ProviderConfiguration { .. }
        )
    {
        FailureDisposition::Retryable
    } else {
        FailureDisposition::Failed
    }
}

fn validate_completion_shape(
    input: &CompleteReturnResolutionInput,
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

fn build_resolution_order_change(
    change_type: &str,
    description: Option<String>,
    preview: Value,
    metadata: Value,
    return_id: Uuid,
    operation_id: Uuid,
) -> PostOrderOrchestrationResult<CreateOrderChangeInput> {
    Ok(CreateOrderChangeInput {
        change_type: change_type.to_string(),
        description,
        preview: attach_return_order_change_context(preview, return_id, operation_id, change_type)?,
        metadata: attach_return_order_change_context(
            metadata,
            return_id,
            operation_id,
            change_type,
        )?,
    })
}

fn attach_return_order_change_context(
    value: Value,
    return_id: Uuid,
    operation_id: Uuid,
    change_type: &str,
) -> PostOrderOrchestrationResult<Value> {
    let mut object = match normalize_object_or_empty(value, "metadata")? {
        Value::Object(object) => object,
        _ => unreachable!("normalize_object_or_empty returns an object"),
    };
    object.insert(
        "order_return_id".to_string(),
        Value::String(return_id.to_string()),
    );
    object.insert(
        "return_completion_operation_id".to_string(),
        Value::String(operation_id.to_string()),
    );
    object.insert(
        "return_decision_action".to_string(),
        Value::String(change_type.to_string()),
    );
    object.insert(
        "return_decision_source".to_string(),
        Value::String("rustok-commerce".to_string()),
    );
    Ok(Value::Object(object))
}

fn completion_request_hash(
    input: &CompleteReturnResolutionInput,
) -> PostOrderOrchestrationResult<String> {
    let payload = serde_json::json!({
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
    });
    let encoded = serde_json::to_vec(&payload).map_err(|error| {
        PostOrderOrchestrationError::Validation(format!(
            "failed to hash return completion request: {error}"
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

fn normalize_object_or_empty(value: Value, field: &str) -> PostOrderOrchestrationResult<Value> {
    match value {
        Value::Null => Ok(serde_json::json!({})),
        Value::Object(_) => Ok(value),
        _ => Err(PostOrderOrchestrationError::Validation(format!(
            "{field} must be a JSON object"
        ))),
    }
}

fn map_journal_error(
    error: super::return_completion_operation::ReturnCompletionOperationError,
) -> PostOrderOrchestrationError {
    PostOrderOrchestrationError::Validation(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> CompleteReturnResolutionInput {
        CompleteReturnResolutionInput {
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
    fn conflicting_helpers_are_rejected_before_execution() {
        let mut value = input();
        value.refund = Some(CompleteReturnRefundInput {
            payment_collection_id: None,
            amount: Decimal::ONE,
            reason: None,
            metadata: Value::Null,
            complete: false,
        });
        value.exchange = Some(CompleteReturnExchangeInput {
            description: None,
            preview: Value::Null,
            metadata: Value::Null,
        });
        assert!(validate_completion_shape(&value).is_err());
    }

    #[test]
    fn helper_resolution_type_must_match() {
        let mut value = input();
        value.resolution_type = Some("claim".to_string());
        value.refund = Some(CompleteReturnRefundInput {
            payment_collection_id: None,
            amount: Decimal::ONE,
            reason: None,
            metadata: Value::Null,
            complete: false,
        });
        assert!(validate_completion_shape(&value).is_err());
    }

    #[test]
    fn request_hash_is_stable_across_metadata_key_order() {
        let mut left = input();
        left.metadata = serde_json::json!({"b": 2, "a": 1});
        let mut right = input();
        right.metadata = serde_json::json!({"a": 1, "b": 2});
        assert_eq!(
            completion_request_hash(&left).unwrap(),
            completion_request_hash(&right).unwrap()
        );
    }
}
