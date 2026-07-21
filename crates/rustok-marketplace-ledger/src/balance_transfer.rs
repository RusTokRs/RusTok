use std::collections::{HashMap, HashSet};

use chrono::Utc;
use rustok_api::PortContext;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use crate::dto::{
    MarketplaceLedgerAccountCode, MarketplaceLedgerEntryDirection,
    MarketplaceLedgerEntryResponse, MarketplaceLedgerTransactionResponse,
    MarketplaceLedgerTransactionStatus, MarketplaceSellerBalanceBucket,
    MarketplaceSellerBalanceTransferKind, MarketplaceSellerBalanceTransferLineResponse,
    MarketplaceSellerBalanceTransferResponse, PostMarketplaceSellerBalanceTransferInput,
    RebuildMarketplaceSellerBalanceInput, MAX_LEDGER_BALANCE_TRANSFER_LINES,
};
use crate::entities::{
    balance_transfer, balance_transfer_line, entry, entry_balance_bucket, reversal_line,
    transaction,
};
use crate::error::{MarketplaceLedgerError, MarketplaceLedgerResult};
use crate::receipts::{
    admit_command_receipt, command_request_hash, complete_receipt, normalize_idempotency_key,
    replay_command_receipt, replay_existing_command, rollback_receipt, LedgerReceiptAdmission,
    NewLedgerReceipt,
};
use crate::MarketplaceLedgerService;

const COMMAND_KIND: &str = "post_seller_balance_transfer";

impl MarketplaceLedgerService {
    pub async fn post_balance_transfer_with_receipt(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: PostMarketplaceSellerBalanceTransferInput,
    ) -> MarketplaceLedgerResult<MarketplaceSellerBalanceTransferResponse> {
        validate_input(&input)?;
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash(COMMAND_KIND, actor_id, &input)?;
        if let Some(response) =
            replay_existing_command::<MarketplaceSellerBalanceTransferResponse>(
                self.database(),
                tenant_id,
                key.as_str(),
                COMMAND_KIND,
                hash.as_str(),
            )
            .await?
        {
            self.rebuild_seller_balance_projection(
                tenant_id,
                RebuildMarketplaceSellerBalanceInput {
                    seller_id: response.seller_id,
                    currency_code: response.currency_code.clone(),
                },
            )
            .await?;
            return Ok(response);
        }

        let response = match admit_command_receipt(
            self.database(),
            tenant_id,
            actor_id,
            key,
            COMMAND_KIND,
            hash.as_str(),
        )
        .await?
        {
            LedgerReceiptAdmission::Replay(receipt) => {
                replay_command_receipt(receipt, COMMAND_KIND, hash.as_str())?
            }
            LedgerReceiptAdmission::New(receipt) => {
                match post_in_transaction(&receipt, tenant_id, input).await {
                    Ok(response) => complete_receipt(receipt, &response).await?,
                    Err(error) => return rollback_receipt(receipt, error).await,
                }
            }
        };
        self.rebuild_seller_balance_projection(
            tenant_id,
            RebuildMarketplaceSellerBalanceInput {
                seller_id: response.seller_id,
                currency_code: response.currency_code.clone(),
            },
        )
        .await?;
        Ok(response)
    }
}

async fn post_in_transaction(
    receipt: &NewLedgerReceipt,
    tenant_id: Uuid,
    input: PostMarketplaceSellerBalanceTransferInput,
) -> MarketplaceLedgerResult<MarketplaceSellerBalanceTransferResponse> {
    let currency_code = normalize_currency(input.currency_code.clone())?;
    if balance_transfer::Entity::find()
        .filter(balance_transfer::Column::TenantId.eq(tenant_id))
        .filter(balance_transfer::Column::TransferKind.eq(input.kind.as_str()))
        .filter(balance_transfer::Column::SourceId.eq(input.source_id))
        .one(&receipt.transaction)
        .await?
        .is_some()
    {
        return Err(MarketplaceLedgerError::BalanceTransferAlreadyPosted(
            input.source_id,
        ));
    }

    if receipt.transaction.get_database_backend() != DatabaseBackend::Sqlite {
        entry::Entity::find()
            .filter(entry::Column::TenantId.eq(tenant_id))
            .filter(entry::Column::SellerId.eq(input.seller_id))
            .filter(
                entry::Column::AccountCode
                    .eq(MarketplaceLedgerAccountCode::SellerPayable.as_str()),
            )
            .filter(entry::Column::CurrencyCode.eq(currency_code.clone()))
            .order_by_asc(entry::Column::CreatedAt)
            .order_by_asc(entry::Column::Id)
            .lock_exclusive()
            .all(&receipt.transaction)
            .await?;
    }
    // The locking statement can wait on a concurrent transfer while retaining an older
    // statement snapshot. Reread after the locks are acquired so newly committed transfer
    // entries participate in the immutable capacity calculation.
    let seller_entries = entry::Entity::find()
        .filter(entry::Column::TenantId.eq(tenant_id))
        .filter(entry::Column::SellerId.eq(input.seller_id))
        .filter(
            entry::Column::AccountCode
                .eq(MarketplaceLedgerAccountCode::SellerPayable.as_str()),
        )
        .filter(entry::Column::CurrencyCode.eq(currency_code.clone()))
        .order_by_asc(entry::Column::CreatedAt)
        .order_by_asc(entry::Column::Id)
        .all(&receipt.transaction)
        .await?;
    if seller_entries.is_empty() {
        return Err(MarketplaceLedgerError::SellerBalanceNotFound {
            seller_id: input.seller_id,
            currency_code,
        });
    }

    let entry_ids = seller_entries.iter().map(|model| model.id).collect::<Vec<_>>();
    let explicit_buckets = entry_balance_bucket::Entity::find()
        .filter(entry_balance_bucket::Column::TenantId.eq(tenant_id))
        .filter(entry_balance_bucket::Column::EntryId.is_in(entry_ids.clone()))
        .all(&receipt.transaction)
        .await?
        .into_iter()
        .map(|model| (model.entry_id, model.balance_bucket))
        .collect::<HashMap<_, _>>();
    let reversal_buckets = reversal_line::Entity::find()
        .filter(reversal_line::Column::TenantId.eq(tenant_id))
        .filter(reversal_line::Column::EntryId.is_in(entry_ids))
        .all(&receipt.transaction)
        .await?
        .into_iter()
        .filter_map(|model| {
            model
                .seller_balance_bucket
                .map(|bucket| (model.entry_id, bucket))
        })
        .collect::<HashMap<_, _>>();

    let indexed = seller_entries
        .iter()
        .map(|model| (model.id, model))
        .collect::<HashMap<_, _>>();
    let totals = calculate_bucket_totals(&seller_entries, &explicit_buckets, &reversal_buckets)?;
    let (from_bucket, to_bucket) = input.kind.buckets();
    let total_amount = transfer_total(&input)?;
    let available = totals.amount(from_bucket);
    if total_amount > available {
        return Err(MarketplaceLedgerError::Validation(format!(
            "seller balance transfer amount {total_amount} exceeds {} bucket balance {available}",
            from_bucket.as_str()
        )));
    }

    let mut reference_ids = HashSet::with_capacity(input.lines.len());
    let mut assessment_ids = HashSet::with_capacity(input.lines.len());
    let mut references = Vec::with_capacity(input.lines.len());
    let mut order_id = None;
    for line in &input.lines {
        if !reference_ids.insert(line.reference_entry_id) {
            return Err(MarketplaceLedgerError::Validation(format!(
                "reference entry {} appears more than once in seller balance transfer",
                line.reference_entry_id
            )));
        }
        let reference = indexed.get(&line.reference_entry_id).copied().ok_or_else(|| {
            MarketplaceLedgerError::Validation(format!(
                "reference seller payable entry {} was not found",
                line.reference_entry_id
            ))
        })?;
        if reference.amount <= 0 {
            return Err(MarketplaceLedgerError::Validation(format!(
                "reference seller payable entry {} must have a positive amount",
                reference.id
            )));
        }
        if MarketplaceLedgerEntryDirection::parse(reference.direction.as_str())
            != Some(MarketplaceLedgerEntryDirection::Credit)
        {
            return Err(MarketplaceLedgerError::Validation(format!(
                "reference seller payable entry {} must be a credit",
                reference.id
            )));
        }
        let bucket = entry_bucket(reference.id, &explicit_buckets, &reversal_buckets)?;
        if bucket != from_bucket {
            return Err(MarketplaceLedgerError::Validation(format!(
                "reference entry {} belongs to {} bucket, expected {}",
                reference.id,
                bucket.as_str(),
                from_bucket.as_str()
            )));
        }
        if line.amount > reference.amount {
            return Err(MarketplaceLedgerError::Validation(format!(
                "transfer line amount {} exceeds reference entry {} amount {}",
                line.amount, reference.id, reference.amount
            )));
        }
        if input.transferred_at < reference.created_at {
            return Err(MarketplaceLedgerError::Validation(format!(
                "transferred_at must not be earlier than reference entry {}",
                reference.id
            )));
        }
        if !assessment_ids.insert(reference.assessment_id) {
            return Err(MarketplaceLedgerError::Validation(format!(
                "commission assessment {} appears more than once in seller balance transfer",
                reference.assessment_id
            )));
        }
        match order_id {
            Some(expected) if expected != reference.order_id => {
                return Err(MarketplaceLedgerError::Validation(
                    "seller balance transfer reference entries must belong to one order"
                        .to_string(),
                ));
            }
            None => order_id = Some(reference.order_id),
            _ => {}
        }
        references.push((line, reference));
    }
    let order_id = order_id.ok_or_else(|| {
        MarketplaceLedgerError::Validation(
            "seller balance transfer could not derive an order scope".to_string(),
        )
    })?;

    let transaction_id = generate_id();
    let transfer_id = generate_id();
    let created_at = Utc::now().fixed_offset();
    let source_kind = input.kind.source_kind();
    let transaction_model = transaction::ActiveModel {
        id: Set(transaction_id),
        tenant_id: Set(tenant_id),
        source_kind: Set(source_kind.to_string()),
        source_id: Set(input.source_id),
        order_id: Set(order_id),
        currency_code: Set(currency_code.clone()),
        debit_total_amount: Set(total_amount),
        credit_total_amount: Set(total_amount),
        status: Set(MarketplaceLedgerTransactionStatus::Posted.as_str().to_string()),
        posted_at: Set(input.transferred_at.clone()),
        metadata: Set(serde_json::json!({
            "transfer_id": transfer_id,
            "transfer_kind": input.kind.as_str(),
            "transfer_source_id": input.source_id,
            "seller_id": input.seller_id,
            "from_bucket": from_bucket.as_str(),
            "to_bucket": to_bucket.as_str(),
            "line_count": input.lines.len(),
        })),
        created_at: Set(created_at.clone()),
    }
    .insert(&receipt.transaction)
    .await
    .map_err(|error| map_transfer_insert_error(error, input.source_id))?;
    let transfer_model = balance_transfer::ActiveModel {
        id: Set(transfer_id),
        tenant_id: Set(tenant_id),
        transaction_id: Set(transaction_id),
        transfer_kind: Set(input.kind.as_str().to_string()),
        source_id: Set(input.source_id),
        seller_id: Set(input.seller_id),
        currency_code: Set(currency_code.clone()),
        from_bucket: Set(from_bucket.as_str().to_string()),
        to_bucket: Set(to_bucket.as_str().to_string()),
        total_amount: Set(total_amount),
        transferred_at: Set(input.transferred_at.clone()),
        metadata: Set(input.metadata.clone()),
        created_at: Set(created_at.clone()),
    }
    .insert(&receipt.transaction)
    .await
    .map_err(|error| map_transfer_insert_error(error, input.source_id))?;

    let mut transaction_entries = Vec::with_capacity(references.len() * 2);
    let mut response_lines = Vec::with_capacity(references.len());
    for (line, reference) in references {
        let debit = insert_transfer_entry(
            receipt,
            tenant_id,
            transfer_id,
            transaction_id,
            input.kind,
            input.source_id,
            reference,
            from_bucket,
            MarketplaceLedgerEntryDirection::Debit,
            line.amount,
            currency_code.as_str(),
            created_at.clone(),
        )
        .await?;
        let credit = insert_transfer_entry(
            receipt,
            tenant_id,
            transfer_id,
            transaction_id,
            input.kind,
            input.source_id,
            reference,
            to_bucket,
            MarketplaceLedgerEntryDirection::Credit,
            line.amount,
            currency_code.as_str(),
            created_at.clone(),
        )
        .await?;
        balance_transfer_line::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            transfer_id: Set(transfer_id),
            reference_entry_id: Set(reference.id),
            debit_entry_id: Set(debit.id),
            credit_entry_id: Set(credit.id),
            amount: Set(line.amount),
            created_at: Set(created_at.clone()),
        }
        .insert(&receipt.transaction)
        .await?;
        transaction_entries.push(debit.clone());
        transaction_entries.push(credit.clone());
        response_lines.push(MarketplaceSellerBalanceTransferLineResponse {
            reference_entry_id: reference.id,
            amount: line.amount,
            from_bucket,
            to_bucket,
            debit_entry: debit,
            credit_entry: credit,
        });
    }
    sort_entries(&mut transaction_entries);
    response_lines.sort_by_key(|line| line.reference_entry_id);
    let transaction = map_transaction(transaction_model, transaction_entries)?;
    Ok(MarketplaceSellerBalanceTransferResponse {
        id: transfer_model.id,
        tenant_id: transfer_model.tenant_id,
        transaction_id: transfer_model.transaction_id,
        kind: input.kind,
        source_id: transfer_model.source_id,
        seller_id: transfer_model.seller_id,
        currency_code: transfer_model.currency_code,
        from_bucket,
        to_bucket,
        total_amount: transfer_model.total_amount,
        transferred_at: transfer_model.transferred_at,
        metadata: transfer_model.metadata,
        created_at: transfer_model.created_at,
        transaction,
        lines: response_lines,
    })
}

#[allow(clippy::too_many_arguments)]
async fn insert_transfer_entry(
    receipt: &NewLedgerReceipt,
    tenant_id: Uuid,
    transfer_id: Uuid,
    transaction_id: Uuid,
    kind: MarketplaceSellerBalanceTransferKind,
    source_id: Uuid,
    reference: &entry::Model,
    bucket: MarketplaceSellerBalanceBucket,
    direction: MarketplaceLedgerEntryDirection,
    amount: i64,
    currency_code: &str,
    created_at: chrono::DateTime<chrono::FixedOffset>,
) -> MarketplaceLedgerResult<MarketplaceLedgerEntryResponse> {
    let entry_id = generate_id();
    let model = entry::ActiveModel {
        id: Set(entry_id),
        tenant_id: Set(tenant_id),
        transaction_id: Set(transaction_id),
        order_id: Set(reference.order_id),
        assessment_id: Set(reference.assessment_id),
        allocation_id: Set(reference.allocation_id),
        order_line_item_id: Set(reference.order_line_item_id),
        seller_id: Set(reference.seller_id),
        account_code: Set(MarketplaceLedgerAccountCode::SellerPayable
            .as_str()
            .to_string()),
        direction: Set(direction.as_str().to_string()),
        currency_code: Set(currency_code.to_string()),
        amount: Set(amount),
        metadata: Set(serde_json::json!({
            "transfer_id": transfer_id,
            "transfer_kind": kind.as_str(),
            "transfer_source_id": source_id,
            "reference_entry_id": reference.id,
            "balance_bucket": bucket.as_str(),
        })),
        created_at: Set(created_at.clone()),
    }
    .insert(&receipt.transaction)
    .await?;
    entry_balance_bucket::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        entry_id: Set(entry_id),
        seller_id: Set(reference.seller_id.ok_or_else(|| {
            MarketplaceLedgerError::Validation(
                "seller payable reference entry requires seller identity".to_string(),
            )
        })?),
        balance_bucket: Set(bucket.as_str().to_string()),
        source_kind: Set(kind.source_kind().to_string()),
        source_id: Set(source_id),
        created_at: Set(created_at),
    }
    .insert(&receipt.transaction)
    .await?;
    map_entry(model)
}

fn validate_input(
    input: &PostMarketplaceSellerBalanceTransferInput,
) -> MarketplaceLedgerResult<()> {
    if input.source_id.is_nil() || input.seller_id.is_nil() {
        return Err(MarketplaceLedgerError::Validation(
            "seller balance transfer source_id and seller_id must not be nil".to_string(),
        ));
    }
    normalize_currency(input.currency_code.clone())?;
    if input.lines.is_empty() || input.lines.len() > MAX_LEDGER_BALANCE_TRANSFER_LINES {
        return Err(MarketplaceLedgerError::Validation(format!(
            "seller balance transfer requires 1 to {MAX_LEDGER_BALANCE_TRANSFER_LINES} lines"
        )));
    }
    if !input.metadata.is_object() {
        return Err(MarketplaceLedgerError::Validation(
            "seller balance transfer metadata must be an object".to_string(),
        ));
    }
    for line in &input.lines {
        if line.reference_entry_id.is_nil() || line.amount <= 0 {
            return Err(MarketplaceLedgerError::Validation(
                "seller balance transfer line requires a reference entry and positive amount"
                    .to_string(),
            ));
        }
    }
    Ok(())
}

fn transfer_total(
    input: &PostMarketplaceSellerBalanceTransferInput,
) -> MarketplaceLedgerResult<i64> {
    let total = input.lines.iter().try_fold(0_i64, |total, line| {
        total.checked_add(line.amount).ok_or_else(|| {
            MarketplaceLedgerError::Validation(
                "seller balance transfer total overflow".to_string(),
            )
        })
    })?;
    if total <= 0 {
        return Err(MarketplaceLedgerError::Validation(
            "seller balance transfer total must be positive".to_string(),
        ));
    }
    Ok(total)
}

#[derive(Default)]
struct BucketTotals {
    pending: i64,
    available: i64,
    reserved: i64,
    paid: i64,
}

impl BucketTotals {
    fn apply(
        &mut self,
        bucket: MarketplaceSellerBalanceBucket,
        direction: MarketplaceLedgerEntryDirection,
        amount: i64,
    ) -> MarketplaceLedgerResult<()> {
        if amount < 0 {
            return Err(MarketplaceLedgerError::Validation(
                "seller balance entry amount must not be negative".to_string(),
            ));
        }
        let target = match bucket {
            MarketplaceSellerBalanceBucket::Pending => &mut self.pending,
            MarketplaceSellerBalanceBucket::Available => &mut self.available,
            MarketplaceSellerBalanceBucket::Reserved => &mut self.reserved,
            MarketplaceSellerBalanceBucket::Paid => &mut self.paid,
        };
        *target = match direction {
            MarketplaceLedgerEntryDirection::Credit => target.checked_add(amount),
            MarketplaceLedgerEntryDirection::Debit => target.checked_sub(amount),
        }
        .ok_or_else(|| {
            MarketplaceLedgerError::Validation(
                "seller balance bucket overflow during transfer admission".to_string(),
            )
        })?;
        Ok(())
    }

    fn amount(&self, bucket: MarketplaceSellerBalanceBucket) -> i64 {
        match bucket {
            MarketplaceSellerBalanceBucket::Pending => self.pending,
            MarketplaceSellerBalanceBucket::Available => self.available,
            MarketplaceSellerBalanceBucket::Reserved => self.reserved,
            MarketplaceSellerBalanceBucket::Paid => self.paid,
        }
    }
}

fn calculate_bucket_totals(
    entries: &[entry::Model],
    explicit_buckets: &HashMap<Uuid, String>,
    reversal_buckets: &HashMap<Uuid, String>,
) -> MarketplaceLedgerResult<BucketTotals> {
    let mut totals = BucketTotals::default();
    for model in entries {
        let direction = MarketplaceLedgerEntryDirection::parse(model.direction.as_str())
            .ok_or_else(|| {
                MarketplaceLedgerError::Validation(format!(
                    "unknown ledger entry direction `{}`",
                    model.direction
                ))
            })?;
        totals.apply(
            entry_bucket(model.id, explicit_buckets, reversal_buckets)?,
            direction,
            model.amount,
        )?;
    }
    Ok(totals)
}

fn entry_bucket(
    entry_id: Uuid,
    explicit_buckets: &HashMap<Uuid, String>,
    reversal_buckets: &HashMap<Uuid, String>,
) -> MarketplaceLedgerResult<MarketplaceSellerBalanceBucket> {
    explicit_buckets
        .get(&entry_id)
        .or_else(|| reversal_buckets.get(&entry_id))
        .map(|value| {
            MarketplaceSellerBalanceBucket::parse(value.as_str()).ok_or_else(|| {
                MarketplaceLedgerError::Validation(format!(
                    "unknown seller balance bucket `{value}`"
                ))
            })
        })
        .transpose()
        .map(|value| value.unwrap_or(MarketplaceSellerBalanceBucket::Pending))
}

fn map_transaction(
    model: transaction::Model,
    entries: Vec<MarketplaceLedgerEntryResponse>,
) -> MarketplaceLedgerResult<MarketplaceLedgerTransactionResponse> {
    let status = MarketplaceLedgerTransactionStatus::parse(model.status.as_str()).ok_or_else(|| {
        MarketplaceLedgerError::Validation(format!(
            "unknown ledger transaction status `{}`",
            model.status
        ))
    })?;
    if model.debit_total_amount != model.credit_total_amount {
        return Err(MarketplaceLedgerError::Validation(format!(
            "ledger transaction {} is not balanced",
            model.id
        )));
    }
    Ok(MarketplaceLedgerTransactionResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        source_kind: model.source_kind,
        source_id: model.source_id,
        order_id: model.order_id,
        currency_code: model.currency_code,
        debit_total_amount: model.debit_total_amount,
        credit_total_amount: model.credit_total_amount,
        status,
        posted_at: model.posted_at,
        metadata: model.metadata,
        created_at: model.created_at,
        entries,
    })
}

fn map_entry(model: entry::Model) -> MarketplaceLedgerResult<MarketplaceLedgerEntryResponse> {
    let account_code = MarketplaceLedgerAccountCode::parse(model.account_code.as_str())
        .ok_or_else(|| {
            MarketplaceLedgerError::Validation(format!(
                "unknown ledger account code `{}`",
                model.account_code
            ))
        })?;
    let direction = MarketplaceLedgerEntryDirection::parse(model.direction.as_str()).ok_or_else(
        || {
            MarketplaceLedgerError::Validation(format!(
                "unknown ledger entry direction `{}`",
                model.direction
            ))
        },
    )?;
    Ok(MarketplaceLedgerEntryResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        transaction_id: model.transaction_id,
        order_id: model.order_id,
        assessment_id: model.assessment_id,
        allocation_id: model.allocation_id,
        order_line_item_id: model.order_line_item_id,
        seller_id: model.seller_id,
        account_code,
        direction,
        currency_code: model.currency_code,
        amount: model.amount,
        metadata: model.metadata,
        created_at: model.created_at,
    })
}

fn sort_entries(entries: &mut [MarketplaceLedgerEntryResponse]) {
    entries.sort_by_key(|entry| {
        (
            entry.order_line_item_id,
            entry.assessment_id,
            entry.direction.as_str().to_string(),
            entry.id,
        )
    });
}

fn normalize_currency(value: String) -> MarketplaceLedgerResult<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(MarketplaceLedgerError::Validation(
            "currency_code must contain exactly three ASCII letters".to_string(),
        ));
    }
    Ok(value)
}

fn map_transfer_insert_error(error: sea_orm::DbErr, source_id: Uuid) -> MarketplaceLedgerError {
    if is_unique_constraint(&error) {
        MarketplaceLedgerError::BalanceTransferAlreadyPosted(source_id)
    } else {
        error.into()
    }
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
