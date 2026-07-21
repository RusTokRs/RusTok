use std::collections::{HashMap, HashSet};

use chrono::Utc;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::dto::{
    MarketplaceLedgerAccountCode, MarketplaceLedgerEntryDirection,
    MarketplaceLedgerTransactionResponse, MarketplaceSellerBalanceBucket,
    MarketplaceSellerBalanceResponse, ReadMarketplaceSellerBalanceRequest,
    RebuildMarketplaceSellerBalanceInput,
};
use crate::entities::{entry, entry_balance_bucket, reversal_line, seller_balance};
use crate::error::{MarketplaceLedgerError, MarketplaceLedgerResult};
use crate::MarketplaceLedgerService;

impl MarketplaceLedgerService {
    pub async fn read_seller_balance_projection(
        &self,
        tenant_id: Uuid,
        request: ReadMarketplaceSellerBalanceRequest,
    ) -> MarketplaceLedgerResult<MarketplaceSellerBalanceResponse> {
        validate_seller_id(request.seller_id)?;
        let currency_code = normalize_currency(request.currency_code)?;
        let model = seller_balance::Entity::find()
            .filter(seller_balance::Column::TenantId.eq(tenant_id))
            .filter(seller_balance::Column::SellerId.eq(request.seller_id))
            .filter(seller_balance::Column::CurrencyCode.eq(currency_code.clone()))
            .one(self.database())
            .await?
            .ok_or_else(|| MarketplaceLedgerError::SellerBalanceNotFound {
                seller_id: request.seller_id,
                currency_code,
            })?;
        map_balance(model)
    }

    pub async fn rebuild_seller_balance_projection(
        &self,
        tenant_id: Uuid,
        request: RebuildMarketplaceSellerBalanceInput,
    ) -> MarketplaceLedgerResult<MarketplaceSellerBalanceResponse> {
        validate_seller_id(request.seller_id)?;
        let currency_code = normalize_currency(request.currency_code)?;
        let entries = entry::Entity::find()
            .filter(entry::Column::TenantId.eq(tenant_id))
            .filter(entry::Column::SellerId.eq(request.seller_id))
            .filter(
                entry::Column::AccountCode
                    .eq(MarketplaceLedgerAccountCode::SellerPayable.as_str()),
            )
            .filter(entry::Column::CurrencyCode.eq(currency_code.clone()))
            .order_by_asc(entry::Column::CreatedAt)
            .order_by_asc(entry::Column::Id)
            .all(self.database())
            .await?;

        let entry_ids = entries.iter().map(|model| model.id).collect::<Vec<_>>();
        let (explicit_classifications, reversal_classifications) = if entry_ids.is_empty() {
            (HashMap::new(), HashMap::new())
        } else {
            let explicit = entry_balance_bucket::Entity::find()
                .filter(entry_balance_bucket::Column::TenantId.eq(tenant_id))
                .filter(entry_balance_bucket::Column::EntryId.is_in(entry_ids.clone()))
                .all(self.database())
                .await?
                .into_iter()
                .map(|model| (model.entry_id, model.balance_bucket))
                .collect::<HashMap<_, _>>();
            let reversal = reversal_line::Entity::find()
                .filter(reversal_line::Column::TenantId.eq(tenant_id))
                .filter(reversal_line::Column::EntryId.is_in(entry_ids))
                .all(self.database())
                .await?
                .into_iter()
                .filter_map(|model| {
                    model
                        .seller_balance_bucket
                        .map(|bucket| (model.entry_id, bucket))
                })
                .collect::<HashMap<_, _>>();
            (explicit, reversal)
        };

        let mut totals = BalanceTotals::default();
        for model in &entries {
            let direction = MarketplaceLedgerEntryDirection::parse(model.direction.as_str())
                .ok_or_else(|| {
                    MarketplaceLedgerError::Validation(format!(
                        "unknown ledger entry direction `{}`",
                        model.direction
                    ))
                })?;
            let bucket = explicit_classifications
                .get(&model.id)
                .or_else(|| reversal_classifications.get(&model.id))
                .map(|value| parse_bucket(value.as_str()))
                .transpose()?
                .unwrap_or(MarketplaceSellerBalanceBucket::Pending);
            totals.apply(bucket, direction, model.amount)?;
        }

        let source_entry_count = i64::try_from(entries.len()).map_err(|_| {
            MarketplaceLedgerError::Validation(
                "seller balance source entry count exceeds supported range".to_string(),
            )
        })?;
        let last_entry = entries.last();
        let current_balance = totals
            .pending
            .checked_add(totals.available)
            .and_then(|value| value.checked_add(totals.reserved))
            .ok_or_else(|| {
                MarketplaceLedgerError::Validation(
                    "seller current balance overflow during rebuild".to_string(),
                )
            })?;
        let negative_amount = if current_balance < 0 {
            current_balance.checked_neg().ok_or_else(|| {
                MarketplaceLedgerError::Validation(
                    "seller negative balance overflow during rebuild".to_string(),
                )
            })?
        } else {
            0
        };
        let now = Utc::now().fixed_offset();

        let existing = seller_balance::Entity::find()
            .filter(seller_balance::Column::TenantId.eq(tenant_id))
            .filter(seller_balance::Column::SellerId.eq(request.seller_id))
            .filter(seller_balance::Column::CurrencyCode.eq(currency_code.clone()))
            .one(self.database())
            .await?;
        let model = match existing {
            Some(model) => {
                let mut active = model.into_active_model();
                active.pending_amount = Set(totals.pending);
                active.available_amount = Set(totals.available);
                active.reserved_amount = Set(totals.reserved);
                active.paid_amount = Set(totals.paid);
                active.negative_amount = Set(negative_amount);
                active.source_entry_count = Set(source_entry_count);
                active.last_entry_id = Set(last_entry.map(|entry| entry.id));
                active.last_entry_created_at =
                    Set(last_entry.map(|entry| entry.created_at.clone()));
                active.rebuilt_at = Set(now.clone());
                active.updated_at = Set(now);
                active.update(self.database()).await?
            }
            None => {
                let active = seller_balance::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    seller_id: Set(request.seller_id),
                    currency_code: Set(currency_code.clone()),
                    pending_amount: Set(totals.pending),
                    available_amount: Set(totals.available),
                    reserved_amount: Set(totals.reserved),
                    paid_amount: Set(totals.paid),
                    negative_amount: Set(negative_amount),
                    source_entry_count: Set(source_entry_count),
                    last_entry_id: Set(last_entry.map(|entry| entry.id)),
                    last_entry_created_at: Set(last_entry.map(|entry| entry.created_at.clone())),
                    rebuilt_at: Set(now.clone()),
                    updated_at: Set(now),
                };
                match active.insert(self.database()).await {
                    Ok(model) => model,
                    Err(error) if is_unique_constraint(&error) => {
                        let model = seller_balance::Entity::find()
                            .filter(seller_balance::Column::TenantId.eq(tenant_id))
                            .filter(seller_balance::Column::SellerId.eq(request.seller_id))
                            .filter(
                                seller_balance::Column::CurrencyCode.eq(currency_code.clone()),
                            )
                            .one(self.database())
                            .await?
                            .ok_or(error)?;
                        let mut active = model.into_active_model();
                        active.pending_amount = Set(totals.pending);
                        active.available_amount = Set(totals.available);
                        active.reserved_amount = Set(totals.reserved);
                        active.paid_amount = Set(totals.paid);
                        active.negative_amount = Set(negative_amount);
                        active.source_entry_count = Set(source_entry_count);
                        active.last_entry_id = Set(last_entry.map(|entry| entry.id));
                        active.last_entry_created_at =
                            Set(last_entry.map(|entry| entry.created_at.clone()));
                        active.rebuilt_at = Set(now.clone());
                        active.updated_at = Set(now);
                        active.update(self.database()).await?
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        };
        map_balance(model)
    }

    pub async fn rebuild_seller_balances_for_transaction(
        &self,
        tenant_id: Uuid,
        transaction: &MarketplaceLedgerTransactionResponse,
    ) -> MarketplaceLedgerResult<Vec<MarketplaceSellerBalanceResponse>> {
        let mut scopes = HashSet::new();
        for entry in &transaction.entries {
            if entry.account_code == MarketplaceLedgerAccountCode::SellerPayable {
                if let Some(seller_id) = entry.seller_id {
                    scopes.insert((seller_id, entry.currency_code.clone()));
                }
            }
        }
        let mut balances = Vec::with_capacity(scopes.len());
        for (seller_id, currency_code) in scopes {
            balances.push(
                self.rebuild_seller_balance_projection(
                    tenant_id,
                    RebuildMarketplaceSellerBalanceInput {
                        seller_id,
                        currency_code,
                    },
                )
                .await?,
            );
        }
        balances.sort_by_key(|balance| (balance.seller_id, balance.currency_code.clone()));
        Ok(balances)
    }
}

#[derive(Default)]
struct BalanceTotals {
    pending: i64,
    available: i64,
    reserved: i64,
    paid: i64,
}

impl BalanceTotals {
    fn apply(
        &mut self,
        bucket: MarketplaceSellerBalanceBucket,
        direction: MarketplaceLedgerEntryDirection,
        amount: i64,
    ) -> MarketplaceLedgerResult<()> {
        if amount < 0 {
            return Err(MarketplaceLedgerError::Validation(
                "seller balance source entry amount must not be negative".to_string(),
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
                "seller balance overflow during rebuild".to_string(),
            )
        })?;
        Ok(())
    }
}

fn map_balance(
    model: seller_balance::Model,
) -> MarketplaceLedgerResult<MarketplaceSellerBalanceResponse> {
    let source_entry_count = u64::try_from(model.source_entry_count).map_err(|_| {
        MarketplaceLedgerError::Validation(format!(
            "seller balance projection {} has an invalid source entry count",
            model.id
        ))
    })?;
    Ok(MarketplaceSellerBalanceResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        seller_id: model.seller_id,
        currency_code: model.currency_code,
        pending_amount: model.pending_amount,
        available_amount: model.available_amount,
        reserved_amount: model.reserved_amount,
        paid_amount: model.paid_amount,
        negative_amount: model.negative_amount,
        source_entry_count,
        last_entry_id: model.last_entry_id,
        last_entry_created_at: model.last_entry_created_at,
        rebuilt_at: model.rebuilt_at,
        updated_at: model.updated_at,
    })
}

fn parse_bucket(value: &str) -> MarketplaceLedgerResult<MarketplaceSellerBalanceBucket> {
    MarketplaceSellerBalanceBucket::parse(value).ok_or_else(|| {
        MarketplaceLedgerError::Validation(format!(
            "unknown seller balance bucket `{value}`"
        ))
    })
}

fn validate_seller_id(seller_id: Uuid) -> MarketplaceLedgerResult<()> {
    if seller_id.is_nil() {
        return Err(MarketplaceLedgerError::Validation(
            "seller_id must not be nil".to_string(),
        ));
    }
    Ok(())
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

fn is_unique_constraint(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}
