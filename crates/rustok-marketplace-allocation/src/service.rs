use std::collections::HashSet;

use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::dto::{
    AllocateMarketplaceOrderLineInput, AllocateMarketplaceOrderLinesInput,
    AllocateMarketplaceOrderLinesResponse, ListMarketplaceAllocationsBySellerRequest,
    MAX_ALLOCATION_LINES_PER_COMMAND, MAX_ALLOCATIONS_PER_PAGE, MarketplaceAllocationListResponse,
    MarketplaceAllocationStatus, MarketplaceOrderAllocationResponse,
};
use crate::entities::allocation;
use crate::error::{MarketplaceAllocationError, MarketplaceAllocationResult};
use crate::receipts::{
    AllocationReceiptAdmission, NewAllocationReceipt, admit_receipt, allocation_request_hash,
    complete_receipt, normalize_idempotency_key, replay_receipt, rollback_receipt,
};

pub struct MarketplaceAllocationService {
    db: DatabaseConnection,
}

impl MarketplaceAllocationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn allocate_order_lines_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: AllocateMarketplaceOrderLinesInput,
    ) -> MarketplaceAllocationResult<AllocateMarketplaceOrderLinesResponse> {
        let input = normalize_allocation_input(input)?;
        let key = normalize_idempotency_key(idempotency_key)?;
        let request_hash = allocation_request_hash(actor_id, &input)?;

        match admit_receipt(&self.db, tenant_id, actor_id, key, request_hash.as_str()).await? {
            AllocationReceiptAdmission::Replay(receipt) => {
                replay_receipt(receipt, request_hash.as_str())
            }
            AllocationReceiptAdmission::New(receipt) => {
                let result = allocate_in_transaction(&receipt, tenant_id, input).await;
                match result {
                    Ok(response) => complete_receipt(receipt, &response).await,
                    Err(error) => rollback_receipt(receipt, error).await,
                }
            }
        }
    }

    pub async fn get_by_order_line(
        &self,
        tenant_id: Uuid,
        order_line_item_id: Uuid,
    ) -> MarketplaceAllocationResult<MarketplaceOrderAllocationResponse> {
        allocation::Entity::find()
            .filter(allocation::Column::TenantId.eq(tenant_id))
            .filter(allocation::Column::OrderLineItemId.eq(order_line_item_id))
            .one(&self.db)
            .await?
            .ok_or(MarketplaceAllocationError::AllocationNotFound(
                order_line_item_id,
            ))
            .and_then(map_allocation)
    }

    pub async fn list_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> MarketplaceAllocationResult<Vec<MarketplaceOrderAllocationResponse>> {
        allocation::Entity::find()
            .filter(allocation::Column::TenantId.eq(tenant_id))
            .filter(allocation::Column::OrderId.eq(order_id))
            .order_by_asc(allocation::Column::CreatedAt)
            .order_by_asc(allocation::Column::Id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(map_allocation)
            .collect()
    }

    pub async fn list_by_seller(
        &self,
        tenant_id: Uuid,
        request: ListMarketplaceAllocationsBySellerRequest,
    ) -> MarketplaceAllocationResult<MarketplaceAllocationListResponse> {
        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, MAX_ALLOCATIONS_PER_PAGE);
        let mut query = allocation::Entity::find()
            .filter(allocation::Column::TenantId.eq(tenant_id))
            .filter(allocation::Column::SellerId.eq(request.seller_id));
        if let Some(status) = request.status {
            query = query.filter(allocation::Column::Status.eq(status.as_str()));
        }
        let paginator = query
            .order_by_desc(allocation::Column::CreatedAt)
            .order_by_desc(allocation::Column::Id)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await?
            .into_iter()
            .map(map_allocation)
            .collect::<MarketplaceAllocationResult<Vec<_>>>()?;
        Ok(MarketplaceAllocationListResponse {
            items,
            total,
            page,
            per_page,
        })
    }
}

async fn allocate_in_transaction(
    receipt: &NewAllocationReceipt,
    tenant_id: Uuid,
    input: AllocateMarketplaceOrderLinesInput,
) -> MarketplaceAllocationResult<AllocateMarketplaceOrderLinesResponse> {
    let line_ids = input
        .lines
        .iter()
        .map(|line| line.order_line_item_id)
        .collect::<Vec<_>>();
    if let Some(existing) = allocation::Entity::find()
        .filter(allocation::Column::TenantId.eq(tenant_id))
        .filter(allocation::Column::OrderLineItemId.is_in(line_ids))
        .order_by_asc(allocation::Column::OrderLineItemId)
        .one(&receipt.transaction)
        .await?
    {
        return Err(MarketplaceAllocationError::LineAlreadyAllocated(
            existing.order_line_item_id,
        ));
    }

    let now = Utc::now().fixed_offset();
    let mut allocations = Vec::with_capacity(input.lines.len());
    for line in input.lines {
        let order_line_item_id = line.order_line_item_id;
        let model = allocation::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            order_id: Set(input.order_id),
            order_line_item_id: Set(order_line_item_id),
            seller_id: Set(line.seller_id),
            listing_id: Set(line.listing_id),
            master_product_id: Set(line.master_product_id),
            master_variant_id: Set(line.master_variant_id),
            quantity: Set(line.quantity),
            currency_code: Set(input.currency_code.clone()),
            unit_amount: Set(line.unit_amount),
            subtotal_amount: Set(line.subtotal_amount),
            discount_amount: Set(line.discount_amount),
            tax_amount: Set(line.tax_amount),
            total_amount: Set(line.total_amount),
            listing_terms_version: Set(line.listing_terms_version),
            pricing_reference: Set(line.pricing_reference),
            inventory_reference: Set(line.inventory_reference),
            fulfillment_profile_slug: Set(line.fulfillment_profile_slug),
            status: Set(MarketplaceAllocationStatus::Allocated.as_str().to_string()),
            metadata: Set(line.metadata),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&receipt.transaction)
        .await
        .map_err(|error| {
            if is_unique_constraint(&error) {
                MarketplaceAllocationError::LineAlreadyAllocated(order_line_item_id)
            } else {
                error.into()
            }
        })?;
        allocations.push(map_allocation(model)?);
    }

    Ok(AllocateMarketplaceOrderLinesResponse {
        order_id: input.order_id,
        currency_code: input.currency_code,
        allocations,
    })
}

fn normalize_allocation_input(
    mut input: AllocateMarketplaceOrderLinesInput,
) -> MarketplaceAllocationResult<AllocateMarketplaceOrderLinesInput> {
    require_uuid(input.order_id, "order_id")?;
    input.currency_code = normalize_currency(input.currency_code)?;
    if input.lines.is_empty() || input.lines.len() > MAX_ALLOCATION_LINES_PER_COMMAND {
        return Err(MarketplaceAllocationError::Validation(format!(
            "allocation command must contain 1 to {MAX_ALLOCATION_LINES_PER_COMMAND} lines"
        )));
    }

    let mut line_ids = HashSet::with_capacity(input.lines.len());
    for line in &mut input.lines {
        normalize_line(line)?;
        if !line_ids.insert(line.order_line_item_id) {
            return Err(MarketplaceAllocationError::DuplicateLine(
                line.order_line_item_id,
            ));
        }
    }
    Ok(input)
}

fn normalize_line(line: &mut AllocateMarketplaceOrderLineInput) -> MarketplaceAllocationResult<()> {
    for (value, field) in [
        (line.order_line_item_id, "order_line_item_id"),
        (line.seller_id, "seller_id"),
        (line.listing_id, "listing_id"),
        (line.master_product_id, "master_product_id"),
        (line.master_variant_id, "master_variant_id"),
    ] {
        require_uuid(value, field)?;
    }
    if line.quantity <= 0 {
        return Err(MarketplaceAllocationError::Validation(
            "quantity must be greater than zero".to_string(),
        ));
    }
    if line.listing_terms_version <= 0 {
        return Err(MarketplaceAllocationError::Validation(
            "listing_terms_version must be greater than zero".to_string(),
        ));
    }
    for (value, field) in [
        (line.unit_amount, "unit_amount"),
        (line.subtotal_amount, "subtotal_amount"),
        (line.discount_amount, "discount_amount"),
        (line.tax_amount, "tax_amount"),
        (line.total_amount, "total_amount"),
    ] {
        if value < 0 {
            return Err(MarketplaceAllocationError::Validation(format!(
                "{field} must not be negative"
            )));
        }
    }
    let expected_subtotal = line
        .unit_amount
        .checked_mul(line.quantity)
        .ok_or_else(|| MarketplaceAllocationError::Validation("subtotal overflow".to_string()))?;
    if line.subtotal_amount != expected_subtotal {
        return Err(MarketplaceAllocationError::Validation(format!(
            "subtotal_amount must equal unit_amount * quantity for line {}",
            line.order_line_item_id
        )));
    }
    if line.discount_amount > line.subtotal_amount {
        return Err(MarketplaceAllocationError::Validation(format!(
            "discount_amount exceeds subtotal_amount for line {}",
            line.order_line_item_id
        )));
    }
    let expected_total = line
        .subtotal_amount
        .checked_sub(line.discount_amount)
        .and_then(|value| value.checked_add(line.tax_amount))
        .ok_or_else(|| MarketplaceAllocationError::Validation("total overflow".to_string()))?;
    if line.total_amount != expected_total {
        return Err(MarketplaceAllocationError::Validation(format!(
            "total_amount must equal subtotal_amount - discount_amount + tax_amount for line {}",
            line.order_line_item_id
        )));
    }
    line.pricing_reference =
        normalize_optional_text(line.pricing_reference.take(), 191, "pricing_reference")?;
    line.inventory_reference =
        normalize_optional_text(line.inventory_reference.take(), 191, "inventory_reference")?;
    line.fulfillment_profile_slug = normalize_optional_text(
        line.fulfillment_profile_slug.take(),
        120,
        "fulfillment_profile_slug",
    )?;
    line.metadata = normalize_metadata(std::mem::take(&mut line.metadata))?;
    Ok(())
}

fn require_uuid(value: Uuid, field: &str) -> MarketplaceAllocationResult<()> {
    if value.is_nil() {
        return Err(MarketplaceAllocationError::Validation(format!(
            "{field} must not be nil"
        )));
    }
    Ok(())
}

fn normalize_currency(value: String) -> MarketplaceAllocationResult<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(MarketplaceAllocationError::Validation(
            "currency_code must contain exactly three ASCII letters".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_optional_text(
    value: Option<String>,
    max_bytes: usize,
    field: &str,
) -> MarketplaceAllocationResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > max_bytes {
        return Err(MarketplaceAllocationError::Validation(format!(
            "{field} must not exceed {max_bytes} bytes"
        )));
    }
    Ok(Some(value.to_string()))
}

fn normalize_metadata(value: serde_json::Value) -> MarketplaceAllocationResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplaceAllocationError::Validation(
            "metadata must be a JSON object".to_string(),
        )),
    }
}

fn map_allocation(
    model: allocation::Model,
) -> MarketplaceAllocationResult<MarketplaceOrderAllocationResponse> {
    let status = MarketplaceAllocationStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceAllocationError::Validation(format!(
            "unknown marketplace allocation status `{}`",
            model.status
        ))
    })?;
    Ok(MarketplaceOrderAllocationResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        order_id: model.order_id,
        order_line_item_id: model.order_line_item_id,
        seller_id: model.seller_id,
        listing_id: model.listing_id,
        master_product_id: model.master_product_id,
        master_variant_id: model.master_variant_id,
        quantity: model.quantity,
        currency_code: model.currency_code,
        unit_amount: model.unit_amount,
        subtotal_amount: model.subtotal_amount,
        discount_amount: model.discount_amount,
        tax_amount: model.tax_amount,
        total_amount: model.total_amount,
        listing_terms_version: model.listing_terms_version,
        pricing_reference: model.pricing_reference,
        inventory_reference: model.inventory_reference,
        fulfillment_profile_slug: model.fulfillment_profile_slug,
        status,
        metadata: model.metadata,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
