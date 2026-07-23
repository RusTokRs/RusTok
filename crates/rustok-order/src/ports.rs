use async_trait::async_trait;
use rust_decimal::Decimal;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

use crate::services::{
    OrderCheckoutIdentityError, OrderCheckoutIdentityJournal, RecordOrderCheckoutIdentity,
};
use crate::{OrderError, OrderResponse, OrderService};

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
                payment_collection_id: request.payment_collection_id,
                shipping_option_id: request.shipping_option_id,
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
        .map_err(|error| {
            tracing::error!(
                correlation_id = %context.correlation_id,
                error = ?error,
                "failed to read legacy order checkout identity"
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
    let sql = match conn.get_database_backend() {
        DbBackend::Postgres => {
            "SELECT id, metadata #>> '{checkout,payment_collection_id}' AS payment_collection_id, metadata #>> '{checkout,shipping_option_id}' AS shipping_option_id, metadata #>> '{checkout,snapshot_hash}' AS snapshot_hash, metadata #>> '{checkout,order_request_hash}' AS request_hash FROM orders WHERE tenant_id = ? AND metadata #>> '{checkout,operation_id}' = ? LIMIT 2"
        }
        DbBackend::Sqlite => {
            "SELECT id, json_extract(metadata, '$.checkout.payment_collection_id') AS payment_collection_id, json_extract(metadata, '$.checkout.shipping_option_id') AS shipping_option_id, json_extract(metadata, '$.checkout.snapshot_hash') AS snapshot_hash, json_extract(metadata, '$.checkout.order_request_hash') AS request_hash FROM orders WHERE tenant_id = ? AND json_extract(metadata, '$.checkout.operation_id') = ? LIMIT 2"
        }
        DbBackend::MySql => {
            "SELECT id, JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.payment_collection_id')) AS payment_collection_id, JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.shipping_option_id')) AS shipping_option_id, JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.snapshot_hash')) AS snapshot_hash, JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_request_hash')) AS request_hash FROM orders WHERE tenant_id = ? AND JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')) = ? LIMIT 2"
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
            let payment_collection_id: Option<String> = row.try_get("", "payment_collection_id")?;
            let shipping_option_id: Option<String> = row.try_get("", "shipping_option_id")?;
            Ok(LegacyCheckoutOrderCandidate {
                order_id: row.try_get("", "id")?,
                payment_collection_id: parse_optional_uuid(payment_collection_id),
                shipping_option_id: parse_optional_uuid(shipping_option_id),
                snapshot_hash: row.try_get("", "snapshot_hash")?,
                request_hash: row.try_get("", "request_hash")?,
            })
        })
        .transpose()
}

fn parse_optional_uuid(value: Option<String>) -> Option<Uuid> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn order_checkout_identity_error_to_port_error(error: OrderCheckoutIdentityError) -> PortError {
    match error {
        OrderCheckoutIdentityError::Validation(message) => {
            PortError::validation("order.checkout_identity_validation", message)
        }
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
        OrderCheckoutIdentityError::Database(error) => {
            tracing::error!(error = ?error, "order checkout identity storage failed");
            PortError::unavailable(
                "order.checkout_identity_storage_unavailable",
                "order checkout identity storage is temporarily unavailable",
            )
        }
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
        tenant_id: Uuid,
        actor_id: Uuid,
        identity: &CheckoutOrderIdentitySnapshot,
        locale: Option<&str>,
        fallback_locale: Option<&str>,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        let mut order = self
            .load_order(tenant_id, identity.order_id, locale, fallback_locale)
            .await?;
        match order.status.as_str() {
            "pending" => {
                order = self
                    .order_service
                    .confirm_order(tenant_id, actor_id, order.id)
                    .await
                    .map_err(order_error_to_port_error)?;
                if let Some(locale) = locale {
                    order = self
                        .order_service
                        .get_order_with_locale_fallback(
                            tenant_id,
                            order.id,
                            locale,
                            fallback_locale,
                        )
                        .await
                        .map_err(order_error_to_port_error)?;
                }
            }
            "confirmed" | "paid" | "shipped" | "delivered" => {}
            "cancelled" => {
                return Err(PortError::conflict(
                    "order.checkout_order_cancelled",
                    "checkout order is already cancelled",
                ));
            }
            _ => {
                return Err(PortError::new(
                    rustok_api::PortErrorKind::InvariantViolation,
                    "order.checkout_order_status_invalid",
                    "checkout order has an unsupported lifecycle state",
                    false,
                ));
            }
        }
        Ok(CheckoutCompletionSnapshot::from_response(
            &order,
            identity.payment_collection_id,
        ))
    }

    async fn load_order(
        &self,
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
        .map_err(order_error_to_port_error)
    }
}

pub fn in_process_checkout_completion_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn CheckoutCompletionPort> {
    Arc::new(InProcessCheckoutCompletionPort::new(db, event_bus))
}

#[async_trait]
impl CheckoutCompletionPort for InProcessCheckoutCompletionPort {
    async fn complete_checkout(
        &self,
        context: PortContext,
        mut request: CompleteCheckoutPortRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        context.require_write_semantics()?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let actor_id = parse_port_actor_id(&context)?;
        let checkout_operation_id = parse_checkout_operation_id(&context)?;
        let (snapshot_hash, request_hash) = checkout_request_hashes(&request)?;

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
                    tenant_id,
                    actor_id,
                    &identity,
                    request.locale.as_deref(),
                    request.fallback_locale.as_deref(),
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
                    tenant_id,
                    actor_id,
                    &identity,
                    request.locale.as_deref(),
                    request.fallback_locale.as_deref(),
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
                    return Err(order_error_to_port_error(create_error));
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
            tenant_id,
            actor_id,
            &identity,
            request.locale.as_deref(),
            request.fallback_locale.as_deref(),
        )
        .await
    }

    async fn read_checkout_result(
        &self,
        context: PortContext,
        request: CheckoutResultRequest,
    ) -> Result<CheckoutCompletionSnapshot, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let identity = self
            .identity_port
            .read_by_cart(
                context,
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
            .load_order(tenant_id, identity.order_id, None, None)
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
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let identity = self
            .identity_port
            .read_by_operation(
                context,
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
            .load_order(tenant_id, identity.order_id, None, None)
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
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        let response = self
            .order_service
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

fn parse_checkout_operation_id(context: &PortContext) -> Result<Uuid, PortError> {
    context
        .causation_id
        .as_deref()
        .ok_or_else(|| {
            PortError::validation(
                "order.checkout_operation_id_required",
                "checkout completion requires a UUID causation_id",
            )
        })
        .and_then(|value| {
            Uuid::parse_str(value).map_err(|_| {
                PortError::validation(
                    "order.checkout_operation_id_invalid",
                    "checkout completion causation_id must be a UUID",
                )
            })
        })
}

fn checkout_request_hashes(
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
        tracing::error!(error = ?error, "failed to encode checkout completion request");
        PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "order.checkout_request_encoding_failed",
            "checkout completion request could not be encoded",
            false,
        )
    })?;
    Ok((hash_json(snapshot)?, hash_json(full_request)?))
}

fn hash_json(value: Value) -> Result<String, PortError> {
    let canonical = canonicalize_json(value);
    let bytes = serde_json::to_vec(&canonical).map_err(|error| {
        tracing::error!(error = ?error, "failed to encode canonical checkout request");
        PortError::new(
            rustok_api::PortErrorKind::InvariantViolation,
            "order.checkout_request_encoding_failed",
            "checkout completion request could not be encoded",
            false,
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

fn order_error_to_port_error(error: OrderError) -> PortError {
    match error {
        OrderError::Database(error) => {
            tracing::error!(error = ?error, "order storage operation failed");
            PortError::unavailable(
                "order.database_unavailable",
                "order storage is temporarily unavailable",
            )
        }
        OrderError::OrderNotFound(_) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "order.order_not_found",
            "order was not found",
            false,
        ),
        OrderError::Validation(message) => PortError::validation("order.validation", message),
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
        OrderError::Core(error) => {
            tracing::error!(error = ?error, "order core operation failed");
            PortError::new(
                rustok_api::PortErrorKind::InvariantViolation,
                "order.invariant_violation",
                "order operation failed an internal invariant",
                false,
            )
        }
    }
}
