use std::{cmp::Ordering, collections::HashSet, sync::Arc};

use chrono::Utc;
use rustok_api::PortContext;
use rustok_core::generate_id;
use rustok_marketplace_allocation::{
    ListMarketplaceAllocationsByOrderRequest, MarketplaceAllocationReadPort,
    MarketplaceOrderAllocationResponse,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::dto::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    CreateMarketplaceCommissionRuleVersionInput,
    ListMarketplaceCommissionAssessmentsBySellerRequest, ListMarketplaceCommissionRulesRequest,
    MAX_COMMISSION_ASSESSMENTS_PER_PAGE, MAX_COMMISSION_RULES_PER_PAGE,
    MarketplaceCommissionAssessmentListResponse, MarketplaceCommissionAssessmentResponse,
    MarketplaceCommissionAssessmentStatus, MarketplaceCommissionRuleListResponse,
    MarketplaceCommissionRuleResponse, MarketplaceCommissionRuleStatus,
};
use crate::entities::{assessment, rule};
use crate::error::{MarketplaceCommissionError, MarketplaceCommissionResult};
use crate::receipts::{
    CommissionReceiptAdmission, NewCommissionReceipt, admit_receipt, command_request_hash,
    complete_receipt, normalize_idempotency_key, replay_existing, replay_receipt, rollback_receipt,
};

const RESPONSE_KIND_RULE: &str = "rule";
const RESPONSE_KIND_ASSESSMENTS: &str = "assessments";

pub struct MarketplaceCommissionService {
    db: DatabaseConnection,
    allocation_reader: Arc<dyn MarketplaceAllocationReadPort>,
}

impl MarketplaceCommissionService {
    pub fn new(
        db: DatabaseConnection,
        allocation_reader: Arc<dyn MarketplaceAllocationReadPort>,
    ) -> Self {
        Self {
            db,
            allocation_reader,
        }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn create_rule_version_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: CreateMarketplaceCommissionRuleVersionInput,
    ) -> MarketplaceCommissionResult<MarketplaceCommissionRuleResponse> {
        let input = normalize_rule_input(input)?;
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash("create_commission_rule_version", actor_id, &input)?;
        match admit_receipt(
            &self.db,
            tenant_id,
            actor_id,
            key,
            "create_commission_rule_version",
            hash.as_str(),
        )
        .await?
        {
            CommissionReceiptAdmission::Replay(receipt) => replay_receipt(
                receipt,
                "create_commission_rule_version",
                hash.as_str(),
                RESPONSE_KIND_RULE,
            ),
            CommissionReceiptAdmission::New(receipt) => {
                let result = create_rule_in_transaction(&receipt, tenant_id, input).await;
                match result {
                    Ok(response) => complete_receipt(receipt, RESPONSE_KIND_RULE, &response).await,
                    Err(error) => rollback_receipt(receipt, error).await,
                }
            }
        }
    }

    pub async fn assess_order_with_receipt(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: AssessMarketplaceOrderCommissionsInput,
    ) -> MarketplaceCommissionResult<AssessMarketplaceOrderCommissionsResponse> {
        if input.order_id.is_nil() {
            return Err(MarketplaceCommissionError::Validation(
                "order_id must not be nil".to_string(),
            ));
        }
        let key = normalize_idempotency_key(idempotency_key)?;
        let hash = command_request_hash("assess_order_commissions", actor_id, &input)?;
        if let Some(response) = replay_existing(
            &self.db,
            tenant_id,
            key.as_str(),
            "assess_order_commissions",
            hash.as_str(),
            RESPONSE_KIND_ASSESSMENTS,
        )
        .await?
        {
            return Ok(response);
        }

        let allocations = self
            .allocation_reader
            .list_allocations_by_order(
                context,
                ListMarketplaceAllocationsByOrderRequest {
                    order_id: input.order_id,
                },
            )
            .await
            .map_err(map_allocation_port_error)?;
        if allocations.is_empty() {
            return Err(MarketplaceCommissionError::Validation(format!(
                "order {} has no marketplace allocations",
                input.order_id
            )));
        }
        validate_allocation_batch(tenant_id, input.order_id, &allocations)?;

        match admit_receipt(
            &self.db,
            tenant_id,
            actor_id,
            key,
            "assess_order_commissions",
            hash.as_str(),
        )
        .await?
        {
            CommissionReceiptAdmission::Replay(receipt) => replay_receipt(
                receipt,
                "assess_order_commissions",
                hash.as_str(),
                RESPONSE_KIND_ASSESSMENTS,
            ),
            CommissionReceiptAdmission::New(receipt) => {
                let result =
                    assess_in_transaction(&receipt, tenant_id, input.assessed_at, allocations)
                        .await;
                match result {
                    Ok(response) => {
                        complete_receipt(receipt, RESPONSE_KIND_ASSESSMENTS, &response).await
                    }
                    Err(error) => rollback_receipt(receipt, error).await,
                }
            }
        }
    }

    pub async fn get_assessment_by_allocation(
        &self,
        tenant_id: Uuid,
        allocation_id: Uuid,
    ) -> MarketplaceCommissionResult<MarketplaceCommissionAssessmentResponse> {
        assessment::Entity::find()
            .filter(assessment::Column::TenantId.eq(tenant_id))
            .filter(assessment::Column::AllocationId.eq(allocation_id))
            .one(&self.db)
            .await?
            .ok_or(MarketplaceCommissionError::AssessmentNotFound(
                allocation_id,
            ))
            .and_then(map_assessment)
    }

    pub async fn list_assessments_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> MarketplaceCommissionResult<Vec<MarketplaceCommissionAssessmentResponse>> {
        assessment::Entity::find()
            .filter(assessment::Column::TenantId.eq(tenant_id))
            .filter(assessment::Column::OrderId.eq(order_id))
            .order_by_asc(assessment::Column::OrderLineItemId)
            .order_by_asc(assessment::Column::Id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(map_assessment)
            .collect()
    }

    pub async fn list_assessments_by_seller(
        &self,
        tenant_id: Uuid,
        request: ListMarketplaceCommissionAssessmentsBySellerRequest,
    ) -> MarketplaceCommissionResult<MarketplaceCommissionAssessmentListResponse> {
        let page = request.page.max(1);
        let per_page = request
            .per_page
            .clamp(1, MAX_COMMISSION_ASSESSMENTS_PER_PAGE);
        let mut query = assessment::Entity::find()
            .filter(assessment::Column::TenantId.eq(tenant_id))
            .filter(assessment::Column::SellerId.eq(request.seller_id));
        if let Some(status) = request.status {
            query = query.filter(assessment::Column::Status.eq(status.as_str()));
        }
        let paginator = query
            .order_by_desc(assessment::Column::AssessedAt)
            .order_by_desc(assessment::Column::Id)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await?
            .into_iter()
            .map(map_assessment)
            .collect::<MarketplaceCommissionResult<Vec<_>>>()?;
        Ok(MarketplaceCommissionAssessmentListResponse {
            items,
            total,
            page,
            per_page,
        })
    }

    pub async fn list_rules(
        &self,
        tenant_id: Uuid,
        request: ListMarketplaceCommissionRulesRequest,
    ) -> MarketplaceCommissionResult<MarketplaceCommissionRuleListResponse> {
        let page = request.page.max(1);
        let per_page = request.per_page.clamp(1, MAX_COMMISSION_RULES_PER_PAGE);
        let mut query = rule::Entity::find().filter(rule::Column::TenantId.eq(tenant_id));
        if let Some(rule_key) = request.rule_key {
            query = query.filter(rule::Column::RuleKey.eq(rule_key));
        }
        if let Some(seller_id) = request.seller_id {
            query = query.filter(rule::Column::SellerId.eq(seller_id));
        }
        if let Some(listing_id) = request.listing_id {
            query = query.filter(rule::Column::ListingId.eq(listing_id));
        }
        if let Some(status) = request.status {
            query = query.filter(rule::Column::Status.eq(status.as_str()));
        }
        let paginator = query
            .order_by_desc(rule::Column::CreatedAt)
            .order_by_desc(rule::Column::Version)
            .paginate(&self.db, per_page);
        let total = paginator.num_items().await?;
        let items = paginator
            .fetch_page(page.saturating_sub(1))
            .await?
            .into_iter()
            .map(map_rule)
            .collect::<MarketplaceCommissionResult<Vec<_>>>()?;
        Ok(MarketplaceCommissionRuleListResponse {
            items,
            total,
            page,
            per_page,
        })
    }
}

async fn create_rule_in_transaction(
    receipt: &NewCommissionReceipt,
    tenant_id: Uuid,
    input: CreateMarketplaceCommissionRuleVersionInput,
) -> MarketplaceCommissionResult<MarketplaceCommissionRuleResponse> {
    let latest = rule::Entity::find()
        .filter(rule::Column::TenantId.eq(tenant_id))
        .filter(rule::Column::RuleKey.eq(input.rule_key))
        .order_by_desc(rule::Column::Version)
        .one(&receipt.transaction)
        .await?;
    let version = latest
        .map(|model| model.version)
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| {
            MarketplaceCommissionError::Validation("rule version overflow".to_string())
        })?;
    let model = rule::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        rule_key: Set(input.rule_key),
        version: Set(version),
        seller_id: Set(input.seller_id),
        listing_id: Set(input.listing_id),
        rate_bps: Set(input.rate_bps),
        fixed_amount: Set(input.fixed_amount),
        currency_code: Set(input.currency_code),
        priority: Set(input.priority),
        effective_from: Set(input.effective_from),
        effective_until: Set(input.effective_until),
        status: Set(input.status.as_str().to_string()),
        metadata: Set(input.metadata),
        created_at: Set(Utc::now().into()),
    }
    .insert(&receipt.transaction)
    .await?;
    map_rule(model)
}

async fn assess_in_transaction(
    receipt: &NewCommissionReceipt,
    tenant_id: Uuid,
    assessed_at: chrono::DateTime<chrono::FixedOffset>,
    allocations: Vec<MarketplaceOrderAllocationResponse>,
) -> MarketplaceCommissionResult<AssessMarketplaceOrderCommissionsResponse> {
    let allocation_ids = allocations
        .iter()
        .map(|allocation| allocation.id)
        .collect::<Vec<_>>();
    if let Some(existing) = assessment::Entity::find()
        .filter(assessment::Column::TenantId.eq(tenant_id))
        .filter(assessment::Column::AllocationId.is_in(allocation_ids))
        .order_by_asc(assessment::Column::AllocationId)
        .one(&receipt.transaction)
        .await?
    {
        return Err(MarketplaceCommissionError::AllocationAlreadyAssessed(
            existing.allocation_id,
        ));
    }

    let seller_ids = allocations
        .iter()
        .map(|allocation| allocation.seller_id)
        .collect::<HashSet<_>>();
    let listing_ids = allocations
        .iter()
        .map(|allocation| allocation.listing_id)
        .collect::<HashSet<_>>();
    let candidates = rule::Entity::find()
        .filter(rule::Column::TenantId.eq(tenant_id))
        .filter(rule::Column::Status.eq(MarketplaceCommissionRuleStatus::Active.as_str()))
        .filter(rule::Column::EffectiveFrom.lte(assessed_at))
        .filter(
            Condition::any()
                .add(rule::Column::EffectiveUntil.is_null())
                .add(rule::Column::EffectiveUntil.gt(assessed_at)),
        )
        .filter(
            Condition::any()
                .add(rule::Column::ListingId.is_null())
                .add(rule::Column::ListingId.is_in(listing_ids)),
        )
        .filter(
            Condition::any()
                .add(rule::Column::SellerId.is_null())
                .add(rule::Column::SellerId.is_in(seller_ids)),
        )
        .all(&receipt.transaction)
        .await?;

    let order_id = allocations[0].order_id;
    let mut responses = Vec::with_capacity(allocations.len());
    let mut commission_total = 0_i64;
    let mut proceeds_total = 0_i64;
    for allocation in allocations {
        let selected = select_rule(&candidates, &allocation)
            .ok_or(MarketplaceCommissionError::RuleNotMatched(allocation.id))?;
        let (commission_amount, seller_proceeds_amount) =
            calculate_commission(selected, &allocation)?;
        commission_total = commission_total
            .checked_add(commission_amount)
            .ok_or_else(|| {
                MarketplaceCommissionError::Validation("commission total overflow".to_string())
            })?;
        proceeds_total = proceeds_total
            .checked_add(seller_proceeds_amount)
            .ok_or_else(|| {
                MarketplaceCommissionError::Validation("proceeds total overflow".to_string())
            })?;
        let model = assessment::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            allocation_id: Set(allocation.id),
            order_id: Set(allocation.order_id),
            order_line_item_id: Set(allocation.order_line_item_id),
            seller_id: Set(allocation.seller_id),
            listing_id: Set(allocation.listing_id),
            rule_id: Set(selected.id),
            rule_key: Set(selected.rule_key),
            rule_version: Set(selected.version),
            currency_code: Set(allocation.currency_code.clone()),
            allocation_total_amount: Set(allocation.total_amount),
            rate_bps: Set(selected.rate_bps),
            fixed_amount: Set(selected.fixed_amount),
            commission_amount: Set(commission_amount),
            seller_proceeds_amount: Set(seller_proceeds_amount),
            status: Set(MarketplaceCommissionAssessmentStatus::Assessed
                .as_str()
                .to_string()),
            metadata: Set(serde_json::json!({
                "rule_specificity": rule_specificity(selected),
                "allocation_status": allocation.status.as_str(),
            })),
            assessed_at: Set(assessed_at),
            created_at: Set(Utc::now().into()),
        }
        .insert(&receipt.transaction)
        .await
        .map_err(|error| {
            if is_unique_constraint(&error) {
                MarketplaceCommissionError::AllocationAlreadyAssessed(allocation.id)
            } else {
                error.into()
            }
        })?;
        responses.push(map_assessment(model)?);
    }
    responses.sort_by_key(|response| (response.order_line_item_id, response.id));
    Ok(AssessMarketplaceOrderCommissionsResponse {
        order_id,
        assessments: responses,
        commission_total_amount: commission_total,
        seller_proceeds_total_amount: proceeds_total,
    })
}

fn select_rule<'a>(
    candidates: &'a [rule::Model],
    allocation: &MarketplaceOrderAllocationResponse,
) -> Option<&'a rule::Model> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate
                .listing_id
                .is_none_or(|listing_id| listing_id == allocation.listing_id)
                && candidate
                    .seller_id
                    .is_none_or(|seller_id| seller_id == allocation.seller_id)
                && (candidate.fixed_amount == 0
                    || candidate.currency_code.as_deref()
                        == Some(allocation.currency_code.as_str()))
        })
        .max_by(|left, right| compare_rules(left, right))
}

fn compare_rules(left: &rule::Model, right: &rule::Model) -> Ordering {
    rule_specificity(left)
        .cmp(&rule_specificity(right))
        .then_with(|| left.priority.cmp(&right.priority))
        .then_with(|| left.version.cmp(&right.version))
        .then_with(|| left.effective_from.cmp(&right.effective_from))
        .then_with(|| right.id.cmp(&left.id))
}

fn rule_specificity(rule: &rule::Model) -> u8 {
    if rule.listing_id.is_some() {
        2
    } else if rule.seller_id.is_some() {
        1
    } else {
        0
    }
}

fn calculate_commission(
    rule: &rule::Model,
    allocation: &MarketplaceOrderAllocationResponse,
) -> MarketplaceCommissionResult<(i64, i64)> {
    let percentage = i128::from(allocation.total_amount)
        .checked_mul(i128::from(rule.rate_bps))
        .ok_or_else(|| {
            MarketplaceCommissionError::Validation("commission multiplication overflow".to_string())
        })?
        / 10_000;
    let commission = percentage
        .checked_add(i128::from(rule.fixed_amount))
        .ok_or_else(|| {
            MarketplaceCommissionError::Validation("commission addition overflow".to_string())
        })?;
    if commission > i128::from(allocation.total_amount) {
        return Err(MarketplaceCommissionError::Validation(format!(
            "commission rule {} version {} exceeds allocation total {}",
            rule.rule_key, rule.version, allocation.id
        )));
    }
    let commission = i64::try_from(commission).map_err(|_| {
        MarketplaceCommissionError::Validation("commission amount overflow".to_string())
    })?;
    let proceeds = allocation
        .total_amount
        .checked_sub(commission)
        .ok_or_else(|| {
            MarketplaceCommissionError::Validation("seller proceeds underflow".to_string())
        })?;
    Ok((commission, proceeds))
}

fn normalize_rule_input(
    mut input: CreateMarketplaceCommissionRuleVersionInput,
) -> MarketplaceCommissionResult<CreateMarketplaceCommissionRuleVersionInput> {
    if input.rule_key.is_nil() {
        return Err(MarketplaceCommissionError::Validation(
            "rule_key must not be nil".to_string(),
        ));
    }
    if input.seller_id.is_some_and(|value| value.is_nil())
        || input.listing_id.is_some_and(|value| value.is_nil())
    {
        return Err(MarketplaceCommissionError::Validation(
            "seller_id and listing_id must not be nil when present".to_string(),
        ));
    }
    if !(0..=10_000).contains(&input.rate_bps) {
        return Err(MarketplaceCommissionError::Validation(
            "rate_bps must be between 0 and 10000".to_string(),
        ));
    }
    if input.fixed_amount < 0 {
        return Err(MarketplaceCommissionError::Validation(
            "fixed_amount must not be negative".to_string(),
        ));
    }
    input.currency_code = match input.currency_code.take() {
        Some(value) => Some(normalize_currency(value)?),
        None => None,
    };
    if input.fixed_amount > 0 && input.currency_code.is_none() {
        return Err(MarketplaceCommissionError::Validation(
            "currency_code is required when fixed_amount is positive".to_string(),
        ));
    }
    if input
        .effective_until
        .is_some_and(|until| until <= input.effective_from)
    {
        return Err(MarketplaceCommissionError::Validation(
            "effective_until must be later than effective_from".to_string(),
        ));
    }
    input.metadata = normalize_metadata(input.metadata)?;
    Ok(input)
}

fn validate_allocation_batch(
    tenant_id: Uuid,
    order_id: Uuid,
    allocations: &[MarketplaceOrderAllocationResponse],
) -> MarketplaceCommissionResult<()> {
    let mut ids = HashSet::with_capacity(allocations.len());
    for allocation in allocations {
        if allocation.tenant_id != tenant_id || allocation.order_id != order_id {
            return Err(MarketplaceCommissionError::Validation(
                "allocation batch does not match tenant or order scope".to_string(),
            ));
        }
        if !ids.insert(allocation.id) {
            return Err(MarketplaceCommissionError::Validation(format!(
                "allocation {} appears more than once",
                allocation.id
            )));
        }
        if allocation.total_amount < 0 {
            return Err(MarketplaceCommissionError::Validation(format!(
                "allocation {} has a negative total",
                allocation.id
            )));
        }
    }
    Ok(())
}

fn normalize_currency(value: String) -> MarketplaceCommissionResult<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(MarketplaceCommissionError::Validation(
            "currency_code must contain exactly three ASCII letters".to_string(),
        ));
    }
    Ok(value)
}

fn normalize_metadata(value: serde_json::Value) -> MarketplaceCommissionResult<serde_json::Value> {
    match value {
        serde_json::Value::Null => Ok(serde_json::json!({})),
        serde_json::Value::Object(_) => Ok(value),
        _ => Err(MarketplaceCommissionError::Validation(
            "metadata must be a JSON object".to_string(),
        )),
    }
}

fn map_rule(model: rule::Model) -> MarketplaceCommissionResult<MarketplaceCommissionRuleResponse> {
    let status =
        MarketplaceCommissionRuleStatus::parse(model.status.as_str()).ok_or_else(|| {
            MarketplaceCommissionError::Validation(format!(
                "unknown commission rule status `{}`",
                model.status
            ))
        })?;
    Ok(MarketplaceCommissionRuleResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        rule_key: model.rule_key,
        version: model.version,
        seller_id: model.seller_id,
        listing_id: model.listing_id,
        rate_bps: model.rate_bps,
        fixed_amount: model.fixed_amount,
        currency_code: model.currency_code,
        priority: model.priority,
        effective_from: model.effective_from,
        effective_until: model.effective_until,
        status,
        metadata: model.metadata,
        created_at: model.created_at,
    })
}

fn map_assessment(
    model: assessment::Model,
) -> MarketplaceCommissionResult<MarketplaceCommissionAssessmentResponse> {
    let status =
        MarketplaceCommissionAssessmentStatus::parse(model.status.as_str()).ok_or_else(|| {
            MarketplaceCommissionError::Validation(format!(
                "unknown commission assessment status `{}`",
                model.status
            ))
        })?;
    Ok(MarketplaceCommissionAssessmentResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        allocation_id: model.allocation_id,
        order_id: model.order_id,
        order_line_item_id: model.order_line_item_id,
        seller_id: model.seller_id,
        listing_id: model.listing_id,
        rule_id: model.rule_id,
        rule_key: model.rule_key,
        rule_version: model.rule_version,
        currency_code: model.currency_code,
        allocation_total_amount: model.allocation_total_amount,
        rate_bps: model.rate_bps,
        fixed_amount: model.fixed_amount,
        commission_amount: model.commission_amount,
        seller_proceeds_amount: model.seller_proceeds_amount,
        status,
        metadata: model.metadata,
        assessed_at: model.assessed_at,
        created_at: model.created_at,
    })
}

fn map_allocation_port_error(error: rustok_api::PortError) -> MarketplaceCommissionError {
    MarketplaceCommissionError::AllocationBoundary {
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
