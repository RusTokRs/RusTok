use std::{collections::HashSet, sync::Arc};

use chrono::Utc;
use rustok_api::PortContext;
use rustok_core::generate_id;
use rustok_marketplace_commission::{
    ListMarketplaceCommissionAssessmentsByOrderRequest, MarketplaceCommissionAssessmentResponse,
    MarketplaceCommissionAssessmentStatus, MarketplaceCommissionReadPort,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use uuid::Uuid;

use crate::dto::{
    ListMarketplaceSellerLedgerEntriesRequest, MAX_LEDGER_ENTRIES_PER_PAGE,
    MarketplaceLedgerAccountCode, MarketplaceLedgerEntryDirection,
    MarketplaceLedgerEntryListResponse, MarketplaceLedgerEntryResponse,
    MarketplaceLedgerTransactionResponse, MarketplaceLedgerTransactionStatus,
    PostMarketplaceOrderLedgerInput,
};
use crate::entities::{entry, transaction};
use crate::error::{MarketplaceLedgerError, MarketplaceLedgerResult};
use crate::receipts::{
    LedgerReceiptAdmission, NewLedgerReceipt, admit_receipt, complete_receipt,
    normalize_idempotency_key, posting_request_hash, replay_existing, replay_receipt,
    rollback_receipt,
};

const SOURCE_KIND_COMMISSION_BATCH: &str = "commission_assessment_batch";

pub struct MarketplaceLedgerService {
    db: DatabaseConnection,
    commission_reader: Arc<dyn MarketplaceCommissionReadPort>,
}

impl MarketplaceLedgerService {
    pub fn new(
        db: DatabaseConnection,
        commission_reader: Arc<dyn MarketplaceCommissionReadPort>,
    ) -> Self {
        Self {
            db,
            commission_reader,
        }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn post_order_with_receipt(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: PostMarketplaceOrderLedgerInput,
    ) -> MarketplaceLedgerResult<MarketplaceLedgerTransactionResponse> {
        if input.order_id.is_nil() {
            return Err(MarketplaceLedgerError::Validation(
                "order_id must not be nil".to_string(),
            ));
        }
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = posting_request_hash(actor_id, &input)?;
        if let Some(response) =
            replay_existing(&self.db, tenant_id, key.as_str(), hash.as_str()).await?
        {
            return Ok(response);
        }

        let assessments = self
            .commission_reader
            .list_assessments_by_order(
                context,
                ListMarketplaceCommissionAssessmentsByOrderRequest {
                    order_id: input.order_id,
                },
            )
            .await
            .map_err(map_commission_port_error)?;
        let batch = validate_assessment_batch(tenant_id, input.order_id, assessments)?;

        match admit_receipt(&self.db, tenant_id, actor_id, key, hash.as_str()).await? {
            LedgerReceiptAdmission::Replay(receipt) => replay_receipt(receipt, hash.as_str()),
            LedgerReceiptAdmission::New(receipt) => {
                let result = post_in_transaction(
                    &receipt,
                    tenant_id,
                    input.order_id,
                    input.posted_at,
                    batch,
                )
                .await;
                match result {
                    Ok(response) => complete_receipt(receipt, &response).await,
                    Err(error) => rollback_receipt(receipt, error).await,
                }
            }
        }
    }

    pub async fn read_order_ledger(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> MarketplaceLedgerResult<MarketplaceLedgerTransactionResponse> {
        let transaction = transaction::Entity::find()
            .filter(transaction::Column::TenantId.eq(tenant_id))
            .filter(transaction::Column::SourceKind.eq(SOURCE_KIND_COMMISSION_BATCH))
            .filter(transaction::Column::SourceId.eq(order_id))
            .one(&self.db)
            .await?
            .ok_or(MarketplaceLedgerError::TransactionNotFound(order_id))?;
        let entries = entry::Entity::find()
            .filter(entry::Column::TenantId.eq(tenant_id))
            .filter(entry::Column::TransactionId.eq(transaction.id))
            .order_by_asc(entry::Column::OrderLineItemId)
            .order_by_asc(entry::Column::AssessmentId)
            .order_by_asc(entry::Column::AccountCode)
            .all(&self.db)
            .await?
            .into_iter()
            .map(map_entry)
            .collect::<MarketplaceLedgerResult<Vec<_>>>()?;
        map_transaction(transaction, entries)
    }

    pub async fn list_seller_entries(
        &self,
        tenant_id: Uuid,
        mut request: ListMarketplaceSellerLedgerEntriesRequest,
    ) -> MarketplaceLedgerResult<MarketplaceLedgerEntryListResponse> {
        if request.seller_id.is_nil() {
            return Err(MarketplaceLedgerError::Validation(
                "seller_id must not be nil".to_string(),
            ));
        }
        request.currency_code = match request.currency_code.take() {
            Some(value) => Some(normalize_currency(value)?),
            None => None,
        };
        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, MAX_LEDGER_ENTRIES_PER_PAGE);
        let mut query = entry::Entity::find()
            .filter(entry::Column::TenantId.eq(tenant_id))
            .filter(entry::Column::SellerId.eq(request.seller_id))
            .filter(
                entry::Column::AccountCode.eq(MarketplaceLedgerAccountCode::SellerPayable.as_str()),
            );
        if let Some(currency_code) = request.currency_code {
            query = query.filter(entry::Column::CurrencyCode.eq(currency_code));
        }
        let paginator = query
            .order_by_desc(entry::Column::CreatedAt)
            .order_by_desc(entry::Column::Id)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await?
            .into_iter()
            .map(map_entry)
            .collect::<MarketplaceLedgerResult<Vec<_>>>()?;
        Ok(MarketplaceLedgerEntryListResponse {
            items,
            total,
            page,
            per_page,
        })
    }
}

struct AssessmentBatch {
    currency_code: String,
    debit_total_amount: i64,
    credit_total_amount: i64,
    assessments: Vec<MarketplaceCommissionAssessmentResponse>,
}

fn validate_assessment_batch(
    tenant_id: Uuid,
    order_id: Uuid,
    assessments: Vec<MarketplaceCommissionAssessmentResponse>,
) -> MarketplaceLedgerResult<AssessmentBatch> {
    if assessments.is_empty() {
        return Err(MarketplaceLedgerError::Validation(format!(
            "order {order_id} has no commission assessments"
        )));
    }
    let currency_code = normalize_currency(assessments[0].currency_code.clone())?;
    let mut assessment_ids = HashSet::with_capacity(assessments.len());
    let mut allocation_ids = HashSet::with_capacity(assessments.len());
    let mut debit_total = 0_i64;
    let mut credit_total = 0_i64;

    for assessment in &assessments {
        if assessment.tenant_id != tenant_id || assessment.order_id != order_id {
            return Err(MarketplaceLedgerError::Validation(
                "commission assessment batch does not match tenant or order scope".to_string(),
            ));
        }
        if assessment.status != MarketplaceCommissionAssessmentStatus::Assessed {
            return Err(MarketplaceLedgerError::Validation(format!(
                "commission assessment {} is `{}` and cannot be posted",
                assessment.id,
                assessment.status.as_str()
            )));
        }
        if normalize_currency(assessment.currency_code.clone())? != currency_code {
            return Err(MarketplaceLedgerError::Validation(format!(
                "order {order_id} commission assessments contain multiple currencies"
            )));
        }
        if !assessment_ids.insert(assessment.id) {
            return Err(MarketplaceLedgerError::Validation(format!(
                "commission assessment {} appears more than once",
                assessment.id
            )));
        }
        if !allocation_ids.insert(assessment.allocation_id) {
            return Err(MarketplaceLedgerError::Validation(format!(
                "allocation {} appears in multiple commission assessments",
                assessment.allocation_id
            )));
        }
        for (amount, field) in [
            (
                assessment.allocation_total_amount,
                "allocation_total_amount",
            ),
            (assessment.commission_amount, "commission_amount"),
            (assessment.seller_proceeds_amount, "seller_proceeds_amount"),
        ] {
            if amount < 0 {
                return Err(MarketplaceLedgerError::Validation(format!(
                    "assessment {} has negative {field}",
                    assessment.id
                )));
            }
        }
        let expected_total = assessment
            .commission_amount
            .checked_add(assessment.seller_proceeds_amount)
            .ok_or_else(|| {
                MarketplaceLedgerError::Validation(
                    "commission assessment credit total overflow".to_string(),
                )
            })?;
        if expected_total != assessment.allocation_total_amount {
            return Err(MarketplaceLedgerError::Validation(format!(
                "assessment {} is not balanced against its allocation total",
                assessment.id
            )));
        }
        debit_total = debit_total
            .checked_add(assessment.allocation_total_amount)
            .ok_or_else(|| {
                MarketplaceLedgerError::Validation("ledger debit total overflow".to_string())
            })?;
        credit_total = credit_total.checked_add(expected_total).ok_or_else(|| {
            MarketplaceLedgerError::Validation("ledger credit total overflow".to_string())
        })?;
    }
    if debit_total != credit_total {
        return Err(MarketplaceLedgerError::Validation(
            "commission assessment batch is not balanced".to_string(),
        ));
    }
    Ok(AssessmentBatch {
        currency_code,
        debit_total_amount: debit_total,
        credit_total_amount: credit_total,
        assessments,
    })
}

async fn post_in_transaction(
    receipt: &NewLedgerReceipt,
    tenant_id: Uuid,
    order_id: Uuid,
    posted_at: chrono::DateTime<chrono::FixedOffset>,
    batch: AssessmentBatch,
) -> MarketplaceLedgerResult<MarketplaceLedgerTransactionResponse> {
    if transaction::Entity::find()
        .filter(transaction::Column::TenantId.eq(tenant_id))
        .filter(transaction::Column::SourceKind.eq(SOURCE_KIND_COMMISSION_BATCH))
        .filter(transaction::Column::SourceId.eq(order_id))
        .one(&receipt.transaction)
        .await?
        .is_some()
    {
        return Err(MarketplaceLedgerError::OrderAlreadyPosted(order_id));
    }
    let assessment_ids = batch
        .assessments
        .iter()
        .map(|assessment| assessment.id)
        .collect::<Vec<_>>();
    if let Some(existing) = entry::Entity::find()
        .filter(entry::Column::TenantId.eq(tenant_id))
        .filter(entry::Column::AssessmentId.is_in(assessment_ids))
        .one(&receipt.transaction)
        .await?
    {
        return Err(MarketplaceLedgerError::AssessmentAlreadyPosted(
            existing.assessment_id,
        ));
    }

    let transaction_id = generate_id();
    let created_at = Utc::now().fixed_offset();
    let transaction_model = transaction::ActiveModel {
        id: Set(transaction_id),
        tenant_id: Set(tenant_id),
        source_kind: Set(SOURCE_KIND_COMMISSION_BATCH.to_string()),
        source_id: Set(order_id),
        order_id: Set(order_id),
        currency_code: Set(batch.currency_code.clone()),
        debit_total_amount: Set(batch.debit_total_amount),
        credit_total_amount: Set(batch.credit_total_amount),
        status: Set(MarketplaceLedgerTransactionStatus::Posted
            .as_str()
            .to_string()),
        posted_at: Set(posted_at),
        metadata: Set(serde_json::json!({
            "assessment_count": batch.assessments.len(),
            "source": SOURCE_KIND_COMMISSION_BATCH,
        })),
        created_at: Set(created_at),
    }
    .insert(&receipt.transaction)
    .await
    .map_err(|error| {
        if is_unique_constraint(&error) {
            MarketplaceLedgerError::OrderAlreadyPosted(order_id)
        } else {
            error.into()
        }
    })?;

    let mut entries = Vec::with_capacity(batch.assessments.len() * 3);
    let tx_ctx = LedgerTransactionContext {
        receipt,
        tenant_id,
        transaction_id,
        created_at,
    };
    for assessment in batch.assessments {
        entries.push(
            insert_entry(
                &tx_ctx,
                NewAssessmentEntryInput {
                    assessment: &assessment,
                    seller_id: None,
                    account_code: MarketplaceLedgerAccountCode::MarketplaceClearing,
                    direction: MarketplaceLedgerEntryDirection::Debit,
                    amount: assessment.allocation_total_amount,
                },
            )
            .await?,
        );
        entries.push(
            insert_entry(
                &tx_ctx,
                NewAssessmentEntryInput {
                    assessment: &assessment,
                    seller_id: None,
                    account_code: MarketplaceLedgerAccountCode::PlatformCommissionRevenue,
                    direction: MarketplaceLedgerEntryDirection::Credit,
                    amount: assessment.commission_amount,
                },
            )
            .await?,
        );
        entries.push(
            insert_entry(
                &tx_ctx,
                NewAssessmentEntryInput {
                    assessment: &assessment,
                    seller_id: Some(assessment.seller_id),
                    account_code: MarketplaceLedgerAccountCode::SellerPayable,
                    direction: MarketplaceLedgerEntryDirection::Credit,
                    amount: assessment.seller_proceeds_amount,
                },
            )
            .await?,
        );
    }
    entries.sort_by_key(|entry| {
        (
            entry.order_line_item_id,
            entry.assessment_id,
            entry.account_code.as_str().to_string(),
        )
    });
    map_transaction(transaction_model, entries)
}

struct LedgerTransactionContext<'a> {
    pub receipt: &'a NewLedgerReceipt,
    pub tenant_id: Uuid,
    pub transaction_id: Uuid,
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
}

struct NewAssessmentEntryInput<'a> {
    pub assessment: &'a MarketplaceCommissionAssessmentResponse,
    pub seller_id: Option<Uuid>,
    pub account_code: MarketplaceLedgerAccountCode,
    pub direction: MarketplaceLedgerEntryDirection,
    pub amount: i64,
}

async fn insert_entry(
    ctx: &LedgerTransactionContext<'_>,
    input: NewAssessmentEntryInput<'_>,
) -> MarketplaceLedgerResult<MarketplaceLedgerEntryResponse> {
    let model = entry::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(ctx.tenant_id),
        transaction_id: Set(ctx.transaction_id),
        order_id: Set(input.assessment.order_id),
        assessment_id: Set(input.assessment.id),
        allocation_id: Set(input.assessment.allocation_id),
        order_line_item_id: Set(input.assessment.order_line_item_id),
        seller_id: Set(input.seller_id),
        account_code: Set(input.account_code.as_str().to_string()),
        direction: Set(input.direction.as_str().to_string()),
        currency_code: Set(input.assessment.currency_code.clone()),
        amount: Set(input.amount),
        metadata: Set(serde_json::json!({
            "rule_id": input.assessment.rule_id,
            "rule_key": input.assessment.rule_key,
            "rule_version": input.assessment.rule_version,
        })),
        created_at: Set(ctx.created_at),
    }
    .insert(&ctx.receipt.transaction)
    .await
    .map_err(|error| {
        if is_unique_constraint(&error) {
            MarketplaceLedgerError::AssessmentAlreadyPosted(input.assessment.id)
        } else {
            error.into()
        }
    })?;
    map_entry(model)
}

fn map_transaction(
    model: transaction::Model,
    entries: Vec<MarketplaceLedgerEntryResponse>,
) -> MarketplaceLedgerResult<MarketplaceLedgerTransactionResponse> {
    let status =
        MarketplaceLedgerTransactionStatus::parse(model.status.as_str()).ok_or_else(|| {
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
    let direction =
        MarketplaceLedgerEntryDirection::parse(model.direction.as_str()).ok_or_else(|| {
            MarketplaceLedgerError::Validation(format!(
                "unknown ledger entry direction `{}`",
                model.direction
            ))
        })?;
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

fn normalize_currency(value: String) -> MarketplaceLedgerResult<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(MarketplaceLedgerError::Validation(
            "currency_code must contain exactly three ASCII letters".to_string(),
        ));
    }
    Ok(value)
}

fn map_commission_port_error(error: rustok_api::PortError) -> MarketplaceLedgerError {
    let (kind, message) = match error.kind {
        rustok_api::PortErrorKind::Validation => (
            crate::error::MarketplaceCommissionBoundaryKind::Validation,
            "marketplace commission request is invalid",
        ),
        rustok_api::PortErrorKind::NotFound => (
            crate::error::MarketplaceCommissionBoundaryKind::NotFound,
            "marketplace commission resource was not found",
        ),
        rustok_api::PortErrorKind::Conflict => (
            crate::error::MarketplaceCommissionBoundaryKind::Conflict,
            "marketplace commission operation conflicts with the current state",
        ),
        rustok_api::PortErrorKind::Forbidden => (
            crate::error::MarketplaceCommissionBoundaryKind::Forbidden,
            "marketplace commission permission was denied",
        ),
        rustok_api::PortErrorKind::Unavailable => (
            crate::error::MarketplaceCommissionBoundaryKind::Unavailable,
            "marketplace commission service is temporarily unavailable",
        ),
        rustok_api::PortErrorKind::Timeout => (
            crate::error::MarketplaceCommissionBoundaryKind::Timeout,
            "marketplace commission request timed out",
        ),
        rustok_api::PortErrorKind::InvariantViolation => (
            crate::error::MarketplaceCommissionBoundaryKind::InvariantViolation,
            "marketplace commission requires operator review",
        ),
    };

    MarketplaceLedgerError::CommissionBoundary {
        code: error.code,
        message: message.to_string(),
        retryable: error.retryable,
        kind,
    }
}

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
