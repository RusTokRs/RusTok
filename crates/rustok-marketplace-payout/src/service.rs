use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::Utc;
use rustok_api::PortContext;
use rustok_core::generate_id;
use rustok_marketplace_ledger::{
    ListMarketplaceSellerLedgerEntriesRequest, MarketplaceLedgerAccountCode,
    MarketplaceLedgerCommandPort, MarketplaceLedgerEntryDirection, MarketplaceLedgerEntryResponse,
    MarketplaceLedgerReadPort,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::dto::{
    ListMarketplaceSellerPayoutsRequest, MarketplacePayoutItemResponse,
    MarketplacePayoutListResponse, MarketplacePayoutResponse, MarketplacePayoutStatus,
    ScheduleMarketplacePayoutInput, MAX_PAYOUTS_PER_PAGE, MAX_PAYOUT_ITEMS_PER_BATCH,
};
use crate::entities::{item, payout};
use crate::error::{MarketplacePayoutError, MarketplacePayoutResult};
use crate::receipts::{
    admit_receipt, complete_receipt, normalize_idempotency_key, replay_existing, replay_receipt,
    rollback_receipt, schedule_request_hash, NewPayoutReceipt, PayoutReceiptAdmission,
};

const LEDGER_PAGE_SIZE: u64 = 200;

pub struct MarketplacePayoutService {
    db: DatabaseConnection,
    ledger_reader: Arc<dyn MarketplaceLedgerReadPort>,
    ledger_writer: Option<Arc<dyn MarketplaceLedgerCommandPort>>,
}

impl MarketplacePayoutService {
    pub fn new(db: DatabaseConnection, ledger_reader: Arc<dyn MarketplaceLedgerReadPort>) -> Self {
        Self {
            db,
            ledger_reader,
            ledger_writer: None,
        }
    }

    pub fn with_ledger_writer(
        mut self,
        ledger_writer: Arc<dyn MarketplaceLedgerCommandPort>,
    ) -> Self {
        self.ledger_writer = Some(ledger_writer);
        self
    }

    pub(crate) fn ledger_writer(
        &self,
    ) -> MarketplacePayoutResult<Arc<dyn MarketplaceLedgerCommandPort>> {
        self.ledger_writer
            .clone()
            .ok_or(MarketplacePayoutError::LedgerWriterNotConfigured)
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub(crate) async fn schedule_with_receipt(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: ScheduleMarketplacePayoutInput,
    ) -> MarketplacePayoutResult<MarketplacePayoutResponse> {
        let input = normalize_schedule_input(input)?;
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = schedule_request_hash(actor_id, &input)?;
        if let Some(response) =
            replay_existing(&self.db, tenant_id, key.as_str(), hash.as_str()).await?
        {
            return Ok(response);
        }

        let entries = self
            .load_selected_entries(context, tenant_id, &input)
            .await?;
        match admit_receipt(&self.db, tenant_id, actor_id, key, hash.as_str()).await? {
            PayoutReceiptAdmission::Replay(receipt) => replay_receipt(receipt, hash.as_str()),
            PayoutReceiptAdmission::New(receipt) => {
                let result = schedule_in_transaction(&receipt, tenant_id, input, entries).await;
                match result {
                    Ok(response) => complete_receipt(receipt, &response).await,
                    Err(error) => rollback_receipt(receipt, error).await,
                }
            }
        }
    }

    pub async fn read_payout(
        &self,
        tenant_id: Uuid,
        payout_id: Uuid,
    ) -> MarketplacePayoutResult<MarketplacePayoutResponse> {
        let payout = payout::Entity::find()
            .filter(payout::Column::TenantId.eq(tenant_id))
            .filter(payout::Column::Id.eq(payout_id))
            .one(&self.db)
            .await?
            .ok_or(MarketplacePayoutError::PayoutNotFound(payout_id))?;
        let items = item::Entity::find()
            .filter(item::Column::TenantId.eq(tenant_id))
            .filter(item::Column::PayoutId.eq(payout_id))
            .order_by_asc(item::Column::LedgerEntryId)
            .all(&self.db)
            .await?
            .into_iter()
            .map(map_item)
            .collect();
        map_payout(payout, items)
    }

    pub async fn list_seller_payouts(
        &self,
        tenant_id: Uuid,
        mut request: ListMarketplaceSellerPayoutsRequest,
    ) -> MarketplacePayoutResult<MarketplacePayoutListResponse> {
        if request.seller_id.is_nil() {
            return Err(MarketplacePayoutError::Validation(
                "seller_id must not be nil".to_string(),
            ));
        }
        request.currency_code = match request.currency_code.take() {
            Some(value) => Some(normalize_currency(value)?),
            None => None,
        };
        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, MAX_PAYOUTS_PER_PAGE);
        let mut query = payout::Entity::find()
            .filter(payout::Column::TenantId.eq(tenant_id))
            .filter(payout::Column::SellerId.eq(request.seller_id));
        if let Some(currency_code) = request.currency_code {
            query = query.filter(payout::Column::CurrencyCode.eq(currency_code));
        }
        if let Some(status) = request.status {
            query = query.filter(payout::Column::Status.eq(status.as_str()));
        }
        let paginator = query
            .order_by_desc(payout::Column::ScheduledFor)
            .order_by_desc(payout::Column::Id)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let payout_models = paginator.fetch_page(page.saturating_sub(1)).await?;
        let payout_ids = payout_models
            .iter()
            .map(|model| model.id)
            .collect::<Vec<_>>();
        let mut items_by_payout = item::Entity::find()
            .filter(item::Column::TenantId.eq(tenant_id))
            .filter(item::Column::PayoutId.is_in(payout_ids))
            .order_by_asc(item::Column::LedgerEntryId)
            .all(&self.db)
            .await?
            .into_iter()
            .fold(
                HashMap::<Uuid, Vec<MarketplacePayoutItemResponse>>::new(),
                |mut grouped, model| {
                    grouped
                        .entry(model.payout_id)
                        .or_default()
                        .push(map_item(model));
                    grouped
                },
            );
        let items = payout_models
            .into_iter()
            .map(|model| {
                let payout_items = items_by_payout.remove(&model.id).unwrap_or_default();
                map_payout(model, payout_items)
            })
            .collect::<MarketplacePayoutResult<Vec<_>>>()?;
        Ok(MarketplacePayoutListResponse {
            items,
            total,
            page,
            per_page,
        })
    }

    pub(crate) async fn load_selected_entries(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        input: &ScheduleMarketplacePayoutInput,
    ) -> MarketplacePayoutResult<Vec<MarketplaceLedgerEntryResponse>> {
        let wanted = input
            .ledger_entry_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        let mut found = HashMap::<Uuid, MarketplaceLedgerEntryResponse>::new();
        let mut page = 1_u64;
        loop {
            let response = self
                .ledger_reader
                .list_seller_entries(
                    context.clone(),
                    ListMarketplaceSellerLedgerEntriesRequest {
                        seller_id: input.seller_id,
                        currency_code: Some(input.currency_code.clone()),
                        page,
                        per_page: LEDGER_PAGE_SIZE,
                    },
                )
                .await
                .map_err(map_ledger_port_error)?;
            for entry in response.items {
                if wanted.contains(&entry.id) {
                    found.insert(entry.id, entry);
                }
            }
            if found.len() == wanted.len()
                || page.saturating_mul(response.per_page) >= response.total
            {
                break;
            }
            page = page.checked_add(1).ok_or_else(|| {
                MarketplacePayoutError::Validation(
                    "ledger pagination overflow while loading payout entries".to_string(),
                )
            })?;
        }

        let mut entries = Vec::with_capacity(input.ledger_entry_ids.len());
        for entry_id in &input.ledger_entry_ids {
            let entry = found
                .remove(entry_id)
                .ok_or(MarketplacePayoutError::LedgerEntryNotFound(*entry_id))?;
            validate_ledger_entry(tenant_id, input, &entry)?;
            entries.push(entry);
        }
        Ok(entries)
    }
}

async fn schedule_in_transaction(
    receipt: &NewPayoutReceipt,
    tenant_id: Uuid,
    input: ScheduleMarketplacePayoutInput,
    entries: Vec<MarketplaceLedgerEntryResponse>,
) -> MarketplacePayoutResult<MarketplacePayoutResponse> {
    let entry_ids = entries.iter().map(|entry| entry.id).collect::<Vec<_>>();
    if let Some(existing) = item::Entity::find()
        .filter(item::Column::TenantId.eq(tenant_id))
        .filter(item::Column::LedgerEntryId.is_in(entry_ids))
        .one(&receipt.transaction)
        .await?
    {
        return Err(MarketplacePayoutError::LedgerEntryAlreadyAssigned(
            existing.ledger_entry_id,
        ));
    }
    let total_amount = entries.iter().try_fold(0_i64, |total, entry| {
        total
            .checked_add(entry.amount)
            .ok_or_else(|| MarketplacePayoutError::Validation("payout total overflow".to_string()))
    })?;
    if total_amount <= 0 {
        return Err(MarketplacePayoutError::Validation(
            "payout total must be greater than zero".to_string(),
        ));
    }

    let payout_id = generate_id();
    let now = Utc::now().fixed_offset();
    let payout_model = payout::ActiveModel {
        id: Set(payout_id),
        tenant_id: Set(tenant_id),
        seller_id: Set(input.seller_id),
        currency_code: Set(input.currency_code),
        total_amount: Set(total_amount),
        status: Set(MarketplacePayoutStatus::Scheduled.as_str().to_string()),
        scheduled_for: Set(input.scheduled_for),
        destination_reference: Set(input.destination_reference),
        external_reference: Set(None),
        failure_code: Set(None),
        metadata: Set(input.metadata),
        created_at: Set(now),
        updated_at: Set(now),
        paid_at: Set(None),
    }
    .insert(&receipt.transaction)
    .await?;

    let mut payout_items = Vec::with_capacity(entries.len());
    for entry in entries {
        let model = item::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            payout_id: Set(payout_id),
            ledger_entry_id: Set(entry.id),
            amount: Set(entry.amount),
            created_at: Set(now),
        }
        .insert(&receipt.transaction)
        .await
        .map_err(|error| {
            if is_unique_constraint(&error) {
                MarketplacePayoutError::LedgerEntryAlreadyAssigned(entry.id)
            } else {
                error.into()
            }
        })?;
        payout_items.push(map_item(model));
    }
    payout_items.sort_by_key(|item| item.ledger_entry_id);
    map_payout(payout_model, payout_items)
}

pub(crate) fn normalize_schedule_input(
    mut input: ScheduleMarketplacePayoutInput,
) -> MarketplacePayoutResult<ScheduleMarketplacePayoutInput> {
    if input.seller_id.is_nil() {
        return Err(MarketplacePayoutError::Validation(
            "seller_id must not be nil".to_string(),
        ));
    }
    input.currency_code = normalize_currency(input.currency_code)?;
    if input.ledger_entry_ids.is_empty()
        || input.ledger_entry_ids.len() > MAX_PAYOUT_ITEMS_PER_BATCH
    {
        return Err(MarketplacePayoutError::Validation(format!(
            "payout must contain 1 to {MAX_PAYOUT_ITEMS_PER_BATCH} ledger entries"
        )));
    }
    if input.ledger_entry_ids.iter().any(Uuid::is_nil) {
        return Err(MarketplacePayoutError::Validation(
            "ledger_entry_ids must not contain nil UUIDs".to_string(),
        ));
    }
    input.ledger_entry_ids.sort_unstable();
    input.ledger_entry_ids.dedup();
    if input.ledger_entry_ids.is_empty() {
        return Err(MarketplacePayoutError::Validation(
            "payout ledger entry set must not be empty".to_string(),
        ));
    }
    input.destination_reference = normalize_optional_text(
        input.destination_reference.take(),
        191,
        "destination_reference",
    )?;
    input.metadata = normalize_metadata(input.metadata)?;
    Ok(input)
}

fn validate_ledger_entry(
    tenant_id: Uuid,
    input: &ScheduleMarketplacePayoutInput,
    entry: &MarketplaceLedgerEntryResponse,
) -> MarketplacePayoutResult<()> {
    if entry.tenant_id != tenant_id {
        return Err(MarketplacePayoutError::Validation(format!(
            "ledger entry {} does not match payout tenant scope",
            entry.id
        )));
    }
    if entry.seller_id != Some(input.seller_id)
        || entry.account_code != MarketplaceLedgerAccountCode::SellerPayable
        || entry.direction != MarketplaceLedgerEntryDirection::Credit
    {
        return Err(MarketplacePayoutError::Validation(format!(
            "ledger entry {} is not a seller payable credit for seller {}",
            entry.id, input.seller_id
        )));
    }
    if normalize_currency(entry.currency_code.clone())? != input.currency_code {
        return Err(MarketplacePayoutError::Validation(format!(
            "ledger entry {} currency does not match payout currency",
            entry.id
        )));
    }
    if entry.amount <= 0 {
        return Err(MarketplacePayoutError::Validation(format!(
            "ledger entry {} amount must be greater than zero",
            entry.id
        )));
    }
    Ok(())
}

fn normalize_currency(value: String) -> MarketplacePayoutResult<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(MarketplacePayoutError::Validation(
            "currency_code must contain exactly three ASCII letters".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_optional_text(
    value: Option<String>,
    max_bytes: usize,
    field: &str,
) -> MarketplacePayoutResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > max_bytes {
        return Err(MarketplacePayoutError::Validation(format!(
            "{field} must not exceed {max_bytes} bytes"
        )));
    }
    Ok(Some(value.to_string()))
}

fn normalize_metadata(value: serde_json::Value) -> MarketplacePayoutResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplacePayoutError::Validation(
            "metadata must be a JSON object".to_string(),
        )),
    }
}

fn map_payout(
    model: payout::Model,
    items: Vec<MarketplacePayoutItemResponse>,
) -> MarketplacePayoutResult<MarketplacePayoutResponse> {
    let status = MarketplacePayoutStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplacePayoutError::Validation(format!(
            "unknown marketplace payout status `{}`",
            model.status
        ))
    })?;
    let item_total = items.iter().try_fold(0_i64, |total, item| {
        total.checked_add(item.amount).ok_or_else(|| {
            MarketplacePayoutError::Validation("payout item total overflow".to_string())
        })
    })?;
    if item_total != model.total_amount {
        return Err(MarketplacePayoutError::Validation(format!(
            "payout {} total does not match its items",
            model.id
        )));
    }
    Ok(MarketplacePayoutResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        seller_id: model.seller_id,
        currency_code: model.currency_code,
        total_amount: model.total_amount,
        status,
        scheduled_for: model.scheduled_for,
        destination_reference: model.destination_reference,
        external_reference: model.external_reference,
        failure_code: model.failure_code,
        metadata: model.metadata,
        created_at: model.created_at,
        updated_at: model.updated_at,
        paid_at: model.paid_at,
        items,
    })
}

fn map_item(model: item::Model) -> MarketplacePayoutItemResponse {
    MarketplacePayoutItemResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        payout_id: model.payout_id,
        ledger_entry_id: model.ledger_entry_id,
        amount: model.amount,
        created_at: model.created_at,
    }
}

fn map_ledger_port_error(error: rustok_api::PortError) -> MarketplacePayoutError {
    MarketplacePayoutError::LedgerBoundary {
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
