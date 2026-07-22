use rustok_api::{PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError};
use rustok_order::{
    AdoptLegacyCheckoutOrderIdentityRequest, BindCheckoutOrderIdentityRequest,
    CheckoutOrderIdentityPort, CheckoutOrderIdentitySnapshot, CreateOrderInput, OrderError,
    OrderResponse, OrderService, ReadCheckoutOrderIdentityByOperationRequest,
    in_process_checkout_order_identity_port,
};
use rustok_outbox::TransactionalEventBus;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutInventoryOrderAdoptionError, CheckoutInventoryOrderAdoptionService,
    CheckoutOperationError, CheckoutOperationJournal, CheckoutOperationStage,
    CheckoutOperationStatus,
};

const ORDER_IDENTITY_PORT_DEADLINE_SECONDS: u64 = 3;

#[derive(Debug, Error)]
pub enum CheckoutOrderCreationError {
    #[error(transparent)]
    Order(#[from] OrderError),
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Adoption(#[from] CheckoutInventoryOrderAdoptionError),
    #[error(
        "checkout order identity boundary failed with `{code}` (retryable={retryable}): {message}"
    )]
    IdentityBoundary {
        code: String,
        message: String,
        retryable: bool,
    },
    #[error("checkout order creation conflict: {0}")]
    Conflict(String),
}

pub type CheckoutOrderCreationResult<T> = Result<T, CheckoutOrderCreationError>;

pub struct CheckoutOrderCreationExecutor {
    order_service: OrderService,
    order_identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    operation_journal: CheckoutOperationJournal,
    adoption_service: CheckoutInventoryOrderAdoptionService,
    port_deadline: Duration,
}

impl CheckoutOrderCreationExecutor {
    pub fn new(db: sea_orm::DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            order_service: OrderService::new(db.clone(), event_bus),
            order_identity_port: in_process_checkout_order_identity_port(db.clone()),
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            adoption_service: CheckoutInventoryOrderAdoptionService::new(db),
            port_deadline: Duration::from_secs(ORDER_IDENTITY_PORT_DEADLINE_SECONDS),
        }
    }

    pub fn with_order_identity_port(
        mut self,
        order_identity_port: Arc<dyn CheckoutOrderIdentityPort>,
    ) -> Self {
        self.order_identity_port = order_identity_port;
        self
    }

    /// Creates one pending order for a durable checkout operation, binds it to
    /// typed order-owner checkout identity, adopts already reserved inventory
    /// rows into its order lines, and checkpoints `order_created`.
    ///
    /// Metadata identity remains only as a temporary owner-side legacy adoption
    /// bridge. Commerce never queries order storage or metadata directly.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_pending_and_adopt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        operation_id: Uuid,
        lease_owner: impl Into<String>,
        mut input: CreateOrderInput,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CheckoutOrderCreationResult<OrderResponse> {
        let lease_owner = lease_owner.into();
        let operation = self.operation_journal.get(tenant_id, operation_id).await?;
        if operation.status != CheckoutOperationStatus::Executing.as_str() {
            return Err(CheckoutOrderCreationError::Conflict(format!(
                "checkout operation {} must be executing, not `{}`",
                operation.id, operation.status
            )));
        }
        if !matches!(
            operation.stage.as_str(),
            stage if stage == CheckoutOperationStage::InventoryReserved.as_str()
                || stage == CheckoutOperationStage::OrderCreated.as_str()
        ) {
            return Err(CheckoutOrderCreationError::Conflict(format!(
                "checkout operation {} cannot create an order from stage `{}`",
                operation.id, operation.stage
            )));
        }
        let snapshot_hash = operation.snapshot_hash.as_deref().ok_or_else(|| {
            CheckoutOrderCreationError::Conflict(format!(
                "checkout operation {} has no immutable cart snapshot hash",
                operation.id
            ))
        })?;

        attach_checkout_identity(&mut input.metadata, operation_id, snapshot_hash)?;
        validate_line_item_provenance(&input)?;
        let request_hash = order_request_hash(&input, channel_id, channel_slug.as_deref())?;
        attach_order_request_hash(&mut input.metadata, request_hash.as_str())?;

        let mut identity = self
            .order_identity_port
            .read_by_operation(
                identity_context(
                    tenant_id,
                    actor_id,
                    operation_id,
                    self.port_deadline,
                    "read",
                    false,
                ),
                ReadCheckoutOrderIdentityByOperationRequest {
                    checkout_operation_id: operation_id,
                },
            )
            .await
            .map_err(identity_boundary_error)?;
        if identity.is_none() {
            identity = self
                .order_identity_port
                .adopt_legacy(
                    identity_context(
                        tenant_id,
                        actor_id,
                        operation_id,
                        self.port_deadline,
                        "adopt",
                        true,
                    ),
                    AdoptLegacyCheckoutOrderIdentityRequest {
                        checkout_operation_id: operation_id,
                        cart_id: operation.cart_id,
                    },
                )
                .await
                .map_err(identity_boundary_error)?;
        }

        let (order, identity) = match identity {
            Some(identity) => {
                validate_identity(
                    &identity,
                    tenant_id,
                    operation_id,
                    operation.cart_id,
                    None,
                    snapshot_hash,
                    request_hash.as_str(),
                )?;
                let order = self
                    .order_service
                    .get_order_with_locale_fallback(
                        tenant_id,
                        identity.order_id,
                        locale,
                        fallback_locale,
                    )
                    .await?;
                (order, identity)
            }
            None => {
                let create_result = self
                    .order_service
                    .create_order_with_channel(
                        tenant_id,
                        actor_id,
                        input,
                        channel_id,
                        channel_slug,
                    )
                    .await;
                match create_result {
                    Ok(order) => {
                        let identity = self
                            .order_identity_port
                            .bind(
                                identity_context(
                                    tenant_id,
                                    actor_id,
                                    operation_id,
                                    self.port_deadline,
                                    "bind",
                                    true,
                                ),
                                BindCheckoutOrderIdentityRequest {
                                    checkout_operation_id: operation_id,
                                    order_id: order.id,
                                    cart_id: operation.cart_id,
                                    payment_collection_id: None,
                                    shipping_option_id: None,
                                    snapshot_hash: snapshot_hash.to_string(),
                                    request_hash: request_hash.clone(),
                                },
                            )
                            .await
                            .map_err(identity_boundary_error)?;
                        (order, identity)
                    }
                    Err(create_error) => {
                        let Some(identity) = self
                            .order_identity_port
                            .adopt_legacy(
                                identity_context(
                                    tenant_id,
                                    actor_id,
                                    operation_id,
                                    self.port_deadline,
                                    "adopt-after-create-race",
                                    true,
                                ),
                                AdoptLegacyCheckoutOrderIdentityRequest {
                                    checkout_operation_id: operation_id,
                                    cart_id: operation.cart_id,
                                },
                            )
                            .await
                            .map_err(identity_boundary_error)?
                        else {
                            return Err(create_error.into());
                        };
                        let order = self
                            .order_service
                            .get_order_with_locale_fallback(
                                tenant_id,
                                identity.order_id,
                                locale,
                                fallback_locale,
                            )
                            .await?;
                        (order, identity)
                    }
                }
            }
        };

        validate_identity(
            &identity,
            tenant_id,
            operation_id,
            operation.cart_id,
            Some(order.id),
            snapshot_hash,
            request_hash.as_str(),
        )?;
        if order.tenant_id != tenant_id {
            return Err(CheckoutOrderCreationError::Conflict(format!(
                "order {} belongs to another tenant",
                order.id
            )));
        }
        if operation.stage == CheckoutOperationStage::InventoryReserved.as_str()
            && order.status != "pending"
        {
            return Err(CheckoutOrderCreationError::Conflict(format!(
                "order {} advanced to `{}` before inventory adoption was checkpointed",
                order.id, order.status
            )));
        }

        self.adoption_service
            .adopt_and_checkpoint(tenant_id, operation_id, lease_owner, &order)
            .await?;
        Ok(order)
    }
}

fn attach_checkout_identity(
    metadata: &mut Value,
    operation_id: Uuid,
    snapshot_hash: &str,
) -> CheckoutOrderCreationResult<()> {
    let root = metadata.as_object_mut().ok_or_else(|| {
        CheckoutOrderCreationError::Conflict("order metadata must be a JSON object".to_string())
    })?;
    let checkout = root
        .entry("checkout".to_string())
        .or_insert_with(|| Value::Object(Default::default()))
        .as_object_mut()
        .ok_or_else(|| {
            CheckoutOrderCreationError::Conflict(
                "order metadata.checkout must be a JSON object".to_string(),
            )
        })?;
    checkout.insert(
        "operation_id".to_string(),
        Value::String(operation_id.to_string()),
    );
    checkout.insert(
        "snapshot_hash".to_string(),
        Value::String(snapshot_hash.to_string()),
    );
    Ok(())
}

fn attach_order_request_hash(
    metadata: &mut Value,
    request_hash: &str,
) -> CheckoutOrderCreationResult<()> {
    let checkout = metadata
        .get_mut("checkout")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            CheckoutOrderCreationError::Conflict(
                "order metadata.checkout must be a JSON object".to_string(),
            )
        })?;
    checkout.insert(
        "order_request_hash".to_string(),
        Value::String(request_hash.to_string()),
    );
    Ok(())
}

fn validate_line_item_provenance(input: &CreateOrderInput) -> CheckoutOrderCreationResult<()> {
    let mut seen = HashSet::new();
    for (index, line) in input.line_items.iter().enumerate() {
        if line.variant_id.is_none() {
            continue;
        }
        let cart_line_item_id = line
            .metadata
            .get("checkout")
            .and_then(|checkout| checkout.get("cart_line_item_id"))
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| {
                CheckoutOrderCreationError::Conflict(format!(
                    "variant-backed order line input {index} has no valid cart-line provenance"
                ))
            })?;
        if !seen.insert(cart_line_item_id) {
            return Err(CheckoutOrderCreationError::Conflict(format!(
                "multiple order line inputs reference cart line {cart_line_item_id}"
            )));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_identity(
    identity: &CheckoutOrderIdentitySnapshot,
    tenant_id: Uuid,
    operation_id: Uuid,
    cart_id: Uuid,
    order_id: Option<Uuid>,
    snapshot_hash: &str,
    request_hash: &str,
) -> CheckoutOrderCreationResult<()> {
    if identity.tenant_id != tenant_id
        || identity.checkout_operation_id != operation_id
        || identity.source_cart_id.is_some() && identity.source_cart_id != Some(cart_id)
        || order_id.is_some() && Some(identity.order_id) != order_id
        || identity.snapshot_hash.as_deref() != Some(snapshot_hash)
        || identity.request_hash.as_deref() != Some(request_hash)
    {
        return Err(CheckoutOrderCreationError::Conflict(format!(
            "checkout operation {operation_id} is bound to different typed order identity evidence"
        )));
    }
    Ok(())
}

fn identity_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    operation_id: Uuid,
    deadline: Duration,
    action: &str,
    write: bool,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        PLATFORM_FALLBACK_LOCALE,
        format!("checkout:{operation_id}:order-identity:{action}"),
    )
    .with_causation_id(operation_id.to_string())
    .with_deadline(deadline);
    if write {
        context.with_idempotency_key(format!(
            "checkout:{operation_id}:order-identity:{action}"
        ))
    } else {
        context
    }
}

fn identity_boundary_error(error: PortError) -> CheckoutOrderCreationError {
    CheckoutOrderCreationError::IdentityBoundary {
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn order_request_hash(
    input: &CreateOrderInput,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
) -> CheckoutOrderCreationResult<String> {
    let value = serde_json::to_value((input, channel_id, channel_slug)).map_err(|error| {
        CheckoutOrderCreationError::Conflict(format!(
            "failed to serialize order creation request: {error}"
        ))
    })?;
    let canonical = canonicalize_json(value);
    let payload = serde_json::to_vec(&canonical).map_err(|error| {
        CheckoutOrderCreationError::Conflict(format!(
            "failed to encode order creation request: {error}"
        ))
    })?;
    Ok(Sha256::digest(payload)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(values) => {
            let ordered = values
                .into_iter()
                .map(|(key, value)| (key, canonicalize_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        value => value,
    }
}
