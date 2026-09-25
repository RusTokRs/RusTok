use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

use crate::services::{
    OrderCheckoutIdentityError, OrderCheckoutIdentityJournal, RecordOrderCheckoutIdentity,
};
use crate::{OrderError, OrderResponse, OrderService, OrderStatusKind};

const ORDER_PORT_BOUNDARY: &str = "order_port";

struct OrderPortContextFacts {
    correlation_id_length: usize,
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

struct OrderPortErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn order_port_context_facts(context: &PortContext) -> OrderPortContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    OrderPortContextFacts {
        correlation_id_length: context.correlation_id.chars().count(),
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

fn order_checkout_identity_error_facts(
    error: &OrderCheckoutIdentityError,
) -> OrderPortErrorFacts {
    match error {
        OrderCheckoutIdentityError::Validation(message)
        | OrderCheckoutIdentityError::Conflict(message) => OrderPortErrorFacts {
            error_variant: match error {
                OrderCheckoutIdentityError::Validation(_) => "checkout_identity_validation",
                OrderCheckoutIdentityError::Conflict(_) => "checkout_identity_conflict",
                _ => unreachable!(),
            },
            text_field_count: 1,
            text_total_length: message.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        OrderCheckoutIdentityError::OrderNotFound(id) => OrderPortErrorFacts {
            error_variant: "checkout_identity_order_not_found",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 1,
            uuid_non_nil_count: if id.is_nil() { 0 } else { 1 },
            opaque_payload_present: false,
        },
        OrderCheckoutIdentityError::Database(_) => OrderPortErrorFacts {
            error_variant: "checkout_identity_database",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
    }
}

fn order_error_facts(error: &OrderError) -> OrderPortErrorFacts {
    match error {
        OrderError::Validation(message) => OrderPortErrorFacts {
            error_variant: "validation",
            text_field_count: 1,
            text_total_length: message.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        OrderError::OrderNotFound(id)
        | OrderError::OrderReturnNotFound(id)
        | OrderError::OrderChangeNotFound(id) => OrderPortErrorFacts {
            error_variant: match error {
                OrderError::OrderNotFound(_) => "order_not_found",
                OrderError::OrderReturnNotFound(_) => "order_return_not_found",
                OrderError::OrderChangeNotFound(_) => "order_change_not_found",
                _ => unreachable!(),
            },
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 1,
            uuid_non_nil_count: if id.is_nil() { 0 } else { 1 },
            opaque_payload_present: false,
        },
        OrderError::InvalidTransition { from, to } => OrderPortErrorFacts {
            error_variant: "invalid_transition",
            text_field_count: 2,
            text_total_length: from.chars().count() + to.chars().count(),
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        OrderError::Database(_) => OrderPortErrorFacts {
            error_variant: "database",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
        OrderError::Core(_) => OrderPortErrorFacts {
            error_variant: "core",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: true,
        },
        OrderError::IdempotencyConflict => OrderPortErrorFacts {
            error_variant: "idempotency_conflict",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
        OrderError::CommandReceiptCorrupt => OrderPortErrorFacts {
            error_variant: "command_receipt_corrupt",
            text_field_count: 0,
            text_total_length: 0,
            uuid_field_count: 0,
            uuid_non_nil_count: 0,
            opaque_payload_present: false,
        },
    }
}

fn log_order_port_failure(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    facts: &OrderPortErrorFacts,
    technical_failure: bool,
) {
    let context_facts = order_port_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = "rustok_order",
            owner_operation,
            correlation_id = %context.correlation_id,
            correlation_id_length = context_facts.correlation_id_length,
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
            error_variant = facts.error_variant,
            text_field_count = facts.text_field_count,
            text_total_length = facts.text_total_length,
            uuid_field_count = facts.uuid_field_count,
            uuid_non_nil_count = facts.uuid_non_nil_count,
            opaque_payload_present = facts.opaque_payload_present,
            boundary = ORDER_PORT_BOUNDARY,
            "order owner operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = "rustok_order",
            owner_operation,
            correlation_id = %context.correlation_id,
            correlation_id_length = context_facts.correlation_id_length,
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
            error_variant = facts.error_variant,
            text_field_count = facts.text_field_count,
            text_total_length = facts.text_total_length,
            uuid_field_count = facts.uuid_field_count,
            uuid_non_nil_count = facts.uuid_non_nil_count,
            opaque_payload_present = facts.opaque_payload_present,
            boundary = ORDER_PORT_BOUNDARY,
            "order owner operation was rejected with bounded diagnostics"
        );
    }
}

fn log_order_context_rejection(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    parse_target: &'static str,
) {
    let context_facts = order_port_context_facts(context);
    tracing::warn!(
        owner = "rustok_order",
        owner_operation,
        correlation_id = %context.correlation_id,
        correlation_id_length = context_facts.correlation_id_length,
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
        parse_target,
        parse_failed = true,
        boundary = ORDER_PORT_BOUNDARY,
        "order port context was rejected with bounded diagnostics"
    );
}


/// Transport-neutral order-owner boundary for durable checkout identity.
#[async_trait]
pub trait CheckoutOrderIdentityPort: Send + Sync {
    async fn read_by_operation(
        &self,
        context: PortContext,
        request: ReadCheckoutOrderIdentityByOperationRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError>;

    async fn read_by_cart(
        &self,
        context: PortContext,
        request: ReadCheckoutOrderIdentityByCartRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError>;

    async fn bind(
        &self,
        context: PortContext,
        request: BindCheckoutOrderIdentityRequest,
    ) -> Result<CheckoutOrderIdentitySnapshot, PortError>;

    /// Temporary owner-side compatibility operation. It adopts an order created
    /// by the old metadata path into typed owner persistence. Consumers must not
    /// inspect order metadata or query owner tables themselves.
    async fn adopt_legacy(
        &self,
        context: PortContext,
        request: AdoptLegacyCheckoutOrderIdentityRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError>;
}

#[derive(Clone)]
pub struct InProcessCheckoutOrderIdentityPort {
    db: DatabaseConnection,
    journal: OrderCheckoutIdentityJournal,
}

impl InProcessCheckoutOrderIdentityPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            journal: OrderCheckoutIdentityJournal::new(db.clone()),
            db,
        }
    }
}

pub fn in_process_checkout_order_identity_port(
    db: DatabaseConnection,
) -> Arc<dyn CheckoutOrderIdentityPort> {
    Arc::new(InProcessCheckoutOrderIdentityPort::new(db))
}

#[async_trait]
impl CheckoutOrderIdentityPort for InProcessCheckoutOrderIdentityPort {
    async fn read_by_operation(
        &self,
        context: PortContext,
        request: ReadCheckoutOrderIdentityByOperationRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError> {
        let owner_operation = "read_checkout_identity_by_operation";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        self.journal
            .get_by_operation(tenant_id, request.checkout_operation_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(|error| {
                order_checkout_identity_error_to_port_error(&context, owner_operation, error)
            })
    }

    async fn read_by_cart(
        &self,
        context: PortContext,
        request: ReadCheckoutOrderIdentityByCartRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError> {
        let owner_operation = "read_checkout_identity_by_cart";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        self.journal
            .get_by_cart(tenant_id, request.cart_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(|error| {
                order_checkout_identity_error_to_port_error(&context, owner_operation, error)
            })
    }

    async fn bind(
        &self,
        context: PortContext,
        request: BindCheckoutOrderIdentityRequest,
    ) -> Result<CheckoutOrderIdentitySnapshot, PortError> {
        let owner_operation = "bind_checkout_identity";
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        self.journal
            .record(RecordOrderCheckoutIdentity {
                tenant_id,
                checkout_operation_id: request.checkout_operation_id,
                order_id: request.order_id,
                source_cart_id: request.cart_id,
                payment_collection_id: request.payment_collection_id,
                shipping_option_id: request.shipping_option_id,
                snapshot_hash: request.snapshot_hash,
                request_hash: request.request_hash,
            })
            .await
            .map(Into::into)
            .map_err(|error| {
                order_checkout_identity_error_to_port_error(&context, owner_operation, error)
            })
    }

    async fn adopt_legacy(
        &self,
        context: PortContext,
        request: AdoptLegacyCheckoutOrderIdentityRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError> {
        let owner_operation = "adopt_legacy_checkout_identity";
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        if let Some(existing) = self
            .journal
            .get_by_operation(tenant_id, request.checkout_operation_id)
            .await
            .map_err(|error| {
                order_checkout_identity_error_to_port_error(&context, owner_operation, error)
            })?
        {
            if existing.source_cart_id.is_some() && existing.source_cart_id != Some(request.cart_id)
            {
                return Err(PortError::conflict(
                    "order.checkout_identity_cart_conflict",
                    "checkout operation is already bound to another cart",
                ));
            }
            return Ok(Some(existing.into()));
        }

        let candidate = find_legacy_checkout_order_candidate(
            &self.db,
            tenant_id,
            request.checkout_operation_id,
        )
        .await
        .map_err(|_| {
            let facts = OrderPortErrorFacts {
                error_variant: "checkout_identity_storage",
                text_field_count: 0,
                text_total_length: 0,
                uuid_field_count: 0,
                uuid_non_nil_count: 0,
                opaque_payload_present: true,
            };
            log_order_port_failure(
                context,
                owner_operation,
                "order.checkout_identity_storage_unavailable",
                &facts,
                true,
            );
            PortError::unavailable(
                "order.checkout_identity_storage_unavailable",
                "order checkout identity storage is temporarily unavailable",
            )
        })?;
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let snapshot_hash = candidate.snapshot_hash.ok_or_else(|| {
            PortError::conflict(
                "order.checkout_identity_snapshot_missing",
                "legacy checkout order has no immutable snapshot hash",
            )
        })?;
        let request_hash = candidate.request_hash.ok_or_else(|| {
            PortError::conflict(
                "order.checkout_identity_request_hash_missing",
                "legacy checkout order has no immutable order request hash",
            )
        })?;
        self.journal
            .record(RecordOrderCheckoutIdentity {
                tenant_id,
                checkout_operation_id: request.checkout_operation_id,
                order_id: candidate.order_id,
                source_cart_id: request.cart_id,
                payment_collection_id: candidate.payment_collection_id,
                shipping_option_id: candidate.shipping_option_id,
                snapshot_hash,
                request_hash,
            })
            .await
            .map(Into::into)
            .map(Some)
            .map_err(|error| {
                order_checkout_identity_error_to_port_error(&context, owner_operation, error)
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadCheckoutOrderIdentityByOperationRequest {
    pub checkout_operation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadCheckoutOrderIdentityByCartRequest {
    pub cart_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BindCheckoutOrderIdentityRequest {
    pub checkout_operation_id: Uuid,
    pub order_id: Uuid,
    pub cart_id: Uuid,
    pub payment_collection_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub snapshot_hash: String,
    pub request_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdoptLegacyCheckoutOrderIdentityRequest {
    pub checkout_operation_id: Uuid,
    pub cart_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutOrderIdentitySnapshot {
    pub checkout_operation_id: Uuid,
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub source_cart_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub snapshot_hash: Option<String>,
    pub request_hash: Option<String>,
}

impl From<crate::entities::order_checkout_identity::Model> for CheckoutOrderIdentitySnapshot {
    fn from(value: crate::entities::order_checkout_identity::Model) -> Self {
        Self {
            checkout_operation_id: value.checkout_operation_id,
            tenant_id: value.tenant_id,
            order_id: value.order_id,
            source_cart_id: value.source_cart_id,
            payment_collection_id: value.payment_collection_id,
            shipping_option_id: value.shipping_option_id,
            snapshot_hash: value.snapshot_hash,
            request_hash: value.request_hash,
        }
    }
}

struct LegacyCheckoutOrderCandidate {
    order_id: Uuid,
    payment_collection_id: Option<Uuid>,
    shipping_option_id: Option<Uuid>,
    snapshot_hash: Option<String>,
    request_hash: Option<String>,
}

async fn find_legacy_checkout_order_candidate<C>(
    conn: &C,
    tenant_id: Uuid,
    checkout_operation_id: Uuid,
) -> Result<Option<LegacyCheckoutOrderCandidate>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let orders = crate::entities::order::Entity::find()
        .filter(crate::entities::order::Column::TenantId.eq(tenant_id))
        .all(conn)
        .await?;

    let op_str = checkout_operation_id.to_string();
    let mut matches = orders
        .into_iter()
        .filter_map(|order| {
            let checkout = order.metadata.get("checkout")?;
            let op_id = checkout.get("operation_id")?.as_str()?;
            if op_id == op_str {
                Some(LegacyCheckoutOrderCandidate {
                    order_id: order.id,
                    payment_collection_id: checkout
                        .get("payment_collection_id")
                        .and_then(serde_json::Value::as_str)
                        .and_then(|s| Uuid::parse_str(s).ok()),
                    shipping_option_id: checkout
                        .get("shipping_option_id")
                        .and_then(serde_json::Value::as_str)
                        .and_then(|s| Uuid::parse_str(s).ok()),
                    snapshot_hash: checkout
                        .get("snapshot_hash")
                        .and_then(serde_json::Value::as_str)
                        .map(String::from),
                    request_hash: checkout
                        .get("order_request_hash")
                        .and_then(serde_json::Value::as_str)
                        .map(String::from),
                })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if matches.len() > 1 {
        return Err(sea_orm::DbErr::Custom(
            "multiple orders are bound to one checkout operation".to_string(),
        ));
    }

    Ok(matches.pop())
}

fn order_checkout_identity_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: OrderCheckoutIdentityError,
) -> PortError {
    let facts = order_checkout_identity_error_facts(&error);
    let (code, technical_failure) = match &error {
        OrderCheckoutIdentityError::Validation(_) => ("order.checkout_identity_validation", false),
        OrderCheckoutIdentityError::Conflict(_) => ("order.checkout_identity_conflict", false),
        OrderCheckoutIdentityError::OrderNotFound(_) => {
            ("order.checkout_identity_order_not_found", false)
        }
        OrderCheckoutIdentityError::Database(_) => (
            "order.checkout_identity_storage_unavailable",
            true,
        ),
    };
    log_order_port_failure(context, owner_operation, code, &facts, technical_failure);
    match error {
        OrderCheckoutIdentityError::Validation(_) => PortError::validation(
            "order.checkout_identity_validation",
            "checkout order identity request is invalid",
        ),
        OrderCheckoutIdentityError::Conflict(_) => PortError::conflict(
            "order.checkout_identity_conflict",
            "checkout order identity conflicts with an existing order binding",
        ),
        OrderCheckoutIdentityError::OrderNotFound(_) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "order.checkout_identity_order_not_found",
            "order for checkout identity was not found",
            false,
        ),
        OrderCheckoutIdentityError::Database(_) => PortError::unavailable(
            "order.checkout_identity_storage_unavailable",
            "order checkout identity storage is temporarily unavailable",
        ),
    }
}


/// Transport-neutral owner boundary for checkout completion and recovery reads.
#[async_trait]
pub trait CheckoutCompletionPort: Send + Sync {
    async fn complete_checkout(
        &self,
        context: PortContext,
        request: CompleteCheckoutPortRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError>;

    async fn read_checkout_result(
        &self,
        context: PortContext,
        request: CheckoutResultRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError>;

    async fn read_checkout_result_by_operation(
        &self,
        context: PortContext,
        request: CheckoutResultByOperationRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError>;

    async fn read_order_status(
        &self,
        context: PortContext,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusSnapshot, PortError>;
}

pub struct InProcessCheckoutCompletionPort {
    order_service: OrderService,
    identity_port: InProcessCheckoutOrderIdentityPort,
}

struct ExistingCompletionContext<'a> {
    owner_operation: &'static str,
    tenant_id: Uuid,
    actor_id: Uuid,
    identity: &'a CheckoutOrderIdentitySnapshot,
    locale: Option<&'a str>,
    fallback_locale: Option<&'a str>,
}

impl InProcessCheckoutCompletionPort {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> InProcessCheckoutCompletionPort {
        Self {
            order_service: OrderService::new(db.clone(), event_bus),
            identity_port: InProcessCheckoutOrderIdentityPort::new(db),
        }
    }

    async fn read_identity_by_operation(
        &self,
        context: &PortContext,
        checkout_operation_id: Uuid,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError> {
        self.identity_port
            .read_by_operation(
                context.clone(),
                ReadCheckoutOrderIdentityByOperationRequest {
                    checkout_operation_id,
                },
            )
            .await
    }

    async fn adopt_legacy_identity(
        &self,
        context: &PortContext,
        checkout_operation_id: Uuid,
        cart_id: Uuid,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError> {
        self.identity_port
            .adopt_legacy(
                context.clone(),
                AdoptLegacyCheckoutOrderIdentityRequest {
                    checkout_operation_id,
                    cart_id,
                },
            )
            .await
    }

    async fn resolve_existing_completion(
        &self,
        context: &PortContext,
        completion: ExistingCompletionContext<'_>,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        let mut order = self
            .load_order(
                context,
                completion.owner_operation,
                completion.tenant_id,
                completion.identity.order_id,
                completion.locale,
                completion.fallback_locale,
            )
            .await?;
        match order.status_kind() {
            OrderStatusKind::Pending => {
                order = self
                    .order_service
                    .confirm_order(completion.tenant_id, completion.actor_id, order.id)
                    .await
                    .map_err(|error| {
                        order_error_to_port_error(context, completion.owner_operation, error)
                    })?;
                if let Some(locale) = completion.locale {
                    order = self
                        .order_service
                        .get_order_with_locale_fallback(
                            completion.tenant_id,
                            order.id,
                            locale,
                            completion.fallback_locale,
                        )
                        .await
                        .map_err(|error| {
                            order_error_to_port_error(context, completion.owner_operation, error)
                        })?;
                }
            }
            OrderStatusKind::Confirmed
            | OrderStatusKind::Paid
            | OrderStatusKind::Shipped
            | OrderStatusKind::Delivered => {}
            OrderStatusKind::Cancelled => {
                return Err(PortError::conflict(
                    "order.checkout_order_cancelled",
                    "checkout order is already cancelled",
                ));
            }
            OrderStatusKind::Unknown => {
                return Err(PortError::invariant_violation(
                    "order.checkout_order_status_invalid",
                    "checkout order has an unsupported lifecycle state",
                ));
            }
        }
        Ok(CheckoutCompletionSnapshot::from_response(
            &order,
            completion.identity.payment_collection_id,
        ))
    }

    async fn load_order(
        &self,
        context: &PortContext,
        owner_operation: &'static str,
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
        .map_err(|error| order_error_to_port_error(context, owner_operation, error))
    }
}

pub fn in_process_checkout_completion_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutCompletionPort> {
    Arc::new(InProcessCheckoutCompletionPort::new(db, event_bus))
}

#[async_trait]
impl CheckoutCompletionPort for crate::InProcessCheckoutCompletionPort {
    async fn complete_checkout(
        &self,
        context: PortContext,
        mut request: CompleteCheckoutPortRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        let owner_operation = "complete_checkout";
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let actor_id = parse_port_actor_id(&context, owner_operation)?;
        let checkout_operation_id = parse_checkout_operation_id(&context, owner_operation)?;
        let (snapshot_hash, request_hash) =
            checkout_request_hashes(&context, owner_operation, &request)?;

        if let Some(identity) = self
            .read_identity_by_operation(&context, checkout_operation_id)
            .await?
        {
            validate_completion_identity(
                &identity,
                tenant_id,
                checkout_operation_id,
                &request,
                snapshot_hash.as_str(),
                request_hash.as_str(),
            )?;
            return self
                .resolve_existing_completion(
                    &context,
                    ExistingCompletionContext {
                        owner_operation,
                        tenant_id,
                        actor_id,
                        identity: &identity,
                        locale: request.locale.as_deref(),
                        fallback_locale: request.fallback_locale.as_deref(),
                    },
                )
                .await;
        }

        if let Some(identity) = self
            .adopt_legacy_identity(&context, checkout_operation_id, request.cart_id)
            .await?
        {
            validate_completion_identity(
                &identity,
                tenant_id,
                checkout_operation_id,
                &request,
                snapshot_hash.as_str(),
                request_hash.as_str(),
            )?;
            return self
                .resolve_existing_completion(
                    &context,
                    ExistingCompletionContext {
                        owner_operation,
                        tenant_id,
                        actor_id,
                        identity: &identity,
                        locale: request.locale.as_deref(),
                        fallback_locale: request.fallback_locale.as_deref(),
                    },
                )
                .await;
        }

        attach_checkout_owner_metadata(
            &mut request.metadata,
            checkout_operation_id,
            request.cart_id,
            request.payment_collection_id,
            request.shipping_option_id,
            snapshot_hash.as_str(),
            request_hash.as_str(),
        )?;

        let create_input = crate::CreateOrderInput {
            customer_id: request.customer_id,
            currency_code: request.currency_code.clone(),
            shipping_total: request.shipping_total,
            line_items: request.line_items.clone(),
            adjustments: request.adjustments.clone(),
            tax_lines: request.tax_lines.clone(),
            metadata: request.metadata.clone(),
        };
        let create_result = self
            .order_service
            .create_order_with_channel(
                tenant_id,
                actor_id,
                create_input,
                request.channel_id,
                request.channel_slug.clone(),
            )
            .await;

        let (order, identity) = match create_result {
            Ok(order) => {
                let bind_result = self
                    .identity_port
                    .bind(
                        context.clone(),
                        BindCheckoutOrderIdentityRequest {
                            checkout_operation_id,
                            order_id: order.id,
                            cart_id: request.cart_id,
                            payment_collection_id: request.payment_collection_id,
                            shipping_option_id: request.shipping_option_id,
                            snapshot_hash: snapshot_hash.clone(),
                            request_hash: request_hash.clone(),
                        },
                    )
                    .await;
                match bind_result {
                    Ok(identity) => (order, identity),
                    Err(bind_error) => {
                        let Some(identity) = self
                            .read_identity_by_operation(&context, checkout_operation_id)
                            .await?
                            .or(self
                                .adopt_legacy_identity(
                                    &context,
                                    checkout_operation_id,
                                    request.cart_id,
                                )
                                .await?)
                        else {
                            return Err(bind_error);
                        };
                        validate_completion_identity(
                            &identity,
                            tenant_id,
                            checkout_operation_id,
                            &request,
                            snapshot_hash.as_str(),
                            request_hash.as_str(),
                        )?;
                        if identity.order_id != order.id {
                            return Err(PortError::conflict(
                                "order.checkout_concurrent_order_conflict",
                                "checkout operation resolved to another order during identity binding",
                            ));
                        }
                        (order, identity)
                    }
                }
            }
            Err(create_error) => {
                let Some(identity) = self
                    .adopt_legacy_identity(&context, checkout_operation_id, request.cart_id)
                    .await?
                else {
                    return Err(order_error_to_port_error(
                        &context,
                        owner_operation,
                        create_error,
                    ));
                };
                validate_completion_identity(
                    &identity,
                    tenant_id,
                    checkout_operation_id,
                    &request,
                    snapshot_hash.as_str(),
                    request_hash.as_str(),
                )?;
                let order = self
                    .load_order(
                        &context,
                        owner_operation,
                        tenant_id,
                        identity.order_id,
                        request.locale.as_deref(),
                        request.fallback_locale.as_deref(),
                    )
                    .await?;
                (order, identity)
            }
        };

        if identity.order_id != order.id {
            return Err(PortError::conflict(
                "order.checkout_identity_order_conflict",
                "checkout identity is bound to another order",
            ));
        }
        self.resolve_existing_completion(
            &context,
            ExistingCompletionContext {
                owner_operation,
                tenant_id,
                actor_id,
                identity: &identity,
                locale: request.locale.as_deref(),
                fallback_locale: request.fallback_locale.as_deref(),
            },
        )
        .await
    }

    async fn read_checkout_result(
        &self,
        context: PortContext,
        request: CheckoutResultRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        let owner_operation = "read_checkout_result";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let identity = self
            .identity_port
            .read_by_cart(
                context.clone(),
                ReadCheckoutOrderIdentityByCartRequest {
                    cart_id: request.cart_id,
                },
            )
            .await?
            .ok_or_else(|| {
                PortError::new(
                    rustok_api::PortErrorKind::NotFound,
                    "order.checkout_result_not_found",
                    "checkout result was not found for the requested cart",
                    false,
                )
            })?;
        let order = self
            .load_order(
                &context,
                owner_operation,
                tenant_id,
                identity.order_id,
                None,
                None,
            )
            .await?;
        Ok(CheckoutCompletionSnapshot::from_response(
            &order,
            identity.payment_collection_id,
        ))
    }

    async fn read_checkout_result_by_operation(
        &self,
        context: PortContext,
        request: CheckoutResultByOperationRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        let owner_operation = "read_checkout_result_by_operation";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
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
                PortError::new(
                    rustok_api::PortErrorKind::NotFound,
                    "order.checkout_result_not_found",
                    "checkout result was not found for the requested operation",
                    false,
                )
            })?;
        let order = self
            .load_order(
                &context,
                owner_operation,
                tenant_id,
                identity.order_id,
                None,
                None,
            )
            .await?;
        Ok(CheckoutCompletionSnapshot::from_response(
            &order,
            identity.payment_collection_id,
        ))
    }

    async fn read_order_status(
        &self,
        context: PortContext,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusSnapshot, PortError> {
        let owner_operation = "read_order_status";
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let response = self
            .order_service
            .get_order(tenant_id, request.order_id)
            .await
            .map_err(|error| order_error_to_port_error(&context, owner_operation, error))?;
        Ok(OrderStatusSnapshot::from_response(&response))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteCheckoutPortRequest {
    pub cart_id: Uuid,
    pub customer_id: Option<Uuid>,
    pub payment_collection_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
    pub currency_code: String,
    pub shipping_total: Decimal,
    pub line_items: Vec<crate::CreateOrderLineItemInput>,
    pub adjustments: Vec<crate::CreateOrderAdjustmentInput>,
    pub tax_lines: Vec<crate::CreateOrderTaxLineInput>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutResultRequest {
    pub cart_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckoutResultByOperationRequest {
    pub checkout_operation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrderStatusRequest {
    pub order_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckoutCompletionSnapshot {
    pub order_id: Uuid,
    pub status: String,
    pub currency_code: String,
    pub total: Decimal,
    pub payment_collection_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderStatusSnapshot {
    pub order_id: Uuid,
    pub status: String,
    pub paid: bool,
    pub shipped: bool,
    pub delivered: bool,
    pub total_amount: Decimal,
}

impl OrderStatusSnapshot {
    pub fn from_response(response: &OrderResponse) -> Self {
        Self {
            order_id: response.id,
            status: response.status.clone(),
            paid: response.paid_at.is_some(),
            shipped: response.shipped_at.is_some(),
            delivered: response.delivered_at.is_some(),
            total_amount: response.total_amount,
        }
    }
}

impl CheckoutCompletionSnapshot {
    pub fn from_response(response: &OrderResponse, payment_collection_id: Option<Uuid>) -> Self {
        Self {
            order_id: response.id,
            status: response.status.clone(),
            currency_code: response.currency_code.clone(),
            total: response.total_amount,
            payment_collection_id,
        }
    }
}

fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        log_order_context_rejection(
            context,
            owner_operation,
            "order.tenant_id_invalid",
            "tenant_id",
        );
        PortError::validation(
            "order.tenant_id_invalid",
            "order request context is invalid",
        )
    })
}


fn parse_port_actor_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.trim()).map_err(|_| {
        log_order_context_rejection(
            context,
            owner_operation,
            "order.actor_id_invalid",
            "actor_id",
        );
        PortError::validation("order.actor_id_invalid", "order request context is invalid")
    })
}


fn parse_checkout_operation_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    let value = context.causation_id.as_deref().ok_or_else(|| {
        log_order_context_rejection(
            context,
            owner_operation,
            "order.checkout_operation_id_required",
            "causation_id",
        );
        PortError::validation(
            "order.checkout_operation_id_required",
            "order request context is invalid",
        )
    })?;
    Uuid::parse_str(value).map_err(|_| {
        log_order_context_rejection(
            context,
            owner_operation,
            "order.checkout_operation_id_invalid",
            "causation_id",
        );
        PortError::validation(
            "order.checkout_operation_id_invalid",
            "order request context is invalid",
        )
    })
}


fn checkout_request_hashes(
    context: &PortContext,
    owner_operation: &'static str,
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
        let facts = order_port_context_facts(context);
        tracing::error!(
            owner = "rustok_order",
            owner_operation,
            correlation_id = %context.correlation_id,
            correlation_id_length = facts.correlation_id_length,
            tenant_id_length = facts.tenant_id_length,
            operation = owner_operation,
            code = "order.checkout_request_encoding_failed",
            encoding_failed = true,
            boundary = ORDER_PORT_BOUNDARY,
            "failed to encode checkout completion request"
        );
        PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "order.checkout_request_encoding_failed",
            "checkout completion request could not be encoded",
            false,
        )
    })?;
    Ok((
        hash_json(context, owner_operation, snapshot)?,
        hash_json(context, owner_operation, full_request)?,
    ))
}


fn hash_json(
    context: &PortContext,
    owner_operation: &'static str,
    value: Value,
) -> Result<String, PortError> {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).map_err(|_| {
        let facts = order_port_context_facts(context);
        tracing::error!(
            owner = "rustok_order",
            owner_operation,
            correlation_id = %context.correlation_id,
            correlation_id_length = facts.correlation_id_length,
            tenant_id_length = facts.tenant_id_length,
            operation = owner_operation,
            code = "order.checkout_request_encoding_failed",
            encoding_failed = true,
            boundary = ORDER_PORT_BOUNDARY,
            "failed to encode canonical checkout request"
        );
        PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "order.checkout_request_encoding_failed",
            "checkout request could not be encoded",
            false,
        )
    })?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
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

fn attach_checkout_owner_metadata(
    metadata: &mut Value,
    checkout_operation_id: Uuid,
    cart_id: Uuid,
    payment_collection_id: Option<Uuid>,
    shipping_option_id: Option<Uuid>,
    snapshot_hash: &str,
    request_hash: &str,
) -> Result<(), PortError> {
    let root = metadata.as_object_mut().ok_or_else(|| {
        PortError::validation(
            "order.checkout_metadata_invalid",
            "checkout order metadata must be a JSON object",
        )
    })?;
    let checkout = root
        .entry("checkout".to_string())
        .or_insert_with(|| Value::Object(Default::default()))
        .as_object_mut()
        .ok_or_else(|| {
            PortError::validation(
                "order.checkout_metadata_invalid",
                "checkout metadata namespace must be a JSON object",
            )
        })?;
    checkout.insert(
        "operation_id".to_string(),
        Value::String(checkout_operation_id.to_string()),
    );
    checkout.insert("cart_id".to_string(), Value::String(cart_id.to_string()));
    checkout.insert(
        "snapshot_hash".to_string(),
        Value::String(snapshot_hash.to_string()),
    );
    checkout.insert(
        "order_request_hash".to_string(),
        Value::String(request_hash.to_string()),
    );
    match payment_collection_id {
        Some(id) => {
            checkout.insert(
                "payment_collection_id".to_string(),
                Value::String(id.to_string()),
            );
        }
        None => {
            checkout.remove("payment_collection_id");
        }
    }
    match shipping_option_id {
        Some(id) => {
            checkout.insert(
                "shipping_option_id".to_string(),
                Value::String(id.to_string()),
            );
        }
        None => {
            checkout.remove("shipping_option_id");
        }
    }
    Ok(())
}

fn validate_completion_identity(
    identity: &CheckoutOrderIdentitySnapshot,
    tenant_id: Uuid,
    checkout_operation_id: Uuid,
    request: &CompleteCheckoutPortRequest,
    snapshot_hash: &str,
    request_hash: &str,
) -> Result<(), PortError> {
    let matches = identity.tenant_id == tenant_id
        && identity.checkout_operation_id == checkout_operation_id
        && identity
            .source_cart_id
            .is_none_or(|cart_id| cart_id == request.cart_id)
        && identity
            .payment_collection_id
            .is_none_or(|id| Some(id) == request.payment_collection_id)
        && identity
            .shipping_option_id
            .is_none_or(|id| Some(id) == request.shipping_option_id)
        && identity.snapshot_hash.as_deref() == Some(snapshot_hash)
        && identity.request_hash.as_deref() == Some(request_hash);
    if !matches {
        return Err(PortError::conflict(
            "order.checkout_request_conflict",
            "checkout operation is already bound to a different completion request",
        ));
    }
    Ok(())
}

fn order_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: OrderError,
) -> PortError {
    let facts = order_error_facts(&error);
    let (code, technical_failure) = match &error {
        OrderError::Database(_) => ("order.database_unavailable", true),
        OrderError::OrderNotFound(_) => ("order.order_not_found", false),
        OrderError::Validation(_) => ("order.validation", false),
        OrderError::InvalidTransition { .. } => ("order.invalid_transition", false),
        OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_) => {
            ("order.related_resource_not_found", false)
        }
        OrderError::Core(_) => ("order.invariant_violation", true),
        OrderError::IdempotencyConflict => ("order.idempotency_conflict", false),
        OrderError::CommandReceiptCorrupt => ("order.command_receipt_corrupt", true),
    };
    log_order_port_failure(context, owner_operation, code, &facts, technical_failure);
    match error {
        OrderError::Database(_) => PortError::unavailable(
            "order.database_unavailable",
            "order storage is temporarily unavailable",
        ),
        OrderError::OrderNotFound(_) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "order.order_not_found",
            "order was not found",
            false,
        ),
        OrderError::Validation(_) => {
            PortError::validation("order.validation", "order request is invalid")
        }
        OrderError::InvalidTransition { .. } => PortError::conflict(
            "order.invalid_transition",
            "order lifecycle transition conflicts with the current state",
        ),
        OrderError::OrderReturnNotFound(_) | OrderError::OrderChangeNotFound(_) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "order.related_resource_not_found",
            "related order resource was not found",
            false,
        ),
        OrderError::Core(_) => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "order.invariant_violation",
            "order operation failed an internal invariant",
            false,
        ),
        OrderError::IdempotencyConflict => PortError::conflict(
            "order.idempotency_conflict",
            "order operation conflicts with an existing idempotency key",
        ),
        OrderError::CommandReceiptCorrupt => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "order.command_receipt_corrupt",
            "order command receipt requires operator review",
            false,
        ),
    }
}

