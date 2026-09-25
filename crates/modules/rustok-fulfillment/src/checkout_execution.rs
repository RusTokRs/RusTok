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
use crate::services::fulfillment::CheckoutFulfillmentRecord;

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

        let mut records = Vec::with_capacity(request.plans.len());
        for plan in &request.plans {
            let input = build_input(&request, plan);
            let existing = self
                .find_checkout_fulfillment(
                    context,
                    ENSURE_OPERATION,
                    "find_checkout_fulfillment_before_create",
                    tenant_id,
                    request.checkout_operation_id,
                    plan.index,
                )
                .await?;

            let record = match existing {
                Some(existing) => existing,
                None => match self
                    .service
                    .create_checkout_fulfillment(
                        tenant_id,
                        input,
                        request.checkout_operation_id,
                        plan.index,
                        request.order_plan_hash.as_str(),
                    )
                    .await
                {
                    Ok(created) => CheckoutFulfillmentRecord {
                        index: plan.index,
                        order_id: created.order_id,
                        plan_hash: Some(request.order_plan_hash.clone()),
                        fulfillment: created,
                    },
                    Err(error) => {
                        let adopted = self
                            .find_checkout_fulfillment(
                                context,
                                ENSURE_OPERATION,
                                "adopt_checkout_fulfillment_after_create_error",
                                tenant_id,
                                request.checkout_operation_id,
                                plan.index,
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
                &record,
                FulfillmentExpectation {
                    tenant_id,
                    order_id: request.order_id,
                    customer_id: request.customer_id,
                    plan_hash: request.order_plan_hash.as_str(),
                    plan,
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
            records.push(record);
        }

        records.sort_by_key(|record| record.index);
        Ok(records
            .into_iter()
            .map(|record| record.fulfillment)
            .collect())
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

        let records = self
            .list_checkout_fulfillments(
                context,
                READ_OPERATION,
                "list_checkout_fulfillments_for_read",
                tenant_id,
                request.checkout_operation_id,
            )
            .await?;
        let mut by_index = BTreeMap::new();
        for record in records {
            if by_index.insert(record.index, record).is_some() {
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
            let record = by_index.remove(&plan.index).ok_or_else(|| {
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
                &record,
                FulfillmentExpectation {
                    tenant_id,
                    order_id: request.order_id,
                    customer_id: request.customer_id,
                    plan_hash: request.order_plan_hash.as_str(),
                    plan,
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
            result.push(record.fulfillment);
        }
        Ok(result)
    }

    async fn list_checkout_fulfillments(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        service_operation: &'static str,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> Result<Vec<CheckoutFulfillmentRecord>, PortError> {
        self.service
            .list_checkout_fulfillments(tenant_id, checkout_operation_id)
            .await
            .map_err(|error| fulfillment_error_to_port_error(context, service_operation, error))
            .map(|records| {
                tracing::debug!(
                    boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
                    owner_operation,
                    record_count = records.len(),
                    "checkout fulfillment typed identity lookup completed"
                );
                records
            })
    }

    async fn find_checkout_fulfillment(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
        service_operation: &'static str,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
        checkout_fulfillment_index: u32,
    ) -> Result<Option<CheckoutFulfillmentRecord>, PortError> {
        self.service
            .find_checkout_fulfillment(
                tenant_id,
                checkout_operation_id,
                checkout_fulfillment_index,
            )
            .await
            .map_err(|error| fulfillment_error_to_port_error(context, service_operation, error))
            .map(|record| {
                tracing::debug!(
                    boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
                    owner_operation,
                    record_present = record.is_some(),
                    "checkout fulfillment typed identity lookup completed"
                );
                record
            })
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
                        item.cart_line_item_id,
                    ),
                })
                .collect(),
        ),
        metadata: fulfillment_metadata(plan.metadata.clone()),
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
) -> Result<(struct FulfillmentExpectation<'a> {
    tenant_id: Uuid,
    order_id: Uuid,
    customer_id: Option<Uuid>,
    plan_hash: &'a str,
    plan: &'a CheckoutFulfillmentCommand,
}

fn validate_fulfillment(
    record: &CheckoutFulfillmentRecord,
    expected: FulfillmentExpectation<'_>,
) -> Result<(), PortError> {
    let FulfillmentExpectation {
        tenant_id,
        order_id,
        customer_id,
        plan_hash,
        plan,
    } = expected;

    if record.index != plan.index
        || record.order_id != order_id
        || record.plan_hash.as_deref() != Some(plan_hash)
    {
        return Err(PortError::conflict(
            "fulfillment.checkout_identity_conflict",
            "fulfillment has a mismatched checkout identity",
        ));
    }

    let fulfillment = &record.fulfillment;
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
    Ok(())
}

fn fulfillment_metadata(base: Value) -> Value {
    let mut root = strip_checkout_identity_metadata(base);
    root.insert(
        "commerce_orchestration".to_string(),
        serde_json::json!({"operation": "checkout_create_fulfillment"}),
    );
    Value::Object(root)
}

fn fulfillment_item_metadata(base: Value, cart_line_item_id: Uuid) -> Value {
    let mut root = strip_checkout_identity_metadata(base);
    let mut checkout = root
        .remove("checkout")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    checkout.insert(
        "cart_line_item_id".to_string(),
        Value::String(cart_line_item_id.to_string()),
    );
    if checkout.is_empty() {
        root.remove("checkout");
    } else {
        root.insert("checkout".to_string(), Value::Object(checkout));
    }
    Value::Object(root)
}

fn strip_checkout_identity_metadata(value: Value) -> serde_json::Map<String, Value> {
    let mut root = object_or_empty(value);
    if let Some(Value::Object(mut checkout)) = root.remove("checkout") {
        for key in [
            "operation_id",
            "order_id",
            "order_plan_hash",
            "fulfillment_index",
            "fulfillment_key",
        ] {
            checkout.remove(key);
        }
        if !checkout.is_empty() {
            root.insert("checkout".to_string(), Value::Object(checkout));
        }
    }
    root
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
    let causation_id_present = context.causation_id.is_some();
    let causation_id_length = context
        .causation_id
        .as_ref()
        .map(|value| value.chars().count());
    let context_operation = context
        .causation_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    let causation_id_parse_succeeded = context_operation.is_some();
    let causation_id_matches_expected = context_operation == Some(checkout_operation_id);
    if !causation_id_matches_expected {
        let error = PortError::validation(
            "fulfillment.checkout_operation_id_invalid",
            "checkout fulfillment causation_id must match the checkout operation",
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
        let expected_checkout_operation_id_non_nil = !checkout_operation_id.is_nil();
        let internal_message_present = !error.message.trim().is_empty();
        let internal_message_length = error.message.chars().count();
        let error_kind = "validation";
        tracing::warn!(
            owner = CHECKOUT_FULFILLMENT_OWNER,
            operation = owner_operation,
            validation_phase = "causation_id",
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
            causation_id_parse_succeeded,
            causation_id_matches_expected,
            traceparent_present,
            traceparent_length = ?traceparent_length,
            idempotency_key_present,
            idempotency_key_length = ?idempotency_key_length,
            deadline_ms = ?context.deadline_ms,
            expected_checkout_operation_id_non_nil,
            code = "fulfillment.checkout_operation_id_invalid",
            internal_code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
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
        let parse_cause_type = std::any::type_name_of_val(&cause);
        let tenant_id_length = context.tenant_id.chars().count();
        let tenant_id_parse_failed = true;
        let actor_kind = match &context.actor.kind {
            rustok_api::PortActorKind::User => "user",
            rustok_api::PortActorKind::Service => "service",
            rustok_api::PortActorKind::System => "system",
        };
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
        let error_kind = "validation";
        tracing::warn!(
            parse_cause_type,
            owner = CHECKOUT_FULFILLMENT_OWNER,
            operation = owner_operation,
            validation_phase = "tenant_id",
            correlation_id = %context.correlation_id,
            tenant_id_length,
            tenant_id_parse_failed,
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
            code = "fulfillment.tenant_id_invalid",
            internal_code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
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

    match error {
        FulfillmentError::Validation(cause) => {
            let validation_cause_present = !cause.trim().is_empty();
            let validation_cause_length = cause.chars().count();
            tracing::warn!(
                owner = CHECKOUT_FULFILLMENT_OWNER,
                operation = owner_operation,
                owner_error_kind = "validation",
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
                validation_cause_present,
                validation_cause_length,
                code = "fulfillment.checkout_execution_validation",
                boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
                "fulfillment owner rejected checkout execution request"
            );
            PortError::validation(
                "fulfillment.checkout_execution_validation",
                "checkout fulfillment request is invalid",
            )
        }
        FulfillmentError::ShippingOptionNotFound(id) => {
            let shipping_option_id_non_nil = !id.is_nil();
            tracing::warn!(
                owner = CHECKOUT_FULFILLMENT_OWNER,
                operation = owner_operation,
                owner_error_kind = "shipping_option_not_found",
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
                shipping_option_id_non_nil,
                code = "fulfillment.shipping_option_not_found",
                boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
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
            let fulfillment_id_non_nil = !id.is_nil();
            tracing::warn!(
                owner = CHECKOUT_FULFILLMENT_OWNER,
                operation = owner_operation,
                owner_error_kind = "fulfillment_not_found",
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
                fulfillment_id_non_nil,
                code = "fulfillment.fulfillment_not_found",
                boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
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
            let transition_from_present = !from.trim().is_empty();
            let transition_from_length = from.chars().count();
            let transition_to_present = !to.trim().is_empty();
            let transition_to_length = to.chars().count();
            let transition_changes_state = from != to;
            tracing::warn!(
                owner = CHECKOUT_FULFILLMENT_OWNER,
                operation = owner_operation,
                owner_error_kind = "invalid_transition",
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
                transition_from_present,
                transition_from_length,
                transition_to_present,
                transition_to_length,
                transition_changes_state,
                code = "fulfillment.checkout_execution_state_conflict",
                boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
                "fulfillment lifecycle conflicts with checkout execution"
            );
            PortError::conflict(
                "fulfillment.checkout_execution_state_conflict",
                "fulfillment lifecycle conflicts with checkout execution",
            )
        }
        FulfillmentError::Database(error) => {
            let database_error_type = std::any::type_name_of_val(&error);
            tracing::error!(
                owner = CHECKOUT_FULFILLMENT_OWNER,
                operation = owner_operation,
                owner_error_kind = "database",
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
                database_error_type,
                code = "fulfillment.database_unavailable",
                boundary = CHECKOUT_FULFILLMENT_BOUNDARY,
                "checkout fulfillment storage operation failed"
            );
            PortError::unavailable(
                "fulfillment.database_unavailable",
                "fulfillment storage is temporarily unavailable",
            )
        }
    }
}
