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
            OrderStatusKind::Cancelled => Err(PortError::conflict(
                "order.checkout_order_cancelled",
                "checkout order is already cancelled",
            )),
            OrderStatusKind::Unknown => Err(PortError::invariant_violation(
                "order.checkout_order_status_invalid",
                "checkout order has an unsupported lifecycle state",
            )),
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
    identity: &CheckoutOrderIdentitySnapshot,
    tenant_id: Uuid,
    request: &RecoverExistingCheckoutOrderRequest,
    owner_hashes: &(String, String),
    legacy_snapshot_hash: &str,
    legacy_request_hash: &str,
) -> Result<(), PortError> {
    let base_matches = identity.tenant_id == tenant_id
        && identity.checkout_operation_id == request.checkout_operation_id
        && identity
            .source_cart_id
            .is_none_or(|id| id == request.completion.cart_id)
        && identity
            .payment_collection_id
            .is_none_or(|id| Some(id) == request.completion.payment_collection_id)
        && identity
            .shipping_option_id
            .is_none_or(|id| Some(id) == request.completion.shipping_option_id);
    let owner_hashes_match = identity.snapshot_hash.as_deref() == Some(owner_hashes.0.as_str())
        && identity.request_hash.as_deref() == Some(owner_hashes.1.as_str());
    let legacy_hashes_match = identity.snapshot_hash.as_deref() == Some(legacy_snapshot_hash)
        && identity.request_hash.as_deref() == Some(legacy_request_hash);
    if !base_matches || !(owner_hashes_match || legacy_hashes_match) {
        return Err(PortError::conflict(
            "order.checkout_request_conflict",
            "checkout operation is already bound to a different completion request",
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
        tracing::warn!(
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation,
            code = "order.checkout_operation_id_invalid",
            expected_checkout_operation_id = %checkout_operation_id,
            "checkout recovery received invalid causation identity"
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
        tracing::warn!(
            correlation_id = %context.correlation_id,
            operation,
            field = "tenant_id",
            value_length = context.tenant_id.len(),
            code = "order.tenant_id_invalid",
            "order port received invalid request context"
        );
        PortError::validation(
            "order.tenant_id_invalid",
            "order request context is invalid",
        )
    })
}

fn parse_actor_id(context: &PortContext, operation: &'static str) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|_| {
        tracing::warn!(
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation,
            field = "actor_id",
            value_length = context.actor.id.len(),
            code = "order.actor_id_invalid",
            "order port received invalid request context"
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
    let full_request = serde_json::to_value(request).map_err(|error| {
        tracing::error!(
            error = ?error,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = RECOVER_OPERATION,
            code = "order.checkout_request_encoding_failed",
            "failed to encode checkout recovery request"
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
    let bytes = serde_json::to_vec(&canonical).map_err(|error| {
        tracing::error!(
            error = ?error,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation,
            code = "order.checkout_request_encoding_failed",
            "failed to encode canonical checkout recovery request"
        );
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
    if value.len() < min_len
        || value.len() > max_len
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        tracing::warn!(
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation,
            field,
            value_length = value.len(),
            min_len,
            max_len,
            code = "order.checkout_hash_invalid",
            "checkout recovery rejected invalid hash evidence"
        );
        return Err(PortError::validation(
            "order.checkout_hash_invalid",
            "checkout hash evidence is invalid",
        ));
    }
    Ok(value)
}

fn order_error_to_port_error(
    context: &PortContext,
    operation: &'static str,
    error: OrderError,
) -> PortError {
    match error {
        OrderError::Database(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation,
                code = "order.database_unavailable",
                "order checkout recovery storage failed"
            );
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(_) => {
            PortError::not_found("order.order_not_found", "order was not found")
        }
        OrderError::Validation(cause) => {
            tracing::warn!(
                cause = %cause,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation,
                code = "order.checkout_recovery_validation",
                "order owner rejected checkout recovery"
            );
            PortError::validation(
                "order.checkout_recovery_validation",
                "checkout order recovery request is invalid",
            )
        }
        OrderError::InvalidTransition { .. } => {
            tracing::warn!(
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation,
                code = "order.checkout_recovery_state_conflict",
                "order lifecycle conflicts with checkout recovery"
            );
            PortError::conflict(
                "order.checkout_recovery_state_conflict",
                "order lifecycle transition conflicts with checkout recovery",
            )
        }
        OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_) => {
            PortError::not_found(
                "order.related_resource_not_found",
                "related order resource was not found",
            )
        }
        OrderError::Core(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation,
                code = "order.invariant_violation",
                "order checkout recovery invariant failed"
            );
            PortError::invariant_violation(
                "order.invariant_violation",
                "order operation failed an internal invariant",
            )
        }
    }
}
