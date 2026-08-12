use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::{PaymentCollectionResponse, PaymentCollectionStatusKind};

const PAYMENT_COLLECTION_PORT_BOUNDARY: &str = "payment_collection_port";
const PAYMENT_COLLECTION_OWNER: &str = "rustok_payment";
const CREATE_OR_REUSE_COLLECTION_OPERATION: &str = "create_or_reuse_collection";
const READ_COLLECTION_STATUS_OPERATION: &str = "read_collection_status";

/// Transport-neutral owner boundary for payment collection create/reuse flows.
#[async_trait]
pub trait PaymentCollectionPort: Send + Sync {
    async fn create_or_reuse_collection(
        &self,
        context: PortContext,
        request: PaymentCollectionCreateOrReuseRequest,
    ) -> Result<PaymentCollectionResponse, PortError>;

    async fn read_collection_status(
        &self,
        context: PortContext,
        request: PaymentCollectionStatusRequest,
    ) -> Result<PaymentCollectionStatusSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentCollectionCreateOrReuseRequest {
    pub cart_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub customer_id: Option<Uuid>,
    pub currency_code: String,
    pub amount: Decimal,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentCollectionStatusRequest {
    pub collection_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentCollectionStatusSnapshot {
    pub collection_id: Uuid,
    pub status: String,
    pub currency_code: String,
    pub amount: Decimal,
    pub authorized_amount: Decimal,
    pub captured_amount: Decimal,
    pub provider_id: Option<String>,
}

impl PaymentCollectionStatusSnapshot {
    pub fn from_response(response: &PaymentCollectionResponse) -> Self {
        Self {
            collection_id: response.id,
            status: response.status.clone(),
            currency_code: response.currency_code.clone(),
            amount: response.amount,
            authorized_amount: response.authorized_amount,
            captured_amount: response.captured_amount,
            provider_id: response.provider_id.clone(),
        }
    }

    pub fn status_kind(&self) -> PaymentCollectionStatusKind {
        PaymentCollectionStatusKind::from_raw(self.status.as_str())
    }
}

#[async_trait]
impl PaymentCollectionPort for crate::PaymentService {
    async fn create_or_reuse_collection(
        &self,
        context: PortContext,
        request: PaymentCollectionCreateOrReuseRequest,
    ) -> Result<PaymentCollectionResponse, PortError> {
        let owner_operation = CREATE_OR_REUSE_COLLECTION_OPERATION;
        require_payment_collection_write_admission(&context, owner_operation)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;

        if let Some(cart_id) = request.cart_id
            && let Some(collection) = self
                .find_reusable_collection_by_cart(tenant_id, cart_id)
                .await
                .map_err(|error| {
                    payment_error_to_port_error(
                        &context,
                        "create_or_reuse_collection.read_existing",
                        error,
                    )
                })?
        {
            return Ok(collection);
        }

        let cart_id = request.cart_id;
        let create_result = self
            .create_collection(
                tenant_id,
                crate::CreatePaymentCollectionInput {
                    cart_id,
                    order_id: request.order_id,
                    customer_id: request.customer_id,
                    currency_code: request.currency_code,
                    amount: request.amount,
                    metadata: request.metadata,
                },
            )
            .await;

        match create_result {
            Ok(collection) => Ok(collection),
            Err(create_error) => {
                if let Some(cart_id) = cart_id
                    && let Some(collection) = self
                        .find_reusable_collection_by_cart(tenant_id, cart_id)
                        .await
                        .map_err(|error| {
                            payment_error_to_port_error(
                                &context,
                                "create_or_reuse_collection.adopt_race",
                                error,
                            )
                        })?
                {
                    return Ok(collection);
                }
                Err(payment_error_to_port_error(
                    &context,
                    "create_or_reuse_collection.create",
                    create_error,
                ))
            }
        }
    }

    async fn read_collection_status(
        &self,
        context: PortContext,
        request: PaymentCollectionStatusRequest,
    ) -> Result<PaymentCollectionStatusSnapshot, PortError> {
        let owner_operation = READ_COLLECTION_STATUS_OPERATION;
        require_payment_collection_read_admission(&context, owner_operation)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let response = self
            .get_collection(tenant_id, request.collection_id)
            .await
            .map_err(|error| payment_error_to_port_error(&context, owner_operation, error))?;
        Ok(PaymentCollectionStatusSnapshot::from_response(&response))
    }
}

fn require_payment_collection_read_admission(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .inspect_err(|error| {
            log_payment_collection_admission_rejection(context, owner_operation, "policy", error);
        })
}

fn require_payment_collection_write_admission(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::write())
        .inspect_err(|error| {
            log_payment_collection_admission_rejection(context, owner_operation, "policy", error);
        })?;
    context.require_write_semantics().inspect_err(|error| {
        log_payment_collection_admission_rejection(
            context,
            owner_operation,
            "write_semantics",
            error,
        );
    })
}

fn log_payment_collection_admission_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    admission: &'static str,
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
            owner = PAYMENT_COLLECTION_OWNER,
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
            operation = owner_operation,
            admission,
            code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
            retryable = error.retryable,
            boundary = PAYMENT_COLLECTION_PORT_BOUNDARY,
            "payment collection admission failed"
        );
    } else {
        tracing::warn!(
            owner = PAYMENT_COLLECTION_OWNER,
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
            operation = owner_operation,
            admission,
            code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
            retryable = error.retryable,
            boundary = PAYMENT_COLLECTION_PORT_BOUNDARY,
            "payment collection admission was rejected"
        );
    }
}

fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|parse_error| {
        let error = PortError::validation(
            "payment.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for payment ports",
        );
        let parse_error_type = std::any::type_name_of_val(&parse_error);
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
        let error_kind = "validation";

        tracing::warn!(
            parse_error_type,
            tenant_id_parse_failed = true,
            owner = PAYMENT_COLLECTION_OWNER,
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
            operation = owner_operation,
            validation = "tenant_id",
            code = %error.code,
            internal_message_present,
            internal_message_length,
            error_kind,
            retryable = error.retryable,
            boundary = PAYMENT_COLLECTION_PORT_BOUNDARY,
            "payment collection tenant context was rejected"
        );
        error
    })
}

#[derive(Debug)]
struct PaymentCollectionOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn payment_collection_owner_error_facts(
    error: &crate::PaymentError,
) -> PaymentCollectionOwnerErrorFacts {
    let (
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    ) = match error {
        crate::PaymentError::Validation(value) => {
            ("validation", 1, value.chars().count(), 0, 0, false)
        }
        crate::PaymentError::PaymentCollectionNotFound(id) => (
            "payment_collection_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        crate::PaymentError::PaymentNotFound(id) => (
            "payment_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        crate::PaymentError::RefundNotFound(id) => (
            "refund_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        crate::PaymentError::InvalidTransition { from, to } => (
            "invalid_transition",
            2,
            from.chars().count() + to.chars().count(),
            0,
            0,
            false,
        ),
        crate::PaymentError::ProviderUnavailable {
            provider_id,
            operation,
        } => (
            "provider_unavailable",
            2,
            provider_id.chars().count() + operation.chars().count(),
            0,
            0,
            false,
        ),
        crate::PaymentError::ProviderRejected {
            provider_id,
            operation,
        } => (
            "provider_rejected",
            2,
            provider_id.chars().count() + operation.chars().count(),
            0,
            0,
            false,
        ),
        crate::PaymentError::ProviderInvalidResponse {
            provider_id,
            operation,
        } => (
            "provider_invalid_response",
            2,
            provider_id.chars().count() + operation.chars().count(),
            0,
            0,
            false,
        ),
        crate::PaymentError::ProviderOutcomeUnknown {
            provider_id,
            operation,
        } => (
            "provider_outcome_unknown",
            2,
            provider_id.chars().count() + operation.chars().count(),
            0,
            0,
            false,
        ),
        crate::PaymentError::ProviderConfiguration { provider_id } => (
            "provider_configuration",
            1,
            provider_id.chars().count(),
            0,
            0,
            false,
        ),
        crate::PaymentError::Database(_) => ("database", 0, 0, 0, 0, true),
    };

    PaymentCollectionOwnerErrorFacts {
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    }
}

fn payment_collection_owner_error_code(error: &crate::PaymentError) -> &'static str {
    match error {
        crate::PaymentError::Validation(_) => "payment.validation",
        crate::PaymentError::PaymentCollectionNotFound(_) => "payment.collection_not_found",
        crate::PaymentError::PaymentNotFound(_) => "payment.payment_not_found",
        crate::PaymentError::RefundNotFound(_) => "payment.refund_not_found",
        crate::PaymentError::InvalidTransition { .. } => "payment.invalid_transition",
        crate::PaymentError::ProviderUnavailable { .. } => "payment.provider_unavailable",
        crate::PaymentError::ProviderRejected { .. } => "payment.provider_rejected",
        crate::PaymentError::ProviderInvalidResponse { .. } => "payment.provider_invalid_response",
        crate::PaymentError::ProviderOutcomeUnknown { .. } => "payment.provider_outcome_unknown",
        crate::PaymentError::ProviderConfiguration { .. } => "payment.provider_not_configured",
        crate::PaymentError::Database(_) => "payment.database_unavailable",
    }
}

fn payment_collection_owner_error_is_technical(error: &crate::PaymentError) -> bool {
    matches!(
        error,
        crate::PaymentError::ProviderUnavailable { .. }
            | crate::PaymentError::ProviderInvalidResponse { .. }
            | crate::PaymentError::ProviderOutcomeUnknown { .. }
            | crate::PaymentError::ProviderConfiguration { .. }
            | crate::PaymentError::Database(_)
    )
}

fn payment_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: crate::PaymentError,
) -> PortError {
    let code = payment_collection_owner_error_code(&error);
    let technical_failure = payment_collection_owner_error_is_technical(&error);
    let error_facts = payment_collection_owner_error_facts(&error);
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

    if technical_failure {
        tracing::error!(
            owner = PAYMENT_COLLECTION_OWNER,
            owner_error_variant = error_facts.error_variant,
            owner_error_text_field_count = error_facts.text_field_count,
            owner_error_text_total_length = error_facts.text_total_length,
            owner_error_uuid_field_count = error_facts.uuid_field_count,
            owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
            owner_error_opaque_payload_present = error_facts.opaque_payload_present,
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
            operation = owner_operation,
            code,
            boundary = PAYMENT_COLLECTION_PORT_BOUNDARY,
            "payment collection owner operation failed"
        );
    } else {
        tracing::warn!(
            owner = PAYMENT_COLLECTION_OWNER,
            owner_error_variant = error_facts.error_variant,
            owner_error_text_field_count = error_facts.text_field_count,
            owner_error_text_total_length = error_facts.text_total_length,
            owner_error_uuid_field_count = error_facts.uuid_field_count,
            owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
            owner_error_opaque_payload_present = error_facts.opaque_payload_present,
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
            operation = owner_operation,
            code,
            boundary = PAYMENT_COLLECTION_PORT_BOUNDARY,
            "payment collection owner operation was rejected"
        );
    }

    match error {
        crate::PaymentError::Validation(_) => {
            PortError::validation("payment.validation", "payment request is invalid")
        }
        crate::PaymentError::PaymentCollectionNotFound(_) => PortError::not_found(
            "payment.collection_not_found",
            "payment collection was not found",
        ),
        crate::PaymentError::PaymentNotFound(_) => {
            PortError::not_found("payment.payment_not_found", "payment was not found")
        }
        crate::PaymentError::RefundNotFound(_) => {
            PortError::not_found("payment.refund_not_found", "refund was not found")
        }
        crate::PaymentError::InvalidTransition { .. } => PortError::conflict(
            "payment.invalid_transition",
            "payment lifecycle conflicts with the requested operation",
        ),
        crate::PaymentError::ProviderUnavailable { .. } => PortError::unavailable(
            "payment.provider_unavailable",
            "payment provider is temporarily unavailable",
        ),
        crate::PaymentError::ProviderRejected { .. } => PortError::conflict(
            "payment.provider_rejected",
            "payment provider rejected the requested operation",
        ),
        crate::PaymentError::ProviderInvalidResponse { .. } => PortError::new(
            PortErrorKind::InvariantViolation,
            "payment.provider_invalid_response",
            "payment provider response could not be applied safely",
            false,
        ),
        crate::PaymentError::ProviderOutcomeUnknown { .. } => PortError::new(
            PortErrorKind::Conflict,
            "payment.provider_outcome_unknown",
            "payment provider outcome requires reconciliation",
            false,
        ),
        crate::PaymentError::ProviderConfiguration { .. } => PortError::new(
            PortErrorKind::InvariantViolation,
            "payment.provider_not_configured",
            "payment provider is not configured for the requested operation",
            false,
        ),
        crate::PaymentError::Database(_) => PortError::unavailable(
            "payment.database_unavailable",
            "payment storage is temporarily unavailable",
        ),
    }
}
