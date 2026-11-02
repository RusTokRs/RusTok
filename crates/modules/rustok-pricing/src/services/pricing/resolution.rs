/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Pure Pricing selection/rule policy shared by single and batch quotes.

use std::cmp::Reverse;

use rust_decimal::Decimal;
use rustok_commerce_foundation::CommerceError;
use rustok_core::{
    error::{ErrorKind, RichError},
    money,
};
use rustok_pricing_persistence::entities::{price, price_list};

use super::{
    PriceListRule, PriceListRuleKind, PriceResolutionContext, PriceResolutionSource, ResolvedPrice,
    calculate_discount_percent, channel_scope_matches, is_sale_price, normalize_channel_slug,
    validate_discount_percent, validate_price_precision, validate_price_tier_quantities,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ResolutionError {
    #[error("Stored pricing data is invalid: {0}")]
    InvalidData(&'static str),
    #[error("More than one price row has winning priority")]
    Ambiguous,
    #[error("Pricing snapshot exceeds its row budget")]
    RowBudget,
    #[error(transparent)]
    Domain(Box<CommerceError>),
    #[error(transparent)]
    Storage(Box<sea_orm::DbErr>),
}

impl From<CommerceError> for ResolutionError {
    fn from(error: CommerceError) -> Self {
        Self::Domain(Box::new(error))
    }
}

impl From<sea_orm::DbErr> for ResolutionError {
    fn from(error: sea_orm::DbErr) -> Self {
        Self::Storage(Box::new(error))
    }
}

impl ResolutionError {
    pub(super) fn into_commerce(self) -> CommerceError {
        match self {
            Self::Domain(error) => *error,
            Self::Storage(error) => CommerceError::Database(*error),
            Self::InvalidData(reason) => RichError::new(ErrorKind::Internal, reason)
                .with_error_code("PRICING_RESOLUTION_DATA_INVALID")
                .into(),
            Self::Ambiguous => RichError::new(
                ErrorKind::Conflict,
                "Price rows have ambiguous winning priority",
            )
            .with_error_code("PRICING_PRICE_AMBIGUOUS")
            .into(),
            Self::RowBudget => RichError::new(ErrorKind::Conflict, "Pricing row budget exceeded")
                .with_error_code("PRICING_RESOLUTION_LIMIT")
                .into(),
        }
    }
}

pub(super) fn stored_rule(
    model: &price_list::Model,
) -> Result<Option<PriceListRule>, ResolutionError> {
    match (model.rule_kind.as_deref(), model.adjustment_percent) {
        (None, None) => Ok(None),
        (Some("percentage_discount"), Some(percent)) => {
            validate_discount_percent(percent)
                .map_err(|_| invalid_data("Stored price-list percentage is invalid"))?;
            Ok(Some(PriceListRule {
                kind: PriceListRuleKind::PercentageDiscount,
                adjustment_percent: percent,
            }))
        }
        _ => Err(invalid_data(
            "Stored price-list rule is unsupported or incomplete",
        )),
    }
}

pub(super) fn invalid_data(message: &'static str) -> ResolutionError {
    ResolutionError::InvalidData(message)
}

/// All scoped rows are owner-validated before selecting a winner. An invalid or ambiguous
/// applicable record is not permission to choose a different amount or currency.
pub(super) fn resolve(
    prices: Vec<price::Model>,
    context: &PriceResolutionContext,
    selected_list: Option<&price_list::Model>,
    rule: Option<&PriceListRule>,
) -> Result<Option<ResolvedPrice>, ResolutionError> {
    let quantity = context.quantity.unwrap_or(1);
    let mut candidates = prices
        .into_iter()
        .filter(|price| matches_context(price, context, quantity))
        .collect::<Vec<_>>();
    for price in &candidates {
        validate_row(price, &context.currency_code, selected_list)?;
    }
    candidates.sort_by_key(|price| priority(price, context));
    let mut candidates = candidates.into_iter();
    let Some(price) = candidates.next() else {
        return Ok(None);
    };
    if let Some(next) = candidates.next()
        && priority(&price, context) == priority(&next, context)
    {
        return Err(ResolutionError::Ambiguous);
    }

    let source = match price.price_list_id {
        Some(price_list_id) => PriceResolutionSource::PriceListOverride {
            price_id: price.id,
            price_list_id,
        },
        None => PriceResolutionSource::Base { price_id: price.id },
    };
    let mut resolved = ResolvedPrice {
        currency_code: context.currency_code.clone(),
        amount: price.amount,
        compare_at_amount: price.compare_at_amount,
        discount_percent: calculate_discount_percent(price.amount, price.compare_at_amount),
        on_sale: is_sale_price(price.amount, price.compare_at_amount),
        region_id: price.region_id,
        min_quantity: price.min_quantity,
        max_quantity: price.max_quantity,
        price_list_id: price.price_list_id,
        channel_id: price.channel_id,
        channel_slug: price.channel_slug,
        source,
    };
    if price.price_list_id.is_none()
        && let (Some(list), Some(rule)) = (selected_list, rule)
    {
        let reference = price.compare_at_amount.unwrap_or(price.amount);
        let multiplier = match rule.kind {
            PriceListRuleKind::PercentageDiscount => {
                (Decimal::from(100) - rule.adjustment_percent) / Decimal::from(100)
            }
        };
        let offered = reference
            .checked_mul(multiplier)
            .ok_or_else(|| invalid_data("Price-list adjustment exceeds the money range"))?;
        let offered = money::round_to_currency(offered, &context.currency_code).map_err(|_| {
            invalid_data("Price-list adjustment cannot be represented in its currency")
        })?;
        // Existing sale prices and list sales do not stack. A weaker discount, including an
        // offer rounded back to the same minor unit, must not raise or relabel an existing sale.
        if offered < resolved.amount {
            validate_price_precision(offered, Some(reference), &context.currency_code)
                .map_err(|_| invalid_data("Resolved price exceeds the currency money contract"))?;
            resolved.amount = offered;
            resolved.compare_at_amount = Some(reference);
            resolved.discount_percent = calculate_discount_percent(offered, Some(reference));
            resolved.on_sale = is_sale_price(offered, Some(reference));
            resolved.price_list_id = Some(list.id);
            resolved.source = PriceResolutionSource::PriceListRule {
                price_id: price.id,
                price_list_id: list.id,
                rule: rule.clone(),
            };
        }
    }
    Ok(Some(resolved))
}

fn matches_context(price: &price::Model, context: &PriceResolutionContext, quantity: i32) -> bool {
    if price.currency_code != context.currency_code
        || price
            .price_list_id
            .is_some_and(|id| Some(id) != context.price_list_id)
        || price
            .region_id
            .is_some_and(|id| Some(id) != context.region_id)
        || !channel_scope_matches(
            price.channel_id,
            price.channel_slug.as_deref(),
            context.channel_id,
            context.channel_slug.as_deref(),
        )
    {
        return false;
    }
    // Invalid windows must reach validation rather than disappear as an absent price.
    if validate_price_tier_quantities(price.min_quantity, price.max_quantity).is_err() {
        return true;
    }
    price.min_quantity.is_none_or(|min| quantity >= min)
        && price.max_quantity.is_none_or(|max| quantity <= max)
}

type Priority = (u8, u8, u8, Reverse<i32>, i32);

fn priority(price: &price::Model, context: &PriceResolutionContext) -> Priority {
    (
        u8::from(price.price_list_id != context.price_list_id),
        u8::from(
            price.channel_id.is_none()
                && normalize_channel_slug(price.channel_slug.as_deref()).is_none(),
        ),
        u8::from(price.region_id != context.region_id),
        // An authored tier starting at 1 overrides an untiered base row. Preserve that
        // existing row-class distinction; UUID order still never chooses a tied winner.
        Reverse(price.min_quantity.unwrap_or(0)),
        price.max_quantity.unwrap_or(i32::MAX),
    )
}

pub(super) fn validate_row(
    price: &price::Model,
    currency: &str,
    selected_list: Option<&price_list::Model>,
) -> Result<(), ResolutionError> {
    let stored_currency = rustok_core::money::normalize_currency_code(&price.currency_code)
        .map_err(|_| invalid_data("Stored price currency is invalid"))?;
    if stored_currency != price.currency_code || price.currency_code != currency {
        return Err(invalid_data(
            "Stored price currency must match its canonical currency",
        ));
    }
    if price.id.is_nil()
        || price.variant_id.is_nil()
        || price.region_id.is_some_and(|id| id.is_nil())
        || price.price_list_id.is_some_and(|id| id.is_nil())
        || price.channel_id.is_some_and(|id| id.is_nil())
        || price
            .channel_slug
            .as_deref()
            .is_some_and(|slug| slug.trim().is_empty())
        || price.amount < Decimal::ZERO
        || price
            .compare_at_amount
            .is_some_and(|reference| reference < price.amount)
    {
        return Err(invalid_data(
            "Stored price identity, scope or money is invalid",
        ));
    }
    validate_price_precision(price.amount, price.compare_at_amount, currency)
        .map_err(|_| invalid_data("Stored price is not exactly representable in its currency"))?;
    validate_price_tier_quantities(price.min_quantity, price.max_quantity)
        .map_err(|_| invalid_data("Stored price quantity window is invalid"))?;
    if let Some(price_list_id) = price.price_list_id {
        let list = selected_list
            .filter(|list| list.id == price_list_id)
            .ok_or_else(|| {
                invalid_data("Stored override does not belong to the selected price list")
            })?;
        if price.channel_id != list.channel_id
            || normalize_channel_slug(price.channel_slug.as_deref())
                != normalize_channel_slug(list.channel_slug.as_deref())
        {
            return Err(invalid_data(
                "Stored override scope differs from its price-list scope",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn context() -> PriceResolutionContext {
        PriceResolutionContext {
            currency_code: "USD".into(),
            region_id: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            quantity: Some(1),
        }
    }

    fn row(amount: i64) -> price::Model {
        price::Model {
            id: Uuid::new_v4(),
            variant_id: Uuid::new_v4(),
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".into(),
            region_id: None,
            amount: Decimal::new(amount, 0),
            compare_at_amount: None,
            legacy_amount: None,
            legacy_compare_at_amount: None,
            min_quantity: None,
            max_quantity: None,
        }
    }

    fn list() -> price_list::Model {
        let now = chrono::Utc::now();
        price_list::Model {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            r#type: "sale".into(),
            status: "active".into(),
            channel_id: None,
            channel_slug: None,
            rule_kind: Some("percentage_discount".into()),
            adjustment_percent: Some(Decimal::new(10, 0)),
            starts_at: None,
            ends_at: None,
            created_at: now.into(),
            updated_at: now.into(),
        }
    }

    #[test]
    fn source_is_the_actual_price_row_and_absence_stays_absent() {
        let price = row(20);
        let quote = resolve(vec![price.clone()], &context(), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(
            quote.source,
            PriceResolutionSource::Base { price_id: price.id }
        );
        assert!(resolve(vec![], &context(), None, None).unwrap().is_none());
    }

    #[test]
    fn winning_ties_are_conflicts_in_both_input_orders() {
        let first = row(100);
        let second = row(70);
        for rows in [
            vec![first.clone(), second.clone()],
            vec![second.clone(), first.clone()],
        ] {
            let error = resolve(rows, &context(), None, None).unwrap_err();
            assert!(matches!(error, ResolutionError::Ambiguous));
        }
    }

    #[test]
    fn explicit_minimum_one_remains_an_authored_tier_over_the_base_row() {
        let base = row(100);
        let mut tier = row(90);
        tier.min_quantity = Some(1);
        let quote = resolve(vec![base, tier.clone()], &context(), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(quote.source.price_id(), tier.id);
        assert_eq!(quote.amount, Decimal::new(90, 0));
    }

    #[test]
    fn legacy_mirrors_never_supply_authoritative_money() {
        let mut price = row(20);
        price.legacy_amount = Some(1);
        let quote = resolve(vec![price], &context(), None, None)
            .unwrap()
            .unwrap();
        assert_eq!(quote.amount, Decimal::new(20, 0));
    }

    #[test]
    fn channel_and_region_then_quantity_determine_priority_not_amount_or_uuid() {
        let mut ctx = context();
        ctx.channel_id = Some(Uuid::new_v4());
        ctx.channel_slug = Some("web".into());
        ctx.region_id = Some(Uuid::new_v4());
        ctx.quantity = Some(12);
        let global = row(10);
        let mut scoped = row(100);
        scoped.channel_id = ctx.channel_id;
        scoped.region_id = ctx.region_id;
        scoped.min_quantity = Some(10);
        scoped.max_quantity = Some(15);
        let mut wider = scoped.clone();
        wider.id = Uuid::new_v4();
        wider.max_quantity = Some(20);
        let price = resolve(vec![global, wider, scoped.clone()], &ctx, None, None)
            .unwrap()
            .unwrap();
        assert_eq!(price.source.price_id(), scoped.id);
    }

    #[test]
    fn alternate_channel_representations_do_not_create_a_hidden_winner() {
        let mut ctx = context();
        ctx.channel_id = Some(Uuid::new_v4());
        ctx.channel_slug = Some("web".into());
        let mut by_id = row(20);
        by_id.channel_id = ctx.channel_id;
        let mut by_slug = row(30);
        by_slug.channel_slug = ctx.channel_slug.clone();
        assert!(resolve(vec![by_id, by_slug], &ctx, None, None).is_err());
    }

    #[test]
    fn weaker_list_sales_never_raise_an_existing_sale_or_claim_its_provenance() {
        let list = list();
        let mut ctx = context();
        ctx.price_list_id = Some(list.id);
        let mut price = row(70);
        price.compare_at_amount = Some(Decimal::new(100, 0));
        let quote = resolve(
            vec![price.clone()],
            &ctx,
            Some(&list),
            stored_rule(&list).unwrap().as_ref(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(quote.amount, Decimal::new(70, 0));
        assert_eq!(quote.discount_percent, Some(Decimal::new(30, 0)));
        assert_eq!(quote.price_list_id, None);
        assert_eq!(
            quote.source,
            PriceResolutionSource::Base { price_id: price.id }
        );
    }

    #[test]
    fn discount_percent_describes_final_rounded_money_and_zero_is_a_real_price() {
        let mut list = list();
        list.adjustment_percent = Some(Decimal::new(50, 0));
        let mut ctx = context();
        ctx.currency_code = "JPY".into();
        ctx.price_list_id = Some(list.id);
        let mut price = row(3);
        price.currency_code = "JPY".into();
        let quote = resolve(
            vec![price.clone()],
            &ctx,
            Some(&list),
            stored_rule(&list).unwrap().as_ref(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(quote.amount, Decimal::new(2, 0));
        assert_eq!(quote.discount_percent, Some(Decimal::new(3333, 2)));
        list.adjustment_percent = Some(Decimal::new(100, 0));
        let free = resolve(
            vec![price],
            &ctx,
            Some(&list),
            stored_rule(&list).unwrap().as_ref(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(free.amount, Decimal::ZERO);
        assert!(free.on_sale);
    }

    #[test]
    fn unknown_partial_and_invalid_rule_metadata_is_not_a_base_price_fallback() {
        let mut list = list();
        list.rule_kind = Some("future_rule".into());
        assert!(stored_rule(&list).is_err());
        list.rule_kind = None;
        assert!(stored_rule(&list).is_err());
        list.rule_kind = Some("percentage_discount".into());
        for percent in [Decimal::ZERO, Decimal::new(-1, 0), Decimal::new(101, 0)] {
            list.adjustment_percent = Some(percent);
            assert!(stored_rule(&list).is_err());
        }
        list.rule_kind = None;
        list.adjustment_percent = None;
        assert!(stored_rule(&list).unwrap().is_none());
    }

    #[test]
    fn corrupt_matching_money_or_windows_cannot_disappear_into_unpriced() {
        let mut price = row(10);
        price.amount = Decimal::new(9995, 3);
        assert!(resolve(vec![price], &context(), None, None).is_err());
        let mut price = row(10);
        price.min_quantity = Some(10);
        price.max_quantity = Some(2);
        assert!(resolve(vec![price], &context(), None, None).is_err());
    }

    #[test]
    fn explicit_override_is_not_discounted_again() {
        let list = list();
        let mut ctx = context();
        ctx.price_list_id = Some(list.id);
        let mut override_row = row(80);
        override_row.price_list_id = Some(list.id);
        let quote = resolve(
            vec![row(100), override_row.clone()],
            &ctx,
            Some(&list),
            stored_rule(&list).unwrap().as_ref(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(quote.amount, Decimal::new(80, 0));
        assert_eq!(
            quote.source,
            PriceResolutionSource::PriceListOverride {
                price_id: override_row.id,
                price_list_id: list.id
            }
        );
    }
    #[test]
    fn inspection_and_quotes_share_canonical_stored_money_validation() {
        validate_row(&row(20), "USD", None).unwrap();
        let mut bad_currency = row(20);
        bad_currency.currency_code = "usd".to_owned();
        let mut bad_precision = row(20);
        bad_precision.amount = Decimal::new(9995, 3);
        let mut negative = row(-1);
        negative.compare_at_amount = None;
        for price in [bad_currency, bad_precision, negative] {
            assert!(matches!(
                validate_row(&price, "USD", None),
                Err(ResolutionError::InvalidData(_))
            ));
        }
    }
}
