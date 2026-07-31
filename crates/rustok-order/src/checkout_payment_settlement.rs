use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AdoptLegacyCheckoutOrderIdentityRequest, CheckoutOrderIdentityPort,
    CheckoutOrderIdentitySnapshot, InProcessCheckoutOrderIdentityPort, OrderError, OrderResponse,
    OrderService, OrderStatusKind, ReadCheckoutOrderIdentityByOperationRequest,
};

const ORDER_PAYMENT_SETTLEMENT_OWNER: &str = "rustok_order.checkout_payment_settlement";
const ORDER_PAYMENT_SETTLEMENT_BOUNDARY: &str = "checkout_order_payment_settlement_port";
const SETTLE_PAYMENT_OPERATION: &str = "settle_checkout_payment";

struct OrderPaymentSettlementContextFacts {
    tenant_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

struct OrderPaymentSettlementRequestFacts {
    checkout_operation_id_non_nil: bool,
    cart_id_non_nil: bool,
    order_id_non_nil: bool,
    payment_collection_id_non_nil: bool,
    payment_reference_present: bool,
    payment_reference_length: usize,
    payment_method_present: bool,
    payment_method_length: usize,
    locale_present: bool,
    locale_length: Option<usize>,
    fallback_locale_present: bool,
    fallback_locale_length: Option<usize>,
}

#[async_trait]
pub trait CheckoutOrderPaymentSettlementPort: Send + Sync {
    async fn settle_checkout_payment(
        &self,
        context: PortContext,
        request: SettleCheckoutOrderPaymentRequest,
    ) -> Result<OrderResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettleCheckoutOrderPaymentRequest {
    pub checkout_operation_id: Uuid,
    pub cart_id: Uuid,
    pub order_id: Uuid,
    pub payment_collection_id: Uuid,
    pub payment_reference: String,
    pub payment_method: String,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

pub struct InProcessCheckoutOrderPaymentSettlementPort {
    service: OrderService,
    identity_port: Arc<dyn CheckoutOrderIdentityPort>,
}

impl InProcessCheckoutOrderPaymentSettlementPort {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self::with_identity_port(
            db.clone(),
            event_bus,
            Arc::new(InProcessCheckoutOrderIdentityPort::new(db)),
        )
    }

    pub fn with_identity_port(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    ) -> Self {
        Self {
            service: OrderService::new(db, event_bus),
            identity_port,
        }
    }

    async fn load_order(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        request: &SettleCheckoutOrderPaymentRequest,
    ) -> Result<OrderResponse, PortError> {
        match request.locale.as_deref() {
            Some(locale) => {
                self.service
                    .get_order_with_locale_fallback(
                        tenant_id,
                        request.order_id,
                        locale,
                        request.fallback_locale.as_deref(),
                    )
                    .await
            }
            None => self.service.get_order(tenant_id, request.order_id).await,
        }
        .map_err(|error| order_error_to_port_error(context, "load_checkout_order", error))
    }
}

pub fn in_process_checkout_order_payment_settlement_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutOrderPaymentSettlementPort> {
    Arc::new(InProcessCheckoutOrderPaymentSettlementPort::new(
        db, event_bus,
    ))
}

#[async_trait]
impl CheckoutOrderPaymentSettlementPort for InProcessCheckoutOrderPaymentSettlementPort {
    async fn settle_checkout_payment(
        &self,
        context: PortContext,
        request: SettleCheckoutOrderPaymentRequest,
    ) -> Result<OrderResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context, SETTLE_PAYMENT_OPERATION)?;
        let actor_id = parse_actor_id(&context, SETTLE_PAYMENT_OPERATION)?;
        require_operation_context(
            &context,
            SETTLE_PAYMENT_OPERATION,
            request.checkout_operation_id,
        )?;
        validate_request(&context, &request)?;

        let mut identity = self
            .identity_port
            .read_by_operation(
                context.clone(),
                ReadCheckoutOrderIdentityByOperationRequest {
                    checkout_operation_id: request.checkout_operation_id,
                },
            )
            .await?;
        if identity.is_none() {
            identity = self
                .identity_port
                .adopt_legacy(
                    context.clone(),
                    AdoptLegacyCheckoutOrderIdentityRequest {
                        checkout_operation_id: request.checkout_operation_id,
                        cart_id: request.cart_id,
                    },
                )
                .await?;
        }
        let identity = identity.ok_or_else(|| {
            log_missing_checkout_identity(&context, &request);
            PortError::conflict(
                "order.checkout_payment_identity_missing",
                "checkout requires manual reconciliation",
            )
        })?;
        validate_identity(&context, tenant_id, &request, &identity)?;

        let current = self.load_order(&context, tenant_id, &request).await?;
        let settled = match current.status_kind() {
            OrderStatusKind::Confirmed => self
                .service
                .mark_paid(
                    tenant_id,
                    actor_id,
                    current.id,
                    request.payment_reference.clone(),
                    request.payment_method.clone(),
                )
                .await
                .map_err(|error| {
                    order_error_to_port_error(&context, "mark_checkout_order_paid", error)
                })?,
            OrderStatusKind::Paid | OrderStatusKind::Shipped | OrderStatusKind::Delivered => {
                current
            }
            state @ (OrderStatusKind::Pending
            | OrderStatusKind::Cancelled
            | OrderStatusKind::Unknown) => {
                log_payment_settlement_lifecycle_conflict(&context, current.id, state);
                return Err(PortError::conflict(
                    "order.checkout_payment_state_conflict",
                    "checkout order lifecycle does not allow payment settlement",
                ));
            }
        };
        let payment_reference_matches =
            settled.payment_id.as_deref() == Some(request.payment_reference.as_str());
        let payment_method_matches =
            settled.payment_method.as_deref() == Some(request.payment_method.as_str());
        if !payment_reference_matches || !payment_method_matches {
            log_payment_identity_conflict(
                &context,
                &request,
                &settled,
                payment_reference_matches,
                payment_method_matches,
            );
            return Err(PortError::conflict(
                "order.checkout_payment_reference_conflict",
                "checkout order is settled by another payment identity",
            ));
        }
        Ok(settled)
    }
}

fn order_payment_settlement_context_facts(
    context: &PortContext,
) -> OrderPaymentSettlementContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    OrderPaymentSettlementContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn order_payment_settlement_request_facts(
    request: &SettleCheckoutOrderPaymentRequest,
) -> OrderPaymentSettlementRequestFacts {
    OrderPaymentSettlementRequestFacts {
        checkout_operation_id_non_nil: !request.checkout_operation_id.is_nil(),
        cart_id_non_nil: !request.cart_id.is_nil(),
        order_id_non_nil: !request.order_id.is_nil(),
        payment_collection_id_non_nil: !request.payment_collection_id.is_nil(),
        payment_reference_present: !request.payment_reference.trim().is_empty(),
        payment_reference_length: request.payment_reference.chars().count(),
        payment_method_present: !request.payment_method.trim().is_empty(),
        payment_method_length: request.payment_method.chars().count(),
        locale_present: request.locale.is_some(),
        locale_length: request
            .locale
            .as_ref()
            .map(|value| value.chars().count()),
        fallback_locale_present: request.fallback_locale.is_some(),
        fallback_locale_length: request
            .fallback_locale
            .as_ref()
            .map(|value| value.chars().count()),
    }
}

fn log_missing_checkout_identity(
    context: &PortContext,
    request: &SettleCheckoutOrderPaymentRequest,
) {
    let context_facts = order_payment_settlement_context_facts(context);
    let request_facts = order_payment_settlement_request_facts(request);
    tracing::error!(
        owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
        operation = SETTLE_PAYMENT_OPERATION,
        local_operation = "require_durable_checkout_identity",
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        checkout_operation_id_non_nil = request_facts.checkout_operation_id_non_nil,
        cart_id_non_nil = request_facts.cart_id_non_nil,
        order_id_non_nil = request_facts.order_id_non_nil,
        payment_collection_id_non_nil = request_facts.payment_collection_id_non_nil,
        payment_reference_present = request_facts.payment_reference_present,
        payment_reference_length = request_facts.payment_reference_length,
        payment_method_present = request_facts.payment_method_present,
        payment_method_length = request_facts.payment_method_length,
        locale_present = request_facts.locale_present,
        request_locale_length = ?request_facts.locale_length,
        fallback_locale_present = request_facts.fallback_locale_present,
        fallback_locale_length = ?request_facts.fallback_locale_length,
        code = "order.checkout_payment_identity_missing",
        boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
        "checkout payment settlement has no durable order identity"
    );
}

fn validate_identity(
    context: &PortContext,
    tenant_id: Uuid,
    request: &SettleCheckoutOrderPaymentRequest,
    identity: &CheckoutOrderIdentitySnapshot,
) -> Result<(), PortError> {
    let tenant_matches = identity.tenant_id == tenant_id;
    let checkout_operation_matches =
        identity.checkout_operation_id == request.checkout_operation_id;
    let order_matches = identity.order_id == request.order_id;
    let source_cart_matches = identity
        .source_cart_id
        .is_none_or(|cart_id| cart_id == request.cart_id);
    let payment_collection_matches = identity
        .payment_collection_id
        .is_none_or(|collection_id| collection_id == request.payment_collection_id);
    let valid = tenant_matches
        && checkout_operation_matches
        && order_matches
        && source_cart_matches
        && payment_collection_matches;
    if !valid {
        let context_facts = order_payment_settlement_context_facts(context);
        let request_facts = order_payment_settlement_request_facts(request);
        tracing::error!(
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
            operation = SETTLE_PAYMENT_OPERATION,
            local_operation = "validate_durable_checkout_identity",
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            tenant_matches,
            checkout_operation_matches,
            order_matches,
            source_cart_matches,
            payment_collection_matches,
            request_checkout_operation_id_non_nil = request_facts.checkout_operation_id_non_nil,
            request_cart_id_non_nil = request_facts.cart_id_non_nil,
            request_order_id_non_nil = request_facts.order_id_non_nil,
            request_payment_collection_id_non_nil = request_facts.payment_collection_id_non_nil,
            identity_checkout_operation_id_non_nil = !identity.checkout_operation_id.is_nil(),
            identity_order_id_non_nil = !identity.order_id.is_nil(),
            identity_source_cart_id_present = identity.source_cart_id.is_some(),
            identity_source_cart_id_non_nil = ?identity.source_cart_id.map(|value| !value.is_nil()),
            identity_payment_collection_id_present = identity.payment_collection_id.is_some(),
            identity_payment_collection_id_non_nil = ?identity
                .payment_collection_id
                .map(|value| !value.is_nil()),
            code = "order.checkout_payment_identity_conflict",
            boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            "checkout order identity conflicts with payment settlement"
        );
        return Err(PortError::conflict(
            "order.checkout_payment_identity_conflict",
            "checkout order identity conflicts with the payment settlement request",
        ));
    }
    Ok(())
}

fn log_payment_settlement_lifecycle_conflict(
    context: &PortContext,
    order_id: Uuid,
    order_state: OrderStatusKind,
) {
    let context_facts = order_payment_settlement_context_facts(context);
    tracing::warn!(
        owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
        operation = SETTLE_PAYMENT_OPERATION,
        local_operation = "validate_payment_settlement_lifecycle",
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        order_id_non_nil = !order_id.is_nil(),
        order_state = ?order_state,
        code = "order.checkout_payment_state_conflict",
        boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
        "checkout order lifecycle does not allow payment settlement"
    );
}

fn log_payment_identity_conflict(
    context: &PortContext,
    request: &SettleCheckoutOrderPaymentRequest,
    settled: &OrderResponse,
    payment_reference_matches: bool,
    payment_method_matches: bool,
) {
    let context_facts = order_payment_settlement_context_facts(context);
    let request_facts = order_payment_settlement_request_facts(request);
    tracing::error!(
        owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
        operation = SETTLE_PAYMENT_OPERATION,
        local_operation = "validate_settled_payment_identity",
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        order_id_non_nil = !settled.id.is_nil(),
        payment_reference_matches,
        payment_method_matches,
        requested_payment_reference_present = request_facts.payment_reference_present,
        requested_payment_reference_length = request_facts.payment_reference_length,
        requested_payment_method_present = request_facts.payment_method_present,
        requested_payment_method_length = request_facts.payment_method_length,
        settled_payment_reference_present = settled.payment_id.is_some(),
        settled_payment_reference_length = ?settled
            .payment_id
            .as_ref()
            .map(|value| value.chars().count()),
        settled_payment_method_present = settled.payment_method.is_some(),
        settled_payment_method_length = ?settled
            .payment_method
            .as_ref()
            .map(|value| value.chars().count()),
        code = "order.checkout_payment_reference_conflict",
        boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
        "checkout order is settled by another payment identity"
    );
}

fn validate_request(
    context: &PortContext,
    request: &SettleCheckoutOrderPaymentRequest,
) -> Result<(), PortError> {
    if request.checkout_operation_id.is_nil()
        || request.cart_id.is_nil()
        || request.order_id.is_nil()
        || request.payment_collection_id.is_nil()
        || request.payment_reference.trim().is_empty()
        || request.payment_method.trim().is_empty()
    {
        let context_facts = order_payment_settlement_context_facts(context);
        let request_facts = order_payment_settlement_request_facts(request);
        tracing::warn!(
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
            operation = SETTLE_PAYMENT_OPERATION,
            local_operation = "validate_request",
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            checkout_operation_id_non_nil = request_facts.checkout_operation_id_non_nil,
            cart_id_non_nil = request_facts.cart_id_non_nil,
            order_id_non_nil = request_facts.order_id_non_nil,
            payment_collection_id_non_nil = request_facts.payment_collection_id_non_nil,
            payment_reference_present = request_facts.payment_reference_present,
            payment_reference_length = request_facts.payment_reference_length,
            payment_method_present = request_facts.payment_method_present,
            payment_method_length = request_facts.payment_method_length,
            locale_present = request_facts.locale_present,
            request_locale_length = ?request_facts.locale_length,
            fallback_locale_present = request_facts.fallback_locale_present,
            fallback_locale_length = ?request_facts.fallback_locale_length,
            code = "order.checkout_payment_request_invalid",
            boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            "checkout payment settlement rejected invalid owner identities"
        );
        return Err(PortError::validation(
            "order.checkout_payment_request_invalid",
            "checkout payment settlement request is invalid",
        ));
    }
    Ok(())
}

fn require_operation_context(
    context: &PortContext,
    operation: &'static str,
    checkout_operation_id: Uuid,
) -> Result<(), PortError> {
    let context_operation = context
        .causation_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    if context_operation != Some(checkout_operation_id) {
        let context_facts = order_payment_settlement_context_facts(context);
        tracing::warn!(
            owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
            operation,
            local_operation = "validate_causation_context",
            correlation_id = %context.correlation_id,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            checkout_operation_id_non_nil = !checkout_operation_id.is_nil(),
            causation_matches = false,
            code = "order.checkout_payment_causation_invalid",
            boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
            "checkout payment settlement received invalid causation identity"
        );
        return Err(PortError::validation(
            "order.checkout_payment_causation_invalid",
            "checkout operation context is invalid",
        ));
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        log_context_parse_rejection(
            context,
            operation,
            "tenant_id",
            context.tenant_id.chars().count(),
            "order.tenant_id_invalid",
            &error,
        );
        PortError::validation(
            "order.tenant_id_invalid",
            "order request context is invalid",
        )
    })
}

fn parse_actor_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|error| {
        log_context_parse_rejection(
            context,
            operation,
            "actor_id",
            context.actor.id.chars().count(),
            "order.actor_id_invalid",
            &error,
        );
        PortError::validation("order.actor_id_invalid", "order request context is invalid")
    })
}

fn log_context_parse_rejection<E: std::fmt::Debug>(
    context: &PortContext,
    operation: &'static str,
    field: &'static str,
    value_length: usize,
    code: &'static str,
    error: &E,
) {
    let context_facts = order_payment_settlement_context_facts(context);
    tracing::warn!(
        error = ?error,
        owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
        operation,
        local_operation = "validate_owner_context",
        field,
        value_length,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        code,
        boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
        "order port received invalid request context"
    );
}

fn log_order_payment_owner_warning(
    context: &PortContext,
    operation: &'static str,
    local_operation: &'static str,
    code: &'static str,
    resource: Option<&'static str>,
    resource_id: Option<Uuid>,
    internal_cause: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) {
    let context_facts = order_payment_settlement_context_facts(context);
    tracing::warn!(
        owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
        operation,
        local_operation,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        resource = ?resource,
        resource_id_present = resource_id.is_some(),
        resource_id_non_nil = ?resource_id.map(|value| !value.is_nil()),
        internal_cause_present = internal_cause.is_some(),
        internal_cause_length = ?internal_cause.map(|value| value.chars().count()),
        from = ?from,
        to = ?to,
        code,
        boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
        "order checkout payment settlement owner outcome retained safe context"
    );
}

fn log_order_payment_owner_error<E: std::fmt::Debug>(
    context: &PortContext,
    operation: &'static str,
    local_operation: &'static str,
    code: &'static str,
    error: &E,
) {
    let context_facts = order_payment_settlement_context_facts(context);
    tracing::error!(
        error = ?error,
        owner = ORDER_PAYMENT_SETTLEMENT_OWNER,
        operation,
        local_operation,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        code,
        boundary = ORDER_PAYMENT_SETTLEMENT_BOUNDARY,
        "order checkout payment settlement owner technical outcome retained safe context"
    );
}

fn order_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: OrderError,
) -> PortError {
    match error {
        OrderError::Database(error) => {
            log_order_payment_owner_error(
                context,
                operation,
                "owner_storage",
                "order.database_unavailable",
                &error,
            );
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(order_id) => {
            log_order_payment_owner_warning(
                context,
                operation,
                "load_order",
                "order.order_not_found",
                Some("order"),
                Some(order_id),
                None,
                None,
                None,
            );
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(cause) => {
            log_order_payment_owner_warning(
                context,
                operation,
                "validate_owner_request",
                "order.checkout_payment_validation",
                None,
                None,
                Some(cause.as_str()),
                None,
                None,
            );
            PortError::validation(
                "order.checkout_payment_validation",
                "checkout order payment settlement request is invalid",
            )
        }
        OrderError::InvalidTransition { from, to } => {
            log_order_payment_owner_warning(
                context,
                operation,
                "apply_payment_settlement_state",
                "order.checkout_payment_state_conflict",
                None,
                None,
                None,
                Some(from.as_str()),
                Some(to.as_str()),
            );
            PortError::conflict(
                "order.checkout_payment_state_conflict",
                "order lifecycle conflicts with payment settlement",
            )
        }
        OrderError::OrderReturnNotFound(return_id) => {
            log_order_payment_owner_warning(
                context,
                operation,
                "load_related_order_resource",
                "order.related_resource_not_found",
                Some("order_return"),
                Some(return_id),
                None,
                None,
                None,
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::OrderChangeNotFound(change_id) => {
            log_order_payment_owner_warning(
                context,
                operation,
                "load_related_order_resource",
                "order.related_resource_not_found",
                Some("order_change"),
                Some(change_id),
                None,
                None,
                None,
            );
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::Core(error) => {
            log_order_payment_owner_error(
                context,
                operation,
                "owner_invariant",
                "order.invariant_violation",
                &error,
            );
            PortError::invariant_violation(
                "order.invariant_violation",
                "order payment settlement failed an internal invariant",
            )
        }
    }
}
