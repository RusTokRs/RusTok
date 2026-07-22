use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::services::{
    OrderCheckoutIdentityError, OrderCheckoutIdentityJournal, RecordOrderCheckoutIdentity,
};
use crate::OrderResponse;

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
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        self.journal
            .get_by_operation(tenant_id, request.checkout_operation_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(order_checkout_identity_error_to_port_error)
    }

    async fn read_by_cart(
        &self,
        context: PortContext,
        request: ReadCheckoutOrderIdentityByCartRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        self.journal
            .get_by_cart(tenant_id, request.cart_id)
            .await
            .map(|value| value.map(Into::into))
            .map_err(order_checkout_identity_error_to_port_error)
    }

    async fn bind(
        &self,
        context: PortContext,
        request: BindCheckoutOrderIdentityRequest,
    ) -> Result<CheckoutOrderIdentitySnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;
        self.journal
            .record(RecordOrderCheckoutIdentity {
                tenant_id,
                checkout_operation_id: request.checkout_operation_id,
                order_id: request.order_id,
                source_cart_id: request.cart_id,
                snapshot_hash: request.snapshot_hash,
                request_hash: request.request_hash,
            })
            .await
            .map(Into::into)
            .map_err(order_checkout_identity_error_to_port_error)
    }

    async fn adopt_legacy(
        &self,
        context: PortContext,
        request: AdoptLegacyCheckoutOrderIdentityRequest,
    ) -> Result<Option<CheckoutOrderIdentitySnapshot>, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;
        if let Some(existing) = self
            .journal
            .get_by_operation(tenant_id, request.checkout_operation_id)
            .await
            .map_err(order_checkout_identity_error_to_port_error)?
        {
            if existing.source_cart_id.is_some()
                && existing.source_cart_id != Some(request.cart_id)
            {
                return Err(PortError::new(
                    rustok_api::PortErrorKind::Conflict,
                    "order.checkout_identity_cart_conflict",
                    "checkout operation is already bound to another cart",
                    false,
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
        .map_err(|error| {
            tracing::error!(error = ?error, "failed to read legacy order checkout identity");
            PortError::unavailable(
                "order.checkout_identity_storage_unavailable",
                "order checkout identity storage is temporarily unavailable",
            )
        })?;
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let snapshot_hash = candidate.snapshot_hash.ok_or_else(|| {
            PortError::new(
                rustok_api::PortErrorKind::Conflict,
                "order.checkout_identity_snapshot_missing",
                "legacy checkout order has no immutable snapshot hash",
                false,
            )
        })?;
        let request_hash = candidate.request_hash.ok_or_else(|| {
            PortError::new(
                rustok_api::PortErrorKind::Conflict,
                "order.checkout_identity_request_hash_missing",
                "legacy checkout order has no immutable order request hash",
                false,
            )
        })?;
        self.journal
            .record(RecordOrderCheckoutIdentity {
                tenant_id,
                checkout_operation_id: request.checkout_operation_id,
                order_id: candidate.order_id,
                source_cart_id: request.cart_id,
                snapshot_hash,
                request_hash,
            })
            .await
            .map(Into::into)
            .map(Some)
            .map_err(order_checkout_identity_error_to_port_error)
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
            snapshot_hash: value.snapshot_hash,
            request_hash: value.request_hash,
        }
    }
}

struct LegacyCheckoutOrderCandidate {
    order_id: Uuid,
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
    let sql = match conn.get_database_backend() {
        DbBackend::Postgres => {
            "SELECT id, metadata #>> '{checkout,snapshot_hash}' AS snapshot_hash, metadata #>> '{checkout,order_request_hash}' AS request_hash FROM orders WHERE tenant_id = ? AND metadata #>> '{checkout,operation_id}' = ? LIMIT 2"
        }
        DbBackend::Sqlite => {
            "SELECT id, json_extract(metadata, '$.checkout.snapshot_hash') AS snapshot_hash, json_extract(metadata, '$.checkout.order_request_hash') AS request_hash FROM orders WHERE tenant_id = ? AND json_extract(metadata, '$.checkout.operation_id') = ? LIMIT 2"
        }
        DbBackend::MySql => {
            "SELECT id, JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.snapshot_hash')) AS snapshot_hash, JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_request_hash')) AS request_hash FROM orders WHERE tenant_id = ? AND JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')) = ? LIMIT 2"
        }
    };
    let rows = conn
        .query_all(Statement::from_sql_and_values(
            conn.get_database_backend(),
            sql,
            vec![tenant_id.into(), checkout_operation_id.to_string().into()],
        ))
        .await?;
    if rows.len() > 1 {
        return Err(sea_orm::DbErr::Custom(
            "multiple orders are bound to one checkout operation".to_string(),
        ));
    }
    rows.into_iter()
        .next()
        .map(|row| {
            Ok(LegacyCheckoutOrderCandidate {
                order_id: row.try_get("", "id")?,
                snapshot_hash: row.try_get("", "snapshot_hash")?,
                request_hash: row.try_get("", "request_hash")?,
            })
        })
        .transpose()
}

fn order_checkout_identity_error_to_port_error(error: OrderCheckoutIdentityError) -> PortError {
    match error {
        OrderCheckoutIdentityError::Validation(message) => {
            PortError::validation("order.checkout_identity_validation", message)
        }
        OrderCheckoutIdentityError::Conflict(_) => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "order.checkout_identity_conflict",
            "checkout order identity conflicts with an existing order binding",
            false,
        ),
        OrderCheckoutIdentityError::OrderNotFound(_) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "order.checkout_identity_order_not_found",
            "order for checkout identity was not found",
            false,
        ),
        OrderCheckoutIdentityError::Database(error) => {
            tracing::error!(error = ?error, "order checkout identity storage failed");
            PortError::unavailable(
                "order.checkout_identity_storage_unavailable",
                "order checkout identity storage is temporarily unavailable",
            )
        }
    }
}

/// Transport-neutral owner boundary for checkout completion/result reads.
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

    async fn read_order_status(
        &self,
        context: PortContext,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusSnapshot, PortError>;
}

#[async_trait]
impl CheckoutCompletionPort for crate::OrderService {
    async fn complete_checkout(
        &self,
        context: PortContext,
        request: CompleteCheckoutPortRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let actor_id = parse_port_actor_id(&context)?;
        let CompleteCheckoutPortRequest {
            cart_id: _,
            customer_id,
            payment_collection_id,
            shipping_option_id: _,
            channel_id,
            channel_slug,
            locale,
            fallback_locale,
            currency_code,
            shipping_total,
            line_items,
            adjustments,
            tax_lines,
            metadata,
        } = request;
        let mut response = self
            .create_order_with_channel(
                tenant_id,
                actor_id,
                crate::CreateOrderInput {
                    customer_id,
                    currency_code,
                    shipping_total,
                    line_items,
                    adjustments,
                    tax_lines,
                    metadata,
                },
                channel_id,
                channel_slug,
            )
            .await
            .map_err(order_error_to_port_error)?;
        response = self
            .confirm_order(tenant_id, actor_id, response.id)
            .await
            .map_err(order_error_to_port_error)?;
        if let Some(locale) = locale.as_deref() {
            response = self
                .get_order_with_locale_fallback(
                    tenant_id,
                    response.id,
                    locale,
                    fallback_locale.as_deref(),
                )
                .await
                .map_err(order_error_to_port_error)?;
        }
        Ok(CheckoutCompletionSnapshot::from_response(
            &response,
            payment_collection_id,
        ))
    }

    async fn read_checkout_result(
        &self,
        context: PortContext,
        request: CheckoutResultRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let _tenant_id = parse_port_tenant_id(&context)?;
        let _cart_id = request.cart_id;
        Err(PortError::unavailable(
            "order.checkout_result_projection_unavailable",
            "checkout result lookup by cart id is not exposed by the current order storage projection",
        ))
    }

    async fn read_order_status(
        &self,
        context: PortContext,
        request: OrderStatusRequest,
    ) -> Result<OrderStatusSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let response = self
            .get_order(tenant_id, request.order_id)
            .await
            .map_err(order_error_to_port_error)?;
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

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "order.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for order ports",
        )
    })
}

fn parse_port_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.actor.id).map_err(|_| {
        PortError::validation(
            "order.actor_id_invalid",
            "PortContext.actor.id must be a UUID for order write ports",
        )
    })
}

fn order_error_to_port_error(error: crate::OrderError) -> PortError {
    match error {
        crate::OrderError::Database(error) => PortError::unavailable(
            "order.database_unavailable",
            format!("order storage unavailable: {error}"),
        ),
        crate::OrderError::OrderNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "order.order_not_found",
            format!("order {id} not found"),
            false,
        ),
        crate::OrderError::Validation(message) => {
            PortError::validation("order.validation", message)
        }
        other => PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "order.invariant_violation",
            other.to_string(),
            false,
        ),
    }
}
