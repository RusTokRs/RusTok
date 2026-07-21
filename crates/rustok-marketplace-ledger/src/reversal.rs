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
    MarketplaceLedgerEntryResponse, MarketplaceLedgerReversalEntryResponse,
    MarketplaceLedgerReversalKind, MarketplaceLedgerReversalLineInput,
    MarketplaceLedgerReversalResponse, MarketplaceLedgerTransactionResponse,
    MarketplaceLedgerTransactionStatus, MarketplaceSellerBalanceBucket,
    PostMarketplaceLedgerReversalInput, MAX_LEDGER_REVERSAL_LINES,
};
use crate::entities::{entry, reversal, reversal_line, transaction};
use crate::error::{MarketplaceLedgerError, MarketplaceLedgerResult};
use crate::receipts::{
    admit_command_receipt, command_request_hash, complete_receipt, normalize_idempotency_key,
    replay_command_receipt, replay_existing_command, rollback_receipt, LedgerReceiptAdmission,
    NewLedgerReceipt,
};
use crate::MarketplaceLedgerService;

const COMMAND_KIND: &str = "post_financial_reversal";
const ORIGINAL_SOURCE_KIND: &str = "commission_assessment_batch";

impl MarketplaceLedgerService {
    pub async fn post_reversal_with_receipt(
        &self,
        _context: PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: PostMarketplaceLedgerReversalInput,
    ) -> MarketplaceLedgerResult<MarketplaceLedgerReversalResponse> {
        validate_input(&input)?;
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash(COMMAND_KIND, actor_id, &input)?;
        if let Some(response) = replay_existing_command::<MarketplaceLedgerReversalResponse>(
            self.database(),
            tenant_id,
            key.as_str(),
            COMMAND_KIND,
            hash.as_str(),
        )
        .await?
        {
            self.rebuild_seller_balances_for_transaction(tenant_id, &response.transaction)
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
        self.rebuild_seller_balances_for_transaction(tenant_id, &response.transaction)
            .await?;
        Ok(response)
    }
}

async fn post_in_transaction(
    receipt: &NewLedgerReceipt,
    tenant_id: Uuid,
    input: PostMarketplaceLedgerReversalInput,
) -> MarketplaceLedgerResult<MarketplaceLedgerReversalResponse> {
    let currency_code = normalize_currency(input.currency_code.clone())?;
    let original_transaction = transaction::Entity::find()
        .filter(transaction::Column::TenantId.eq(tenant_id))
        .filter(transaction::Column::SourceKind.eq(ORIGINAL_SOURCE_KIND))
        .filter(transaction::Column::SourceId.eq(input.order_id))
        .one(&receipt.transaction)
        .await?
        .ok_or(MarketplaceLedgerError::TransactionNotFound(input.order_id))?;
    if original_transaction.order_id != input.order_id
        || original_transaction.currency_code != currency_code
    {
        return Err(MarketplaceLedgerError::Validation(
            "reversal order or currency does not match the original ledger transaction".to_string(),
        ));
    }
    if input.reversed_at < original_transaction.posted_at {
        return Err(MarketplaceLedgerError::Validation(
            "reversed_at must not be earlier than the original posting".to_string(),
        ));
    }
    if reversal::Entity::find()
        .filter(reversal::Column::TenantId.eq(tenant_id))
        .filter(reversal::Column::ReversalKind.eq(input.kind.as_str()))
        .filter(reversal::Column::SourceId.eq(input.source_id))
        .one(&receipt.transaction)
        .await?
        .is_some()
    {
        return Err(MarketplaceLedgerError::ReversalAlreadyPosted(input.source_id));
    }

    let mut query = entry::Entity::find()
        .filter(entry::Column::TenantId.eq(tenant_id))
        .filter(entry::Column::TransactionId.eq(original_transaction.id))
        .order_by_asc(entry::Column::Id);
    if receipt.transaction.get_database_backend() != DatabaseBackend::Sqlite {
        query = query.lock_exclusive();
    }
    let originals = index_original_entries(query.all(&receipt.transaction).await?)?;
    let total_amount = reversal_total(&input.lines)?;
    let transaction_id = generate_id();
    let reversal_id = generate_id();
    let created_at = Utc::now().fixed_offset();
    let transaction_metadata = reversal_metadata(&input, original_transaction.id)?;

    let transaction_model = transaction::ActiveModel {
        id: Set(transaction_id),
        tenant_id: Set(tenant_id),
        source_kind: Set(input.kind.source_kind().to_string()),
        source_id: Set(input.source_id),
        order_id: Set(input.order_id),
        currency_code: Set(currency_code.clone()),
        debit_total_amount: Set(total_amount),
        credit_total_amount: Set(total_amount),
        status: Set(MarketplaceLedgerTransactionStatus::Posted.as_str().to_string()),
        posted_at: Set(input.reversed_at.clone()),
        metadata: Set(transaction_metadata),
        created_at: Set(created_at.clone()),
    }
    .insert(&receipt.transaction)
    .await
    .map_err(|error| map_reversal_insert_error(error, input.source_id))?;
    let reversal_model = reversal::ActiveModel {
        id: Set(reversal_id),
        tenant_id: Set(tenant_id),
        transaction_id: Set(transaction_id),
        reversed_transaction_id: Set(original_transaction.id),
        reversal_kind: Set(input.kind.as_str().to_string()),
        source_id: Set(input.source_id),
        order_id: Set(input.order_id),
        currency_code: Set(currency_code.clone()),
        total_amount: Set(total_amount),
        reversed_at: Set(input.reversed_at.clone()),
        metadata: Set(input.metadata.clone()),
        created_at: Set(created_at.clone()),
    }
    .insert(&receipt.transaction)
    .await
    .map_err(|error| map_reversal_insert_error(error, input.source_id))?;

    let mut links = Vec::with_capacity(input.lines.len() * 3);
    let mut entries = Vec::with_capacity(input.lines.len() * 3);
    for line in &input.lines {
        let original = originals.get(&line.assessment_id).ok_or_else(|| {
            MarketplaceLedgerError::Validation(format!(
                "commission assessment {} has no original ledger entries",
                line.assessment_id
            ))
        })?;
        original.validate(line, input.order_id, currency_code.as_str())?;
        let clearing_amount = line
            .commission_amount
            .checked_add(line.seller_amount)
            .ok_or_else(|| MarketplaceLedgerError::Validation("reversal line total overflow".to_string()))?;
        ensure_remaining(receipt, tenant_id, original.clearing()?, clearing_amount).await?;
        ensure_remaining(receipt, tenant_id, original.commission()?, line.commission_amount).await?;
        ensure_remaining(receipt, tenant_id, original.seller()?, line.seller_amount).await?;

        if line.commission_amount > 0 {
            push_entry(
                &mut entries,
                &mut links,
                insert_entry(
                    receipt,
                    tenant_id,
                    reversal_id,
                    transaction_id,
                    input.kind,
                    input.source_id,
                    line,
                    original.commission()?,
                    None,
                    MarketplaceLedgerAccountCode::PlatformCommissionRevenue,
                    MarketplaceLedgerEntryDirection::Debit,
                    line.commission_amount,
                    None,
                    currency_code.as_str(),
                    created_at.clone(),
                )
                .await?,
            );
        }
        if line.seller_amount > 0 {
            push_entry(
                &mut entries,
                &mut links,
                insert_entry(
                    receipt,
                    tenant_id,
                    reversal_id,
                    transaction_id,
                    input.kind,
                    input.source_id,
                    line,
                    original.seller()?,
                    Some(line.seller_id),
                    MarketplaceLedgerAccountCode::SellerPayable,
                    MarketplaceLedgerEntryDirection::Debit,
                    line.seller_amount,
                    Some(line.seller_balance_bucket),
                    currency_code.as_str(),
                    created_at.clone(),
                )
                .await?,
            );
        }
        push_entry(
            &mut entries,
            &mut links,
            insert_entry(
                receipt,
                tenant_id,
                reversal_id,
                transaction_id,
                input.kind,
                input.source_id,
                line,
                original.clearing()?,
                None,
                MarketplaceLedgerAccountCode::MarketplaceClearing,
                MarketplaceLedgerEntryDirection::Credit,
                clearing_amount,
                None,
                currency_code.as_str(),
                created_at.clone(),
            )
            .await?,
        );
    }
    sort_entries(&mut entries);
    links.sort_by_key(|link| entry_sort_key(&link.entry));
    let transaction = map_transaction(transaction_model, entries)?;
    Ok(MarketplaceLedgerReversalResponse {
        id: reversal_model.id,
        tenant_id: reversal_model.tenant_id,
        transaction_id: reversal_model.transaction_id,
        kind: input.kind,
        source_id: reversal_model.source_id,
        order_id: reversal_model.order_id,
        currency_code: reversal_model.currency_code,
        total_amount: reversal_model.total_amount,
        reversed_transaction_id: reversal_model.reversed_transaction_id,
        reversed_at: reversal_model.reversed_at,
        metadata: reversal_model.metadata,
        created_at: reversal_model.created_at,
        transaction,
        entries: links,
    })
}

fn push_entry(
    entries: &mut Vec<MarketplaceLedgerEntryResponse>,
    links: &mut Vec<MarketplaceLedgerReversalEntryResponse>,
    link: MarketplaceLedgerReversalEntryResponse,
) {
    entries.push(link.entry.clone());
    links.push(link);
}

#[allow(clippy::too_many_arguments)]
async fn insert_entry(
    receipt: &NewLedgerReceipt,
    tenant_id: Uuid,
    reversal_id: Uuid,
    transaction_id: Uuid,
    kind: MarketplaceLedgerReversalKind,
    source_id: Uuid,
    line: &MarketplaceLedgerReversalLineInput,
    reversed_entry: &entry::Model,
    seller_id: Option<Uuid>,
    account_code: MarketplaceLedgerAccountCode,
    direction: MarketplaceLedgerEntryDirection,
    amount: i64,
    bucket: Option<MarketplaceSellerBalanceBucket>,
    currency_code: &str,
    created_at: chrono::DateTime<chrono::FixedOffset>,
) -> MarketplaceLedgerResult<MarketplaceLedgerReversalEntryResponse> {
    if amount <= 0 {
        return Err(MarketplaceLedgerError::Validation(
            "reversal entry amount must be positive".to_string(),
        ));
    }
    let entry_id = generate_id();
    let model = entry::ActiveModel {
        id: Set(entry_id),
        tenant_id: Set(tenant_id),
        transaction_id: Set(transaction_id),
        order_id: Set(reversed_entry.order_id),
        assessment_id: Set(line.assessment_id),
        allocation_id: Set(line.allocation_id),
        order_line_item_id: Set(line.order_line_item_id),
        seller_id: Set(seller_id),
        account_code: Set(account_code.as_str().to_string()),
        direction: Set(direction.as_str().to_string()),
        currency_code: Set(currency_code.to_string()),
        amount: Set(amount),
        metadata: Set(serde_json::json!({
            "reversal_id": reversal_id,
            "reversal_kind": kind.as_str(),
            "reversal_source_id": source_id,
        })),
        created_at: Set(created_at.clone()),
    }
    .insert(&receipt.transaction)
    .await?;
    reversal_line::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        reversal_id: Set(reversal_id),
        entry_id: Set(entry_id),
        reversed_entry_id: Set(reversed_entry.id),
        seller_id: Set(seller_id),
        assessment_id: Set(line.assessment_id),
        allocation_id: Set(line.allocation_id),
        order_line_item_id: Set(line.order_line_item_id),
        account_code: Set(account_code.as_str().to_string()),
        direction: Set(direction.as_str().to_string()),
        seller_balance_bucket: Set(bucket.map(|value| value.as_str().to_string())),
        amount: Set(amount),
        created_at: Set(created_at),
    }
    .insert(&receipt.transaction)
    .await?;
    Ok(MarketplaceLedgerReversalEntryResponse {
        entry: map_entry(model)?,
        reversed_entry_id: reversed_entry.id,
        seller_balance_bucket: bucket,
    })
}

async fn ensure_remaining(
    receipt: &NewLedgerReceipt,
    tenant_id: Uuid,
    original: &entry::Model,
    requested: i64,
) -> MarketplaceLedgerResult<()> {
    if requested < 0 {
        return Err(MarketplaceLedgerError::Validation(
            "reversal amount must not be negative".to_string(),
        ));
    }
    if requested == 0 {
        return Ok(());
    }
    let used = reversal_line::Entity::find()
        .filter(reversal_line::Column::TenantId.eq(tenant_id))
        .filter(reversal_line::Column::ReversedEntryId.eq(original.id))
        .all(&receipt.transaction)
        .await?
        .into_iter()
        .try_fold(0_i64, |total, line| {
            total.checked_add(line.amount).ok_or_else(|| {
                MarketplaceLedgerError::Validation("cumulative reversal amount overflow".to_string())
            })
        })?;
    let remaining = original.amount.checked_sub(used).ok_or_else(|| {
        MarketplaceLedgerError::Validation(format!(
            "original ledger entry {} is over-reversed",
            original.id
        ))
    })?;
    if requested > remaining {
        return Err(MarketplaceLedgerError::Validation(format!(
            "reversal amount {requested} exceeds remaining amount {remaining} for ledger entry {}",
            original.id
        )));
    }
    Ok(())
}

#[derive(Default)]
struct OriginalAssessmentEntries {
    clearing: Option<entry::Model>,
    commission: Option<entry::Model>,
    seller: Option<entry::Model>,
}

impl OriginalAssessmentEntries {
    fn insert(
        &mut self,
        account: MarketplaceLedgerAccountCode,
        model: entry::Model,
    ) -> MarketplaceLedgerResult<()> {
        let slot = match account {
            MarketplaceLedgerAccountCode::MarketplaceClearing => &mut self.clearing,
            MarketplaceLedgerAccountCode::PlatformCommissionRevenue => &mut self.commission,
            MarketplaceLedgerAccountCode::SellerPayable => &mut self.seller,
        };
        if slot.replace(model).is_some() {
            return Err(MarketplaceLedgerError::Validation(
                "original ledger transaction contains duplicate assessment accounts".to_string(),
            ));
        }
        Ok(())
    }

    fn clearing(&self) -> MarketplaceLedgerResult<&entry::Model> {
        self.clearing.as_ref().ok_or_else(|| missing_original("marketplace_clearing"))
    }

    fn commission(&self) -> MarketplaceLedgerResult<&entry::Model> {
        self.commission
            .as_ref()
            .ok_or_else(|| missing_original("platform_commission_revenue"))
    }

    fn seller(&self) -> MarketplaceLedgerResult<&entry::Model> {
        self.seller.as_ref().ok_or_else(|| missing_original("seller_payable"))
    }

    fn validate(
        &self,
        line: &MarketplaceLedgerReversalLineInput,
        order_id: Uuid,
        currency_code: &str,
    ) -> MarketplaceLedgerResult<()> {
        for model in [self.clearing()?, self.commission()?, self.seller()?] {
            if model.order_id != order_id
                || model.currency_code != currency_code
                || model.assessment_id != line.assessment_id
                || model.allocation_id != line.allocation_id
                || model.order_line_item_id != line.order_line_item_id
            {
                return Err(MarketplaceLedgerError::Validation(format!(
                    "reversal identity does not match original ledger entry {}",
                    model.id
                )));
            }
        }
        if self.seller()?.seller_id != Some(line.seller_id) {
            return Err(MarketplaceLedgerError::Validation(
                "reversal seller does not match the original seller payable entry".to_string(),
            ));
        }
        Ok(())
    }
}

fn index_original_entries(
    models: Vec<entry::Model>,
) -> MarketplaceLedgerResult<HashMap<Uuid, OriginalAssessmentEntries>> {
    let mut output = HashMap::new();
    for model in models {
        let account = MarketplaceLedgerAccountCode::parse(model.account_code.as_str())
            .ok_or_else(|| {
                MarketplaceLedgerError::Validation(format!(
                    "unknown original ledger account code `{}`",
                    model.account_code
                ))
            })?;
        let expected = match account {
            MarketplaceLedgerAccountCode::MarketplaceClearing => {
                MarketplaceLedgerEntryDirection::Debit
            }
            MarketplaceLedgerAccountCode::PlatformCommissionRevenue
            | MarketplaceLedgerAccountCode::SellerPayable => {
                MarketplaceLedgerEntryDirection::Credit
            }
        };
        if MarketplaceLedgerEntryDirection::parse(model.direction.as_str()) != Some(expected) {
            return Err(MarketplaceLedgerError::Validation(format!(
                "original ledger entry {} has an invalid direction",
                model.id
            )));
        }
        output
            .entry(model.assessment_id)
            .or_insert_with(OriginalAssessmentEntries::default)
            .insert(account, model)?;
    }
    Ok(output)
}

fn validate_input(input: &PostMarketplaceLedgerReversalInput) -> MarketplaceLedgerResult<()> {
    if input.source_id.is_nil() || input.order_id.is_nil() {
        return Err(MarketplaceLedgerError::Validation(
            "reversal source_id and order_id must not be nil".to_string(),
        ));
    }
    normalize_currency(input.currency_code.clone())?;
    if input.lines.is_empty() || input.lines.len() > MAX_LEDGER_REVERSAL_LINES {
        return Err(MarketplaceLedgerError::Validation(format!(
            "reversal requires 1 to {MAX_LEDGER_REVERSAL_LINES} lines"
        )));
    }
    if !input.metadata.is_object() {
        return Err(MarketplaceLedgerError::Validation(
            "reversal metadata must be an object".to_string(),
        ));
    }
    let mut assessments = HashSet::with_capacity(input.lines.len());
    for line in &input.lines {
        if line.assessment_id.is_nil()
            || line.allocation_id.is_nil()
            || line.order_line_item_id.is_nil()
            || line.seller_id.is_nil()
        {
            return Err(MarketplaceLedgerError::Validation(
                "reversal line identities must not be nil".to_string(),
            ));
        }
        if !assessments.insert(line.assessment_id) {
            return Err(MarketplaceLedgerError::Validation(format!(
                "commission assessment {} appears more than once in reversal",
                line.assessment_id
            )));
        }
        if line.commission_amount < 0 || line.seller_amount < 0 {
            return Err(MarketplaceLedgerError::Validation(
                "reversal line amounts must not be negative".to_string(),
            ));
        }
        if line.commission_amount == 0 && line.seller_amount == 0 {
            return Err(MarketplaceLedgerError::Validation(
                "reversal line must reverse a positive amount".to_string(),
            ));
        }
    }
    Ok(())
}

fn reversal_total(lines: &[MarketplaceLedgerReversalLineInput]) -> MarketplaceLedgerResult<i64> {
    let total = lines.iter().try_fold(0_i64, |total, line| {
        total
            .checked_add(line.commission_amount)
            .and_then(|value| value.checked_add(line.seller_amount))
            .ok_or_else(|| MarketplaceLedgerError::Validation("reversal total overflow".to_string()))
    })?;
    if total <= 0 {
        return Err(MarketplaceLedgerError::Validation(
            "reversal total must be positive".to_string(),
        ));
    }
    Ok(total)
}

fn reversal_metadata(
    input: &PostMarketplaceLedgerReversalInput,
    original_transaction_id: Uuid,
) -> MarketplaceLedgerResult<serde_json::Value> {
    let mut metadata = input.metadata.as_object().cloned().ok_or_else(|| {
        MarketplaceLedgerError::Validation("reversal metadata must be an object".to_string())
    })?;
    metadata.insert(
        "ledger_reversal".to_string(),
        serde_json::json!({
            "kind": input.kind.as_str(),
            "source_id": input.source_id,
            "reversed_transaction_id": original_transaction_id,
            "line_count": input.lines.len(),
        }),
    );
    Ok(serde_json::Value::Object(metadata))
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
        .ok_or_else(|| MarketplaceLedgerError::Validation(format!(
            "unknown ledger account code `{}`",
            model.account_code
        )))?;
    let direction = MarketplaceLedgerEntryDirection::parse(model.direction.as_str())
        .ok_or_else(|| MarketplaceLedgerError::Validation(format!(
            "unknown ledger entry direction `{}`",
            model.direction
        )))?;
    if model.amount < 0 {
        return Err(MarketplaceLedgerError::Validation(format!(
            "ledger entry {} has a negative amount",
            model.id
        )));
    }
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
    entries.sort_by_key(entry_sort_key);
}

fn entry_sort_key(
    entry: &MarketplaceLedgerEntryResponse,
) -> (Uuid, Uuid, String, String) {
    (
        entry.order_line_item_id,
        entry.assessment_id,
        entry.account_code.as_str().to_string(),
        entry.direction.as_str().to_string(),
    )
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

fn missing_original(account: &str) -> MarketplaceLedgerError {
    MarketplaceLedgerError::Validation(format!(
        "commission assessment has no original {account} ledger entry"
    ))
}

fn map_reversal_insert_error(error: sea_orm::DbErr, source_id: Uuid) -> MarketplaceLedgerError {
    if is_unique_constraint(&error) {
        MarketplaceLedgerError::ReversalAlreadyPosted(source_id)
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
