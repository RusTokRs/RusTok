use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::{
    AdoptLegacyCheckoutOrderIdentityRequest, CheckoutOrderIdentityPort,
    CheckoutOrderIdentitySnapshot, CompleteCheckoutPortRequest, InProcessCheckoutOrderIdentityPort,
    OrderError, OrderResponse, OrderService, OrderStatusKind,
    ReadCheckoutOrderIdentityByOperationRequest,
};

const CHECKOUT_ORDER_RECOVERY_OWNER: &str = "rustok_order.checkout_order_recovery";
const CHECKOUT_ORDER_RECOVERY_BOUNDARY: &str = "checkout_order_recovery_adapter";
const RECOVER_OPERATION: &str = "recover_existing_checkout";
const READ_OPERATION: &str = "read_checkout_order";

/// Order-owned in-process adapter used while staged commerce checkout migrates
/// from the legacy metadata bridge to the durable `CheckoutCompletionPort`.
///
/// New order creation always stays on `CheckoutCompletionPort`. This adapter
/// only recovers an already-created order, validates both the new owner hashes
/// and the previous staged-checkout hashes, and exposes the full owner order
/// projection required for inventory reservation adoption.
pub struct CheckoutOrderRecoveryAdapter {
    order_service: OrderService,
    identity_port: InProcessCheckoutOrderIdentityPort,
}

impl CheckoutOrderRecoveryAdapter {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            order_service: OrderService::new(db.clone(), event_bus),
            identity_port: InProcessCheckoutOrderIdentityPort::new(db),
        }
    }

    /// Recovers an order created before the staged-checkout cutover.
    ///
    /// The adapter first adopts the old metadata identity into owner persistence,
    /// validates immutable request evidence, and resumes a pending order through
    /// the owner lifecycle. `None` means no existing owner outcome was found and
    /// the caller may invoke `CheckoutCompletionPort::complete_checkout`.
    pub async fn recover_existing_checkout(
        &self,
        context: PortContext,
        request: RecoverExistingCheckoutOrderRequest,
    ) -> Result<Option<OrderResponse>, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_tenant_id(&context, RECOVER_OPERATION)?;
        let actor_id = parse_actor_id(&context, RECOVER_OPERATION)?;
        require_operation_context(&context, RECOVER_OPERATION, request.checkout_operation_id)?;
        let legacy_snapshot_hash = normalize_hash(
            &context,
            RECOVER_OPERATION,
            request.legacy_snapshot_hash.clone(),
            "legacy_snapshot_hash",
            1,
            128,
        )?;
        let legacy_request_hash = normalize_hash(
            &context,
            RECOVER_OPERATION,
            request.legacy_request_hash.clone(),
            "legacy_request_hash",
            64,
            64,
        )?;
        let owner_hashes = checkout_request_hashes(&context, &request.completion)?;

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
                        cart_id: request.completion.cart_id,
                    },
                )
                .await?;
        }
        let Some(identity) = identity else {
            return Ok(None);
        };

        validate_identity(
            &context,
            &identity,
            tenant_id,
            &request,
            &owner_hashes,
            legacy_snapshot_hash.as_str(),
            legacy_request_hash.as_str(),
        )?;
        let order = self
            .resume_order(
                &context,
                tenant_id,
                actor_id,
                identity.order_id,
                request.completion.locale.as_deref(),
                request.completion.fallback_locale.as_deref(),
            )
            .await?;
        Ok(Some(order))
    }

    /// Loads the full typed owner projection for a checkout operation.
    pub async fn read_checkout_order(
        &self,
        context: PortContext,
        request: ReadCheckoutOrderProjectionRequest,
    ) -> Result<OrderResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, READ_OPERATION)?;
        let identity = self
            .identity_port
            .read_by_operation(
                context.clone(),
                ReadCheckoutOrderIdentityByOperationRequest {
                    checkout_operation_id: request.checkout_operation_id,
                },
            )
            .await?
            .ok_or_else(|| {
                log_checkout_order_recovery_identity_not_found(&context, &request);
                PortError::not_found(
                    "order.checkout_order_not_found",
                    "checkout order was not found for the requested operation",
                )
            })?;
        self.load_order(
            &context,
            tenant_id,
            identity.order_id,
            request.locale.as_deref(),
            request.fallback_locale.as_deref(),
        )
        .await
    }

    async fn resume_order(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        order_id: Uuid,
        locale: Option<&str>,
        fallback_locale: Option<&str>,
    ) -> Result<OrderResponse, PortError> {
        let order = self
            .load_order(context, tenant_id, order_id, locale, fallback_locale)
            .await?;
        match order.status_kind() {
            OrderStatusKind::Pending => {
                let order = self
                    .order_service
                    .confirm_order(tenant_id, actor_id, order.id)
                    .await
                    .map_err(|error| {
                        order_error_to_port_error(
                            context,
                            "confirm_recovered_checkout_order",
                            error,
                        )
                    })?;
                if let Some(locale) = locale {
                    self.order_service
                        .get_order_with_locale_fallback(
                            tenant_id,
                            order.id,
                            locale,
                            fallback_locale,
                        )
                        .await
                        .map_err(|error| {
                            order_error_to_port_error(
                                context,
                                "reload_recovered_checkout_order",
                                error,
                            )
                        })
                } else {
                    Ok(order)
                }
            }
            OrderStatusKind::Confirmed
            | OrderStatusKind::Paid
            | OrderStatusKind::Shipped
            | OrderStatusKind::Delivered => Ok(order),
            OrderStatusKind::Cancelled => {
                log_checkout_order_recovery_lifecycle_rejection(
                    context,
                    order.id,
                    "cancelled",
                    "order.checkout_order_cancelled",
                    false,
                );
                Err(PortError::conflict(
                    "order.checkout_order_cancelled",
                    "checkout order is already cancelled",
                ))
            }
            OrderStatusKind::Unknown => {
                log_checkout_order_recovery_lifecycle_rejection(
                    context,
                    order.id,
                    "unknown",
                    "order.checkout_order_status_invalid",
                    true,
                );
                Err(PortError::invariant_violation(
                    "order.checkout_order_status_invalid",
                    "checkout order has an unsupported lifecycle state",
                ))
            }
        }
    }

    async fn load_order(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        order_id: Uuid,
        locale: Option<&str>,
        fallback_locale: Option<&str>,
    ) -> Result<OrderResponse, PortError> {
        match locale {
            Some(locale) => {
                self.order_service
                    .get_order_with_locale_fallback(tenant_id, order_id, locale, fallback_locale)
                    .await
            }
            None => self.order_service.get_order(tenant_id, order_id).await,
        }
        .map_err(|error| order_error_to_port_error(context, "load_checkout_order", error))
    }
}

pub fn in_process_checkout_order_recovery_adapter(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> CheckoutOrderRecoveryAdapter {
    CheckoutOrderRecoveryAdapter::new(db, event_bus)
}

#[derive(Debug, Clone)]
pub struct RecoverExistingCheckoutOrderRequest {
    pub checkout_operation_id: Uuid,
    pub completion: CompleteCheckoutPortRequest,
    pub legacy_snapshot_hash: String,
    pub legacy_request_hash: String,
}

#[derive(Debug, Clone)]
pub struct ReadCheckoutOrderProjectionRequest {
    pub checkout_operation_id: Uuid,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

fn validate_identity(
    context: &PortContext,
    identity: &CheckoutOrderIdentitySnapshot,
    tenant_id: Uuid,
    request: &RecoverExistingCheckoutOrderRequest,
    owner_hashes: &(String, String),
    legacy_snapshot_hash: &str,
    legacy_request_hash: &str,
) -> Result<(), PortError> {
    let tenant_matches = identity.tenant_id == tenant_id;
    let checkout_operation_matches =
        identity.checkout_operation_id == request.checkout_operation_id;
    let source_cart_matches = identity
        .source_cart_id
        .is_none_or(|id| id == request.completion.cart_id);
    let payment_collection_matches = identity
        .payment_collection_id
        .is_none_or(|id| Some(id) == request.completion.payment_collection_id);
    let shipping_option_matches = identity
        .shipping_option_id
        .is_none_or(|id| Some(id) == request.completion.shipping_option_id);
    let base_matches = tenant_matches
        && checkout_operation_matches
        && source_cart_matches
        && payment_collection_matches
        && shipping_option_matches;
    let owner_hashes_match = identity.snapshot_hash.as_deref() == Some(owner_hashes.0.as_str())
        && identity.request_hash.as_deref() == Some(owner_hashes.1.as_str());
    let legacy_hashes_match = identity.snapshot_hash.as_deref() == Some(legacy_snapshot_hash)
        && identity.request_hash.as_deref() == Some(legacy_request_hash);
    if !base_matches || !(owner_hashes_match || legacy_hashes_match) {
        log_checkout_order_recovery_identity_conflict(
            context,
            identity,
            request,
            tenant_matches,
            checkout_operation_matches,
            source_cart_matches,
            payment_collection_matches,
            shipping_option_matches,
            base_matches,
            owner_hashes_match,
            legacy_hashes_match,
        );
        return Err(PortError::conflict(
            "order.checkout_request_conflict",
            "checkout operation is already bound to a different completion request",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn log_checkout_order_recovery_identity_conflict(
    context: &PortContext,
    identity: &CheckoutOrderIdentitySnapshot,
    request: &RecoverExistingCheckoutOrderRequest,
    tenant_matches: bool,
    checkout_operation_matches: bool,
    source_cart_matches: bool,
    payment_collection_matches: bool,
    shipping_option_matches: bool,
    base_matches: bool,
    owner_hashes_match: bool,
    legacy_hashes_match: bool,
) {
    let context_facts = checkout_order_recovery_context_facts(context);
    tracing::error!(
        owner = CHECKOUT_ORDER_RECOVERY_OWNER,
        operation = RECOVER_OPERATION,
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
        request_checkout_operation_id_non_nil = !request.checkout_operation_id.is_nil(),
        request_cart_id_non_nil = !request.completion.cart_id.is_nil(),
        request_payment_collection_id_present = request.completion.payment_collection_id.is_some(),
        request_payment_collection_id_non_nil = ?request
            .completion
            .payment_collection_id
            .map(|id| !id.is_nil()),
        request_shipping_option_id_present = request.completion.shipping_option_id.is_some(),
        request_shipping_option_id_non_nil = ?request
            .completion
            .shipping_option_id
            .map(|id| !id.is_nil()),
        identity_tenant_id_non_nil = !identity.tenant_id.is_nil(),
        identity_checkout_operation_id_non_nil = !identity.checkout_operation_id.is_nil(),
        identity_order_id_non_nil = !identity.order_id.is_nil(),
        identity_source_cart_id_present = identity.source_cart_id.is_some(),
        identity_source_cart_id_non_nil = ?identity.source_cart_id.map(|id| !id.is_nil()),
        identity_payment_collection_id_present = identity.payment_collection_id.is_some(),
        identity_payment_collection_id_non_nil = ?identity
            .payment_collection_id
            .map(|id| !id.is_nil()),
        identity_shipping_option_id_present = identity.shipping_option_id.is_some(),
        identity_shipping_option_id_non_nil = ?identity
            .shipping_option_id
            .map(|id| !id.is_nil()),
        identity_snapshot_hash_present = identity.snapshot_hash.is_some(),
        identity_snapshot_hash_length = ?identity.snapshot_hash.as_ref().map(String::len),
        identity_request_hash_present = identity.request_hash.is_some(),
        identity_request_hash_length = ?identity.request_hash.as_ref().map(String::len),
        tenant_matches,
        checkout_operation_matches,
        source_cart_matches,
        payment_collection_matches,
        shipping_option_matches,
        base_matches,
        owner_hashes_match,
        legacy_hashes_match,
        code = "order.checkout_request_conflict",
        boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
        "checkout recovery identity conflicts with the completion request"
    );
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
        log_checkout_order_recovery_admission_rejection(
            context,
            operation,
            "causation_id",
            context.causation_id.is_some(),
            context
                .causation_id
                .as_ref()
                .map(|value| value.chars().count()),
            context_operation.is_some(),
            context_operation.map(|value| !value.is_nil()),
            Some(!checkout_operation_id.is_nil()),
            Some(false),
            "order.checkout_operation_id_invalid",
        );
        return Err(PortError::validation(
            "order.checkout_operation_id_invalid",
            "checkout operation context is invalid",
        ));
    }
    Ok(())
}

fn parse_tenant_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        log_checkout_order_recovery_admission_rejection(
            context,
            operation,
            "tenant_id",
            true,
            Some(context.tenant_id.chars().count()),
            false,
            None,
            None,
            None,
            "order.tenant_id_invalid",
        );
        PortError::validation(
            "order.tenant_id_invalid",
            "order request context is invalid",
        )
    })
}

fn parse_actor_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|_| {
        log_checkout_order_recovery_admission_rejection(
            context,
            operation,
            "actor_id",
            true,
            Some(context.actor.id.chars().count()),
            false,
            None,
            None,
            None,
            "order.actor_id_invalid",
        );
        PortError::validation("order.actor_id_invalid", "order request context is invalid")
    })
}

fn checkout_request_hashes(
    context: &PortContext,
    request: &CompleteCheckoutPortRequest,
) -> Result<(String, String), PortError> {
    let snapshot = serde_json::json!({
        "cart_id": request.cart_id,
        "customer_id": request.customer_id,
        "shipping_option_id": request.shipping_option_id,
        "channel_id": request.channel_id,
        "channel_slug": request.channel_slug,
        "currency_code": request.currency_code,
        "shipping_total": request.shipping_total,
        "line_items": request.line_items,
        "adjustments": request.adjustments,
        "tax_lines": request.tax_lines,
    });
    let full_request = serde_json::to_value(request).map_err(|_| {
        log_checkout_order_recovery_encoding_failure(
            context,
            RECOVER_OPERATION,
            "checkout_completion_request",
        );
        PortError::invariant_violation(
            "order.checkout_request_encoding_failed",
            "checkout completion request could not be encoded",
        )
    })?;
    Ok((
        hash_json(context, "encode_checkout_snapshot_hash", snapshot)?,
        hash_json(context, "encode_checkout_request_hash", full_request)?,
    ))
}

fn hash_json(
    context: &PortContext,
    operation: &'static str,
    value: Value,
) -> Result<String, PortError> {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).map_err(|_| {
        log_checkout_order_recovery_encoding_failure(context, operation, "canonical_checkout_json");
        PortError::invariant_violation(
            "order.checkout_request_encoding_failed",
            "checkout completion request could not be encoded",
        )
    })?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        value => value,
    }
}

fn normalize_hash(
    context: &PortContext,
    operation: &'static str,
    value: String,
    field: &'static str,
    min_len: usize,
    max_len: usize,
) -> Result<String, PortError> {
    let value = value.trim().to_ascii_lowercase();
    let value_length = value.len();
    let length_within_bounds = (min_len..=max_len).contains(&value_length);
    let ascii_hex = value.bytes().all(|byte| byte.is_ascii_hexdigit());
    if !length_within_bounds || !ascii_hex {
        log_checkout_order_recovery_hash_rejection(
            context,
            operation,
            field,
            value_length,
            min_len,
            max_len,
            length_within_bounds,
            ascii_hex,
        );
        return Err(PortError::validation(
            "order.checkout_hash_invalid",
            "checkout hash evidence is invalid",
        ));
    }
    Ok(value)
}

#[derive(Clone, Copy, Debug)]
struct CheckoutOrderRecoveryContextFacts {
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

#[derive(Clone, Copy, Debug)]
struct CheckoutOrderRecoveryOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn checkout_order_recovery_context_facts(
    context: &PortContext,
) -> CheckoutOrderRecoveryContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    CheckoutOrderRecoveryContextFacts {
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

fn log_checkout_order_recovery_lifecycle_rejection(
    context: &PortContext,
    order_id: Uuid,
    lifecycle_state: &'static str,
    code: &'static str,
    technical_failure: bool,
) {
    let context_facts = checkout_order_recovery_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = CHECKOUT_ORDER_RECOVERY_OWNER,
            operation = RECOVER_OPERATION,
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
            lifecycle_state,
            code,
            boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
            "checkout recovery found an unsupported order lifecycle state"
        );
    } else {
        tracing::warn!(
            owner = CHECKOUT_ORDER_RECOVERY_OWNER,
            operation = RECOVER_OPERATION,
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
            lifecycle_state,
            code,
            boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
            "checkout recovery found a terminal order lifecycle state"
        );
    }
}

fn log_checkout_order_recovery_encoding_failure(
    context: &PortContext,
    operation: &'static str,
    serialization_target: &'static str,
) {
    let context_facts = checkout_order_recovery_context_facts(context);
    tracing::error!(
        owner = CHECKOUT_ORDER_RECOVERY_OWNER,
        operation,
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
        serialization_target,
        code = "order.checkout_request_encoding_failed",
        boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
        "checkout recovery request encoding failed with bounded diagnostics"
    );
}

#[allow(clippy::too_many_arguments)]
fn log_checkout_order_recovery_hash_rejection(
    context: &PortContext,
    operation: &'static str,
    field: &'static str,
    value_length: usize,
    min_len: usize,
    max_len: usize,
    length_within_bounds: bool,
    ascii_hex: bool,
) {
    let context_facts = checkout_order_recovery_context_facts(context);
    tracing::warn!(
        owner = CHECKOUT_ORDER_RECOVERY_OWNER,
        operation,
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
        field,
        value_length,
        min_len,
        max_len,
        length_within_bounds,
        ascii_hex,
        code = "order.checkout_hash_invalid",
        boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
        "checkout recovery rejected invalid hash evidence with bounded diagnostics"
    );
}

fn log_checkout_order_recovery_identity_not_found(
    context: &PortContext,
    request: &ReadCheckoutOrderProjectionRequest,
) {
    let context_facts = checkout_order_recovery_context_facts(context);
    tracing::warn!(
        owner = CHECKOUT_ORDER_RECOVERY_OWNER,
        operation = READ_OPERATION,
        correlation_id = %context.correlation_id,
        tenant_id_length = context_facts.tenant_id_length,
        actor_kind = context_facts.actor_kind,
        actor_id_length = context_facts.actor_id_length,
        claim_count = context_facts.claim_count,
        role_count = context_facts.role_count,
        channel_present = context_facts.channel_present,
        channel_length = ?context_facts.channel_length,
        context_locale_length = context_facts.locale_length,
        causation_id_present = context_facts.causation_id_present,
        causation_id_length = ?context_facts.causation_id_length,
        traceparent_present = context_facts.traceparent_present,
        traceparent_length = ?context_facts.traceparent_length,
        idempotency_key_present = context_facts.idempotency_key_present,
        idempotency_key_length = ?context_facts.idempotency_key_length,
        deadline_ms = ?context_facts.deadline_ms,
        checkout_operation_id_non_nil = !request.checkout_operation_id.is_nil(),
        request_locale_present = request.locale.is_some(),
        request_locale_length = ?request.locale.as_ref().map(|value| value.chars().count()),
        request_fallback_locale_present = request.fallback_locale.is_some(),
        request_fallback_locale_length = ?request
            .fallback_locale
            .as_ref()
            .map(|value| value.chars().count()),
        code = "order.checkout_order_not_found",
        boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
        "checkout order identity was not found for the requested operation"
    );
}

#[allow(clippy::too_many_arguments)]
fn log_checkout_order_recovery_admission_rejection(
    context: &PortContext,
    operation: &'static str,
    field: &'static str,
    field_value_present: bool,
    field_value_length: Option<usize>,
    uuid_parseable: bool,
    uuid_non_nil: Option<bool>,
    expected_uuid_non_nil: Option<bool>,
    matches_expected: Option<bool>,
    code: &'static str,
) {
    let context_facts = checkout_order_recovery_context_facts(context);
    tracing::warn!(
        owner = CHECKOUT_ORDER_RECOVERY_OWNER,
        operation,
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
        field,
        field_value_present,
        field_value_length = ?field_value_length,
        uuid_parseable,
        uuid_non_nil = ?uuid_non_nil,
        expected_uuid_non_nil = ?expected_uuid_non_nil,
        matches_expected = ?matches_expected,
        code,
        boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
        "order checkout recovery admission was rejected with bounded diagnostics"
    );
}

fn checkout_order_recovery_owner_error_facts(
    error: &OrderError,
) -> CheckoutOrderRecoveryOwnerErrorFacts {
    let (
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    ) = match error {
        OrderError::Database(_) => ("database", 0, 0, 0, 0, true),
        OrderError::OrderNotFound(id) => (
            "order_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::Validation(value) => ("validation", 1, value.chars().count(), 0, 0, false),
        OrderError::InvalidTransition { from, to } => (
            "invalid_transition",
            2,
            from.chars().count() + to.chars().count(),
            0,
            0,
            false,
        ),
        OrderError::OrderReturnNotFound(id) => (
            "order_return_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::OrderChangeNotFound(id) => (
            "order_change_not_found",
            0,
            0,
            1,
            if id.is_nil() { 0 } else { 1 },
            false,
        ),
        OrderError::Core(_) => ("core", 0, 0, 0, 0, true),
    };
    CheckoutOrderRecoveryOwnerErrorFacts {
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    }
}

fn checkout_order_recovery_owner_error_code(error: &OrderError) -> &'static str {
    match error {
        OrderError::Database(_) => "order.database_unavailable",
        OrderError::OrderNotFound(_) => "order.order_not_found",
        OrderError::Validation(_) => "order.checkout_recovery_validation",
        OrderError::InvalidTransition { .. } => "order.checkout_recovery_state_conflict",
        OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_) => {
            "order.related_resource_not_found"
        }
        OrderError::Core(_) => "order.invariant_violation",
    }
}

fn checkout_order_recovery_owner_error_is_technical(error: &OrderError) -> bool {
    matches!(error, OrderError::Database(_) | OrderError::Core(_))
}

fn log_checkout_order_recovery_owner_error(
    context: &PortContext,
    operation: &'static str,
    code: &'static str,
    technical_failure: bool,
    error_facts: &CheckoutOrderRecoveryOwnerErrorFacts,
) {
    let context_facts = checkout_order_recovery_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = CHECKOUT_ORDER_RECOVERY_OWNER,
            operation,
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
            owner_error_variant = error_facts.error_variant,
            owner_error_text_field_count = error_facts.text_field_count,
            owner_error_text_total_length = error_facts.text_total_length,
            owner_error_uuid_field_count = error_facts.uuid_field_count,
            owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
            owner_error_opaque_payload_present = error_facts.opaque_payload_present,
            code,
            boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
            "order checkout recovery owner operation failed"
        );
    } else {
        tracing::warn!(
            owner = CHECKOUT_ORDER_RECOVERY_OWNER,
            operation,
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
            owner_error_variant = error_facts.error_variant,
            owner_error_text_field_count = error_facts.text_field_count,
            owner_error_text_total_length = error_facts.text_total_length,
            owner_error_uuid_field_count = error_facts.uuid_field_count,
            owner_error_uuid_non_nil_count = error_facts.uuid_non_nil_count,
            owner_error_opaque_payload_present = error_facts.opaque_payload_present,
            code,
            boundary = CHECKOUT_ORDER_RECOVERY_BOUNDARY,
            "order checkout recovery owner operation was rejected"
        );
    }
}

fn order_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: OrderError,
) -> PortError {
    let code = checkout_order_recovery_owner_error_code(&error);
    let technical_failure = checkout_order_recovery_owner_error_is_technical(&error);
    let error_facts = checkout_order_recovery_owner_error_facts(&error);
    log_checkout_order_recovery_owner_error(
        context,
        operation,
        code,
        technical_failure,
        &error_facts,
    );
    match error {
        OrderError::Database(_) => PortError::unavailable(
            "order.database_unavailable",
            "order storage is temporarily unavailable",
        ),
        OrderError::OrderNotFound(_) => {
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(_) => PortError::validation(
            "order.checkout_recovery_validation",
            "checkout order recovery request is invalid",
        ),
        OrderError::InvalidTransition { .. } => PortError::conflict(
            "order.checkout_recovery_state_conflict",
            "order lifecycle transition conflicts with checkout recovery",
        ),
        OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_) => {
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::Core(_) => PortError::invariant_violation(
            "order.invariant_violation",
            "order operation failed an internal invariant",
        ),
    }
}
