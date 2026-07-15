use rustok_order::{CreateOrderInput, OrderError, OrderResponse, OrderService};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

use super::{
    CheckoutInventoryOrderAdoptionError, CheckoutInventoryOrderAdoptionService,
    CheckoutOperationError, CheckoutOperationJournal, CheckoutOperationStage,
    CheckoutOperationStatus,
};

#[derive(Debug, Error)]
pub enum CheckoutOrderCreationError {
    #[error(transparent)]
    Order(#[from] OrderError),
    #[error(transparent)]
    Operation(#[from] CheckoutOperationError),
    #[error(transparent)]
    Adoption(#[from] CheckoutInventoryOrderAdoptionError),
    #[error("checkout order creation conflict: {0}")]
    Conflict(String),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub type CheckoutOrderCreationResult<T> = Result<T, CheckoutOrderCreationError>;

pub struct CheckoutOrderCreationExecutor {
    db: DatabaseConnection,
    order_service: OrderService,
    operation_journal: CheckoutOperationJournal,
    adoption_service: CheckoutInventoryOrderAdoptionService,
}

impl CheckoutOrderCreationExecutor {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self {
            order_service: OrderService::new(db.clone(), event_bus),
            operation_journal: CheckoutOperationJournal::new(db.clone()),
            adoption_service: CheckoutInventoryOrderAdoptionService::new(db.clone()),
            db,
        }
    }

    /// Creates one pending order for a durable checkout operation, adopts the
    /// already reserved inventory rows into its order lines, and checkpoints
    /// `order_created`.
    ///
    /// The order identity is stored in metadata and protected by an owner-owned
    /// unique expression index. A concurrent or crash replay loads the existing
    /// order and continues adoption instead of creating a second aggregate.
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

        let existing_id = find_order_id_by_operation(&self.db, tenant_id, operation_id).await?;
        let order = match existing_id {
            Some(order_id) => {
                self.order_service
                    .get_order_with_locale_fallback(tenant_id, order_id, locale, fallback_locale)
                    .await?
            }
            None => {
                let create_result = self
                    .order_service
                    .create_order_with_channel(tenant_id, actor_id, input, channel_id, channel_slug)
                    .await;
                match create_result {
                    Ok(order) => order,
                    Err(error) => {
                        let Some(order_id) =
                            find_order_id_by_operation(&self.db, tenant_id, operation_id).await?
                        else {
                            return Err(error.into());
                        };
                        self.order_service
                            .get_order_with_locale_fallback(
                                tenant_id,
                                order_id,
                                locale,
                                fallback_locale,
                            )
                            .await?
                    }
                }
            }
        };

        validate_existing_order(
            &order,
            tenant_id,
            operation_id,
            snapshot_hash,
            request_hash.as_str(),
        )?;
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

async fn find_order_id_by_operation<C>(
    conn: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> Result<Option<Uuid>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let sql = match conn.get_database_backend() {
        DbBackend::Postgres => {
            "SELECT id FROM orders WHERE tenant_id = ? AND metadata #>> '{checkout,operation_id}' = ? LIMIT 2"
        }
        DbBackend::Sqlite => {
            "SELECT id FROM orders WHERE tenant_id = ? AND json_extract(metadata, '$.checkout.operation_id') = ? LIMIT 2"
        }
        DbBackend::MySql => {
            "SELECT id FROM orders WHERE tenant_id = ? AND JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')) = ? LIMIT 2"
        }
    };
    let rows = conn
        .query_all(Statement::from_sql_and_values(
            conn.get_database_backend(),
            sql,
            vec![tenant_id.into(), operation_id.to_string().into()],
        ))
        .await?;
    if rows.len() > 1 {
        return Err(sea_orm::DbErr::Custom(format!(
            "multiple orders are bound to checkout operation {operation_id}"
        )));
    }
    rows.into_iter()
        .next()
        .map(|row| row.try_get("", "id"))
        .transpose()
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

fn validate_existing_order(
    order: &OrderResponse,
    tenant_id: Uuid,
    operation_id: Uuid,
    snapshot_hash: &str,
    request_hash: &str,
) -> CheckoutOrderCreationResult<()> {
    if order.tenant_id != tenant_id {
        return Err(CheckoutOrderCreationError::Conflict(format!(
            "order {} belongs to another tenant",
            order.id
        )));
    }
    let checkout = order
        .metadata
        .get("checkout")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            CheckoutOrderCreationError::Conflict(format!(
                "order {} has no checkout identity metadata",
                order.id
            ))
        })?;
    let operation_id = operation_id.to_string();
    if checkout.get("operation_id").and_then(Value::as_str) != Some(operation_id.as_str())
        || checkout.get("snapshot_hash").and_then(Value::as_str) != Some(snapshot_hash)
        || checkout.get("order_request_hash").and_then(Value::as_str) != Some(request_hash)
    {
        return Err(CheckoutOrderCreationError::Conflict(format!(
            "order {} is bound to another checkout request",
            order.id
        )));
    }
    Ok(())
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
