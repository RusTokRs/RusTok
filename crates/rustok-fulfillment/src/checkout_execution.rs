use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    CreateFulfillmentInput, CreateFulfillmentItemInput, FulfillmentError, FulfillmentResponse,
    FulfillmentService,
};

const CHECKOUT_FULFILLMENT_OWNER: &str = "rustok_fulfillment";
const CHECKOUT_FULFILLMENT_BOUNDARY: &str = "checkout_fulfillment_execution_port";
const ENSURE_OPERATION: &str = "ensure_checkout_fulfillments";
const READ_OPERATION: &str = "read_checkout_fulfillments";

#[async_trait]
pub trait CheckoutFulfillmentExecutionPort: Send + Sync {
    async fn ensure_checkout_fulfillments(
        &self,
        context: PortContext,
        request: EnsureCheckoutFulfillmentsRequest,
    ) -> Result<Vec<FulfillmentResponse>, PortError>;

    async fn read_checkout_fulfillments(
        &self,
        context: PortContext,
        request: ReadCheckoutFulfillmentsRequest,
    ) -> Result<Vec<FulfillmentResponse>, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnsureCheckoutFulfillmentsRequest {
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub order_plan_hash: String,
    pub plans: Vec<CheckoutFulfillmentCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadCheckoutFulfillmentsRequest {
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub order_plan_hash: String,
    pub expected_plans: Vec<CheckoutFulfillmentCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutFulfillmentCommand {
    pub index: u32,
    pub shipping_option_id: Option<Uuid>,
    pub carrier: Option<String>,
    pub tracking_number: Option<String>,
    pub items: Vec<CheckoutFulfillmentItemCommand>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutFulfillmentItemCommand {
    pub order_line_item_id: Uuid,
    pub cart_line_item_id: Uuid,
    pub quantity: i32,
    pub metadata: Value,
}

pub struct InProcessCheckoutFulfillmentExecutionPort {
    service: FulfillmentService,
}

impl InProcessCheckoutFulfillmentExecutionPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            service: FulfillmentService::new(db),
        }
    }

    async fn ensure(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        request: EnsureCheckoutFulfillmentsRequest,
    ) -> Result<Vec<FulfillmentResponse>, PortError> {
        validate_request(
            request.checkout_operation_id,
            request.order_id,
            request.order_plan_hash.as_str(),
            &request.plans,
        )
        .map_err(|error| {
            map_checkout_fulfillment_local_port_error(
                context,
                ENSURE_OPERATION,
                "validate_request",
                error,
            )
        })?;
        let mut result = Vec::with_capacity(request.plans.len());
        for plan in &request.plans {
            let key = fulfillment_key(request.checkout_operation_id, plan.index);
            let input = build_input(&request, plan, key.as_str());
            let existing = self
                .find_by_key(
                    context,
                    ENSURE_OPERATION,
                    "find_checkout_fulfillment_before_create",
                    tenant_id,
                    request.order_id,
                    key.as_str(),
                )
                .await?;
            let fulfillment = match existing {
                Some(existing) => existing,
                None => match self.service.create_fulfillment(tenant_id, input).await {
                    Ok(created) => created,
                    Err(error) => {
                        let adopted = self
                            .find_by_key(
                                context,
                                ENSURE_OPERATION,
                                "adopt_checkout_fulfillment_after_create_error",
                                tenant_id,
                                request.order_id,
                                key.as_str(),
                            )
                            .await?;
                        match adopted {
                            Some(adopted) => adopted,
                            None => {
                                return Err(fulfillment_error_to_port_error(
                                    context,
                                    "create_checkout_fulfillment",
                                    error,
                                ));
                            }
                        }
                    }
                },
            };
            validate_fulfillment(
                &fulfillment,
                FulfillmentExpectation {
                    tenant_id,
                    checkout_operation_id: request.checkout_operation_id,
                    order_id: request.order_id,
                    customer_id: request.customer_id,
                    plan_hash: request.order_plan_hash.as_str(),
                    plan,
                    key: key.as_str(),
                },
            )
            .map_err(|error| {
                map_checkout_fulfillment_local_port_error(
                    context,
                    ENSURE_OPERATION,
                    "validate_fulfillment",
                    error,
                )
            })?;
            result.push(fulfillment);
        }
        result.sort_by_key(fulfillment_index);
        Ok(result)
    }

    async fn read(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        request: ReadCheckoutFulfillmentsRequest,
    ) -> Result<Vec<FulfillmentResponse>, PortError> {
        validate_request(
            request.checkout_operation_id,
            request.order_id,
            request.order_plan_hash.as_str(),
            &request.expected_plans,
        )
        .map_err(|error| {
            map_checkout_fulfillment_local_port_error(
                context,
                READ_OPERATION,
                "validate_request",
                error,
            )
        })?;
        let rows = self
            .service
            .list_by_order(tenant_id, request.order_id)
            .await
            .map_err(|error| {
                fulfillment_error_to_port_error(
                    context,
                    "list_checkout_fulfillments_for_read",
                    error,
                )
            })?;
        let operation_id = request.checkout_operation_id.to_string();
        let mut by_index = BTreeMap::new();
        for row in rows.into_iter().filter(|row| {
            row.metadata
                .get("checkout")
                .and_then(|checkout| checkout.get("operation_id"))
                .and_then(Value::as_str)
                == Some(operation_id.as_str())
        }) {
            let index = fulfillment_index(&row);
            if by_index.insert(index, row).is_some() {
                return Err(map_checkout_fulfillment_local_port_error(
                    context,
                    READ_OPERATION,
                    "collect_checkout_fulfillment_set",
                    PortError::conflict(
                        "fulfillment.checkout_identity_duplicate",
                        "multiple fulfillments share one checkout fulfillment identity",
                    ),
                ));
            }
        }
        if by_index.len() != request.expected_plans.len() {
            return Err(map_checkout_fulfillment_local_port_error(
                context,
                READ_OPERATION,
                "require_complete_checkout_fulfillment_set",
                PortError::conflict(
                    "fulfillment.checkout_set_incomplete",
                    "checkout fulfillment set is incomplete",
                ),
            ));
        }
        let mut result = Vec::with_capacity(request.expected_plans.len());
        for plan in &request.expected_plans {
            let key = fulfillment_key(request.checkout_operation_id, plan.index);
            let fulfillment = by_index.remove(&plan.index).ok_or_else(|| {
                map_checkout_fulfillment_local_port_error(
                    context,
                    READ_OPERATION,
                    "require_complete_checkout_fulfillment_set",
                    PortError::conflict(
                        "fulfillment.checkout_set_incomplete",
                        "checkout fulfillment set is incomplete",
                    ),
                )
            })?;
            validate_fulfillment(
                &fulfillment,
                FulfillmentExpectation {
                    tenant_id,
                    checkout_operation_id: request.checkout_operation_id,
                    order_id: request.order_id,
                    customer_id: request.customer_id,
                    plan_hash: request.order_plan_hash.as_str(),
                    plan,
                    key: key.as_str(),
                },
            )
            .map_err(|error| {
                map_checkout_fulfillment_local_port_error(
                    context,
                    READ_OPERATION,
                    "validate_fulfillment",
                    error,
                )
            })?;
            result.push(fulfillment);
        }
        Ok(result)
    }

    async fn find_by_key(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        service_operation: &'static str,
        tenant_id: Uuid,
        order_id: Uuid,
        key: &str,
    ) -> Result<Option<FulfillmentResponse>, PortError> {
        let rows = self
            .service
            .list_by_order(tenant_id, order_id)
            .await
            .map_err(|error| fulfillment_error_to_port_error(context, service_operation, error))?;
        let mut matches = rows.into_iter().filter(|fulfillment| {
            fulfillment
                .metadata
                .get("checkout")
                .and_then(|checkout| checkout.get("fulfillment_key"))
                .and_then(Value::as_str)
                == Some(key)
        });
        let first = matches.next();
        if matches.next().is_some() {
            return Err(map_checkout_fulfillment_local_port_error(
                context,
                owner_operation,
                "find_checkout_fulfillment_by_key",
                PortError::conflict(
                    "fulfillment.checkout_identity_duplicate",
                    "multiple fulfillments share one checkout fulfillment identity",
                ),
            ));
        }
        Ok(first)
    }
}

fn map_checkout_fulfillment_local_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    local_operation: &'static str,
    error: PortError,
) -> PortError {
    let error_kind = match &error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    };
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    let tenant_id_length = context.tenant_id.chars().count();
    let actor_id_length = context.actor.id.chars().count();
    let claim_count = context.claims.len();
    let role_count = context.roles.len();
    let channel_present = context.channel.is_some();
    let channel_length = context.channel.as_ref().map(|value| value.chars().count());
    let locale_length = context.locale.chars().count();
    let causation_id_present = context.causation_id.is_some();
    let causation_id_length = context
        .causation_id
        .as_ref()
        .map(|value| value.chars().count());
    let traceparent_present = context.traceparent.is_some();
    let traceparent_length = context
        .traceparent
        .as_ref()
        .map(|value| value.chars().count());
    let idempotency_key_present = context.idempotency_key.is_some();
    let idempotency_key_length = context
        .idempotency_key
        .as_ref()
        .map(|value| value.chars().count());
    let internal_message_present = !error.message.trim().is_empty();
    let internal_message_length = error.message.chars().count();

    if technical_failure {
        tracing::error!(
            owner = CHECKOUT_FULFILLMENT_OWNER,
            owner_operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id_length,
            actor_kind,
            actor_id_length,
            claim_count,
            role_count,
            channel_present,
            channel_length = ?channel_length,
            locale_length,
            causation_id_present,
            causation_id_length = ?causation_id_length,
            traceparent_present,
            traceparent_length = ?traceparent_length,
            idempotency_key_present,
            idempotency_key_length = ?idempotency_key_length,
            deadline_ms = ?context.deadline_ms,
            internal_code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
            retryable = error.retryable,
            boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
            "checkout fulfillment local owner operation failed"
        );
    } else {
        tracing::warn!(
            owner = CHECKOUT_FULFILLMENT_OWNER,
            owner_operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id_length,
            actor_kind,
            actor_id_length,
            claim_count,
            role_count,
            channel_present,
            channel_length = ?channel_length,
            locale_length,
            causation_id_present,
            causation_id_length = ?causation_id_length,
            traceparent_present,
            traceparent_length = ?traceparent_length,
            idempotency_key_present,
            idempotency_key_length = ?idempotency_key_length,
            deadline_ms = ?context.deadline_ms,
            internal_code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
            retryable = error.retryable,
            boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
            "checkout fulfillment local owner operation was rejected"
        );
    }

    error
}

pub fn in_process_checkout_fulfillment_execution_port(
    db: DatabaseConnection,
) -> Arc<dyn CheckoutFulfillmentExecutionPort> {
    Arc::new(InProcessCheckoutFulfillmentExecutionPort::new(db))
}

fn require_checkout_fulfillment_read_admission(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .inspect_err(|error| {
            log_checkout_fulfillment_admission_rejection(context, owner_operation, "policy", error);
        })
}

fn require_checkout_fulfillment_write_admission(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .inspect_err(|error| {
            log_checkout_fulfillment_admission_rejection(context, owner_operation, "policy", error);
        })?;
    context.require_write_semantics().inspect_err(|error| {
        log_checkout_fulfillment_admission_rejection(
            context,
            owner_operation,
            "write_semantics",
            error,
        );
    })
}

fn log_checkout_fulfillment_admission_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    admission_phase: &'static str,
    error: &PortError,
) {
    let error_kind = match &error.kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    };
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    let tenant_id_length = context.tenant_id.chars().count();
    let actor_id_length = context.actor.id.chars().count();
    let claim_count = context.claims.len();
    let role_count = context.roles.len();
    let channel_present = context.channel.is_some();
    let channel_length = context.channel.as_ref().map(|value| value.chars().count());
    let locale_length = context.locale.chars().count();
    let causation_id_present = context.causation_id.is_some();
    let causation_id_length = context
        .causation_id
        .as_ref()
        .map(|value| value.chars().count());
    let traceparent_present = context.traceparent.is_some();
    let traceparent_length = context
        .traceparent
        .as_ref()
        .map(|value| value.chars().count());
    let idempotency_key_present = context.idempotency_key.is_some();
    let idempotency_key_length = context
        .idempotency_key
        .as_ref()
        .map(|value| value.chars().count());
    let internal_message_present = !error.message.trim().is_empty();
    let internal_message_length = error.message.chars().count();

    if technical_failure {
        tracing::error!(
            owner = CHECKOUT_FULFILLMENT_OWNER,
            owner_operation,
            admission_phase,
            correlation_id = %context.correlation_id,
            tenant_id_length,
            actor_kind,
            actor_id_length,
            claim_count,
            role_count,
            channel_present,
            channel_length = ?channel_length,
            locale_length,
            causation_id_present,
            causation_id_length = ?causation_id_length,
            traceparent_present,
            traceparent_length = ?traceparent_length,
            idempotency_key_present,
            idempotency_key_length = ?idempotency_key_length,
            deadline_ms = ?context.deadline_ms,
            internal_code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
            retryable = error.retryable,
            boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
            "checkout fulfillment owner admission failed"
        );
    } else {
        tracing::warn!(
            owner = CHECKOUT_FULFILLMENT_OWNER,
            owner_operation,
            admission_phase,
            correlation_id = %context.correlation_id,
            tenant_id_length,
            actor_kind,
            actor_id_length,
            claim_count,
            role_count,
            channel_present,
            channel_length = ?channel_length,
            locale_length,
            causation_id_present,
            causation_id_length = ?causation_id_length,
            traceparent_present,
            traceparent_length = ?traceparent_length,
            idempotency_key_present,
            idempotency_key_length = ?idempotency_key_length,
            deadline_ms = ?context.deadline_ms,
            internal_code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
            retryable = error.retryable,
            boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
            "checkout fulfillment owner admission was rejected"
        );
    }
}

#[async_trait]
impl CheckoutFulfillmentExecutionPort for InProcessCheckoutFulfillmentExecutionPort {
    async fn ensure_checkout_fulfillments(
        &self,
        context: PortContext,
        request: EnsureCheckoutFulfillmentsRequest,
    ) -> Result<Vec<FulfillmentResponse>, PortError> {
        require_checkout_fulfillment_write_admission(&context, ENSURE_OPERATION)?;
        let tenant_id = parse_tenant_id(&context, ENSURE_OPERATION)?;
        require_operation_context(&context, ENSURE_OPERATION, request.checkout_operation_id)?;
        self.ensure(&context, tenant_id, request).await
    }

    async fn read_checkout_fulfillments(
        &self,
        context: PortContext,
        request: ReadCheckoutFulfillmentsRequest,
    ) -> Result<Vec<FulfillmentResponse>, PortError> {
        require_checkout_fulfillment_read_admission(&context, READ_OPERATION)?;
        let tenant_id = parse_tenant_id(&context, READ_OPERATION)?;
        require_operation_context(&context, READ_OPERATION, request.checkout_operation_id)?;
        self.read(&context, tenant_id, request).await
    }
}

fn validate_request(
    checkout_operation_id: Uuid,
    order_id: Uuid,
    plan_hash: &str,
    plans: &[CheckoutFulfillmentCommand],
) -> Result<(), PortError> {
    if checkout_operation_id.is_nil() || order_id.is_nil() {
        return Err(PortError::validation(
            "fulfillment.checkout_identity_invalid",
            "checkout operation and order identity must be non-nil UUIDs",
        ));
    }
    let plan_hash = plan_hash.trim();
    if plan_hash.len() != 64 || !plan_hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(PortError::validation(
            "fulfillment.checkout_plan_hash_invalid",
            "checkout fulfillment plan hash must be a 64-character hexadecimal value",
        ));
    }
    let mut indexes = HashSet::new();
    let mut line_ids = HashSet::new();
    for plan in plans {
        if !indexes.insert(plan.index) || plan.items.is_empty() {
            return Err(PortError::validation(
                "fulfillment.checkout_plan_invalid",
                "checkout fulfillment plans require unique indexes and non-empty items",
            ));
        }
        for item in &plan.items {
            if item.order_line_item_id.is_nil()
                || item.cart_line_item_id.is_nil()
                || item.quantity <= 0
                || !line_ids.insert(item.order_line_item_id)
            {
                return Err(PortError::validation(
                    "fulfillment.checkout_item_invalid",
                    "checkout fulfillment items require unique order lines and positive quantities",
                ));
            }
        }
    }
    Ok(())
}

fn build_input(
    request: &EnsureCheckoutFulfillmentsRequest,
    plan: &CheckoutFulfillmentCommand,
    key: &str,
) -> CreateFulfillmentInput {
    CreateFulfillmentInput {
        order_id: request.order_id,
        shipping_option_id: plan.shipping_option_id,
        customer_id: request.customer_id,
        carrier: plan.carrier.clone(),
        tracking_number: plan.tracking_number.clone(),
        items: Some(
            plan.items
                .iter()
                .map(|item| CreateFulfillmentItemInput {
                    order_line_item_id: item.order_line_item_id,
                    quantity: item.quantity,
                    metadata: fulfillment_item_metadata(
                        item.metadata.clone(),
                        request.checkout_operation_id,
                        item.cart_line_item_id,
                        request.order_plan_hash.as_str(),
                        plan.index,
                    ),
                })
                .collect(),
        ),
        metadata: fulfillment_metadata(
            plan.metadata.clone(),
            request.checkout_operation_id,
            request.order_id,
            request.order_plan_hash.as_str(),
            plan.index,
            key,
        ),
    }
}

struct FulfillmentExpectation<'a> {
    tenant_id: Uuid,
    checkout_operation_id: Uuid,
    order_id: Uuid,
    customer_id: Option<Uuid>,
    plan_hash: &'a str,
    plan: &'a CheckoutFulfillmentCommand,
    key: &'a str,
}

fn validate_fulfillment(
    fulfillment: &FulfillmentResponse,
    expected: FulfillmentExpectation<'_>,
) -> Result<(), PortError> {
    let FulfillmentExpectation {
        tenant_id,
        checkout_operation_id,
        order_id,
        customer_id,
        plan_hash,
        plan,
        key,
    } = expected;
    if fulfillment.tenant_id != tenant_id
        || fulfillment.order_id != order_id
        || fulfillment.shipping_option_id != plan.shipping_option_id
        || fulfillment.customer_id != customer_id
        || fulfillment.carrier.as_deref() != plan.carrier.as_deref()
        || fulfillment.tracking_number.as_deref() != plan.tracking_number.as_deref()
    {
        return Err(PortError::conflict(
            "fulfillment.checkout_plan_conflict",
            "fulfillment does not match the immutable checkout plan",
        ));
    }
    let expected_items = plan
        .items
        .iter()
        .map(|item| (item.order_line_item_id, item.quantity))
        .collect::<BTreeMap<_, _>>();
    let actual_items = fulfillment
        .items
        .iter()
        .map(|item| (item.order_line_item_id, item.quantity))
        .collect::<BTreeMap<_, _>>();
    if expected_items != actual_items {
        return Err(PortError::conflict(
            "fulfillment.checkout_items_conflict",
            "fulfillment items do not match the immutable checkout plan",
        ));
    }
    let checkout = fulfillment
        .metadata
        .get("checkout")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            PortError::conflict(
                "fulfillment.checkout_identity_missing",
                "fulfillment has no checkout identity",
            )
        })?;
    let operation_id = checkout_operation_id.to_string();
    let order_id_text = order_id.to_string();
    if checkout.get("operation_id").and_then(Value::as_str) != Some(operation_id.as_str())
        || checkout.get("order_id").and_then(Value::as_str) != Some(order_id_text.as_str())
        || checkout.get("order_plan_hash").and_then(Value::as_str) != Some(plan_hash)
        || checkout.get("fulfillment_index").and_then(Value::as_u64) != Some(u64::from(plan.index))
        || checkout.get("fulfillment_key").and_then(Value::as_str) != Some(key)
    {
        return Err(PortError::conflict(
            "fulfillment.checkout_identity_conflict",
            "fulfillment has a mismatched checkout identity",
        ));
    }
    Ok(())
}

fn fulfillment_index(fulfillment: &FulfillmentResponse) -> u32 {
    fulfillment
        .metadata
        .get("checkout")
        .and_then(|checkout| checkout.get("fulfillment_index"))
        .and_then(Value::as_u64)
        .and_then(|index| u32::try_from(index).ok())
        .unwrap_or(u32::MAX)
}

fn fulfillment_key(operation_id: Uuid, index: u32) -> String {
    format!("checkout:{operation_id}:fulfillment:{index}")
}

fn fulfillment_metadata(
    base: Value,
    operation_id: Uuid,
    order_id: Uuid,
    plan_hash: &str,
    index: u32,
    key: &str,
) -> Value {
    let mut root = object_or_empty(base);
    let mut checkout = root
        .remove("checkout")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    checkout.insert(
        "operation_id".to_string(),
        Value::String(operation_id.to_string()),
    );
    checkout.insert("order_id".to_string(), Value::String(order_id.to_string()));
    checkout.insert(
        "order_plan_hash".to_string(),
        Value::String(plan_hash.to_string()),
    );
    checkout.insert(
        "fulfillment_index".to_string(),
        Value::Number(u64::from(index).into()),
    );
    checkout.insert(
        "fulfillment_key".to_string(),
        Value::String(key.to_string()),
    );
    root.insert("checkout".to_string(), Value::Object(checkout));
    root.insert(
        "commerce_orchestration".to_string(),
        serde_json::json!({"operation": "checkout_create_fulfillment"}),
    );
    Value::Object(root)
}

fn fulfillment_item_metadata(
    base: Value,
    operation_id: Uuid,
    cart_line_item_id: Uuid,
    plan_hash: &str,
    index: u32,
) -> Value {
    let mut root = object_or_empty(base);
    root.insert(
        "checkout".to_string(),
        serde_json::json!({
            "operation_id": operation_id,
            "cart_line_item_id": cart_line_item_id,
            "order_plan_hash": plan_hash,
            "fulfillment_index": index,
        }),
    );
    Value::Object(root)
}

fn object_or_empty(value: Value) -> serde_json::Map<String, Value> {
    match value {
        Value::Object(object) => object,
        _ => Default::default(),
    }
}

fn require_operation_context(
    context: &PortContext,
    owner_operation: &'static str,
    checkout_operation_id: Uuid,
) -> Result<(), PortError> {
    let context_operation = context
        .causation_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    if context_operation != Some(checkout_operation_id) {
        let error = PortError::validation(
            "fulfillment.checkout_operation_id_invalid",
            "checkout fulfillment causation_id must match the checkout operation",
        );
        tracing::warn!(
            error = ?error,
            owner = CHECKOUT_FULFILLMENT_OWNER,
            operation = owner_operation,
            validation_phase = "causation_id",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            expected_checkout_operation_id = %checkout_operation_id,
            code = "fulfillment.checkout_operation_id_invalid",
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
            "checkout fulfillment execution received invalid causation identity"
        );
        return Err(error);
    }
    Ok(())
}

fn parse_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|cause| {
        let error = PortError::validation(
            "fulfillment.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for fulfillment ports",
        );
        tracing::warn!(
            cause = ?cause,
            error = ?error,
            owner = CHECKOUT_FULFILLMENT_OWNER,
            operation = owner_operation,
            validation_phase = "tenant_id",
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            code = "fulfillment.tenant_id_invalid",
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
            "checkout fulfillment execution received invalid tenant context"
        );
        error
    })
}

fn fulfillment_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: FulfillmentError,
) -> PortError {
    match error {
        FulfillmentError::Validation(cause) => {
            tracing::warn!(
                cause = %cause,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = owner_operation,
                code = "fulfillment.checkout_execution_validation",
                "fulfillment owner rejected checkout execution request"
            );
            PortError::validation(
                "fulfillment.checkout_execution_validation",
                "checkout fulfillment request is invalid",
            )
        }
        FulfillmentError::ShippingOptionNotFound(id) => {
            tracing::warn!(
                shipping_option_id = %id,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = owner_operation,
                code = "fulfillment.shipping_option_not_found",
                "checkout fulfillment shipping option was not found"
            );
            PortError::new(
                PortErrorKind::NotFound,
                "fulfillment.shipping_option_not_found",
                "shipping option was not found",
                false,
            )
        }
        FulfillmentError::FulfillmentNotFound(id) => {
            tracing::warn!(
                fulfillment_id = %id,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = owner_operation,
                code = "fulfillment.fulfillment_not_found",
                "checkout fulfillment resource was not found"
            );
            PortError::new(
                PortErrorKind::NotFound,
                "fulfillment.fulfillment_not_found",
                "fulfillment was not found",
                false,
            )
        }
        FulfillmentError::InvalidTransition { from, to } => {
            tracing::warn!(
                from = %from,
                to = %to,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = owner_operation,
                code = "fulfillment.checkout_execution_state_conflict",
                "fulfillment lifecycle conflicts with checkout execution"
            );
            PortError::conflict(
                "fulfillment.checkout_execution_state_conflict",
                "fulfillment lifecycle conflicts with checkout execution",
            )
        }
        FulfillmentError::Database(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                operation = owner_operation,
                code = "fulfillment.database_unavailable",
                "checkout fulfillment storage operation failed"
            );
            PortError::unavailable(
                "fulfillment.database_unavailable",
                "fulfillment storage is temporarily unavailable",
            )
        }
    }
}
