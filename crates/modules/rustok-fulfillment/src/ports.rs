use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const FULFILLMENT_OWNER: &str = "rustok_fulfillment";
const SHIPPING_SELECTION_BOUNDARY: &str = "fulfillment_shipping_selection_port";

/// Transport-neutral owner boundary for checkout shipping selection.
#[async_trait]
pub trait ShippingSelectionPort: Send + Sync {
    async fn list_seller_shipping_options(
        &self,
        context: PortContext,
        request: ListSellerShippingOptionsRequest,
    ) -> Result<SellerShippingOptionsSnapshot, PortError>;

    async fn select_shipping_option(
        &self,
        context: PortContext,
        request: SelectShippingOptionPortRequest,
    ) -> Result<SelectedShippingOptionSnapshot, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListSellerShippingOptionsRequest {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub shipping_profile_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectShippingOptionPortRequest {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub shipping_option_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SellerShippingOptionsSnapshot {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub options: Vec<ShippingOptionProjection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShippingOptionProjection {
    pub id: Uuid,
    pub provider_id: String,
    pub name: String,
    pub currency_code: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedShippingOptionSnapshot {
    pub cart_id: Uuid,
    pub seller_id: Option<String>,
    pub option: ShippingOptionProjection,
}

struct FulfillmentPortContextFacts {
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

struct FulfillmentOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

#[async_trait]
impl ShippingSelectionPort for crate::FulfillmentService {
    async fn list_seller_shipping_options(
        &self,
        context: PortContext,
        request: ListSellerShippingOptionsRequest,
    ) -> Result<SellerShippingOptionsSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, "list_seller_shipping_options")?;
        let options = self
            .list_shipping_options(tenant_id, Some(&context.locale), Some(&context.locale))
            .await
            .map_err(|error| {
                fulfillment_error_to_port_error(&context, "list_seller_shipping_options", error)
            })?
            .into_iter()
            .filter(|option| {
                request
                    .shipping_profile_slug
                    .as_deref()
                    .map(|profile| {
                        option
                            .allowed_shipping_profile_slugs
                            .as_ref()
                            .map(|profiles| profiles.iter().any(|item| item == profile))
                            .unwrap_or(true)
                    })
                    .unwrap_or(true)
            })
            .map(ShippingOptionProjection::from_response)
            .collect();

        Ok(SellerShippingOptionsSnapshot {
            cart_id: request.cart_id,
            seller_id: request.seller_id,
            options,
        })
    }

    async fn select_shipping_option(
        &self,
        context: PortContext,
        request: SelectShippingOptionPortRequest,
    ) -> Result<SelectedShippingOptionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, "select_shipping_option")?;
        let option = self
            .get_shipping_option(
                tenant_id,
                request.shipping_option_id,
                Some(&context.locale),
                Some(&context.locale),
            )
            .await
            .map_err(|error| {
                fulfillment_error_to_port_error(&context, "select_shipping_option", error)
            })?;

        Ok(SelectedShippingOptionSnapshot {
            cart_id: request.cart_id,
            seller_id: request.seller_id,
            option: ShippingOptionProjection::from_response(option),
        })
    }
}

impl ShippingOptionProjection {
    pub fn from_response(response: crate::ShippingOptionResponse) -> Self {
        Self {
            id: response.id,
            provider_id: response.provider_id,
            name: response.name,
            currency_code: response.currency_code,
            amount: response.amount,
        }
    }
}

fn fulfillment_port_context_facts(context: &PortContext) -> FulfillmentPortContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    FulfillmentPortContextFacts {
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

fn fulfillment_owner_error_facts(error: &crate::FulfillmentError) -> FulfillmentOwnerErrorFacts {
    let (
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    ) = match error {
        crate::FulfillmentError::Validation(value) => {
            ("validation", 1, value.chars().count(), 0, 0, false)
        }
        crate::FulfillmentError::ShippingOptionNotFound(id) => (
            "shipping_option_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        crate::FulfillmentError::FulfillmentNotFound(id) => (
            "fulfillment_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        crate::FulfillmentError::InvalidTransition { from, to } => (
            "invalid_transition",
            2,
            from.chars().count() + to.chars().count(),
            0,
            0,
            false,
        ),
        crate::FulfillmentError::Database(_) => ("database", 0, 0, 0, 0, true),
    };
    FulfillmentOwnerErrorFacts {
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    }
}

fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        let context_facts = fulfillment_port_context_facts(context);
        tracing::warn!(
            owner = FULFILLMENT_OWNER,
            operation = owner_operation,
            correlation_id = %context.correlation_id,
            tenant_id_parse_failed = true,
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
            code = "fulfillment.context_invalid",
            boundary = SHIPPING_SELECTION_BOUNDARY,
            "fulfillment shipping selection request context is invalid"
        );
        PortError::validation(
            "fulfillment.context_invalid",
            "fulfillment request context is invalid",
        )
    })
}

fn fulfillment_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: crate::FulfillmentError,
) -> PortError {
    let error_facts = fulfillment_owner_error_facts(&error);
    let (kind, code, message, retryable, technical_failure) = match &error {
        crate::FulfillmentError::Validation(_) => (
            PortErrorKind::Validation,
            "fulfillment.validation",
            "fulfillment request is invalid",
            false,
            false,
        ),
        crate::FulfillmentError::ShippingOptionNotFound(_) => (
            PortErrorKind::NotFound,
            "fulfillment.shipping_option_not_found",
            "shipping option was not found",
            false,
            false,
        ),
        crate::FulfillmentError::FulfillmentNotFound(_) => (
            PortErrorKind::NotFound,
            "fulfillment.fulfillment_not_found",
            "fulfillment was not found",
            false,
            false,
        ),
        crate::FulfillmentError::InvalidTransition { .. } => (
            PortErrorKind::Conflict,
            "fulfillment.invalid_transition",
            "fulfillment lifecycle transition conflicts with the current state",
            false,
            false,
        ),
        crate::FulfillmentError::Database(_) => (
            PortErrorKind::Unavailable,
            "fulfillment.database_unavailable",
            "fulfillment storage is temporarily unavailable",
            true,
            true,
        ),
    };
    let context_facts = fulfillment_port_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = FULFILLMENT_OWNER,
            operation = owner_operation,
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
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            code,
            retryable,
            boundary = SHIPPING_SELECTION_BOUNDARY,
            "fulfillment shipping selection owner operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = FULFILLMENT_OWNER,
            operation = owner_operation,
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
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            code,
            retryable,
            boundary = SHIPPING_SELECTION_BOUNDARY,
            "fulfillment shipping selection owner operation was rejected with bounded diagnostics"
        );
    }

    PortError::new(kind, code, message, retryable)
}
