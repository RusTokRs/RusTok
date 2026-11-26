use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QuerySelect, Set, TransactionTrait};
use std::collections::HashSet;
use tracing::instrument;
use uuid::Uuid;

use rustok_commerce_foundation::dto::PriceInput;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_core::events::ValidateEvent;
use rustok_core::{generate_id, money};
use rustok_events::DomainEvent;

use super::PricingService;
use super::resolution::{self, ResolutionError};
use super::scope::*;
use super::types::*;

mod entities {
    pub(crate) use crate::entities::{price, price_list, price_list_translation};
    pub use rustok_commerce_foundation::entities::product_variant;
}

/// An existing row must supply a truthful predecessor for PriceUpdated.
/// Absence is represented by None, never by a failed money conversion.
fn price_event_predecessor(
    existing: Option<&entities::price::Model>,
    currency: &str,
) -> CommerceResult<Option<i64>> {
    existing
        .map(|price| {
            if price.currency_code != currency {
                return Err(resolution::invalid_data("Stored price currency differs from event currency").into_commerce());
            }
            validate_price_precision(price.amount, None, currency)
                .map_err(|_| resolution::invalid_data("Stored price cannot supply an exact event predecessor").into_commerce())?;
            money::to_minor_units_exact(price.amount, currency)
                .map_err(|_| resolution::invalid_data("Stored price cannot supply an exact event predecessor").into_commerce())
        })
        .transpose()
}

/// Capture the persisted money, not a possibly altered request value.
/// Refuse any storage divergence before publishing the event or advancing Product.
fn price_event_successor(
    saved: &entities::price::Model,
    currency: &str,
    amount: Decimal,
    compare_at_amount: Option<Decimal>,
) -> CommerceResult<i64> {
    if saved.currency_code != currency
        || saved.amount != amount
        || saved.compare_at_amount != compare_at_amount
    {
        return Err(resolution::invalid_data("Saved price differs from authored money").into_commerce());
    }
    validate_price_precision(saved.amount, saved.compare_at_amount, currency)
        .map_err(|_| resolution::invalid_data("Saved price cannot supply an exact event successor").into_commerce())?;
    money::to_minor_units_exact(saved.amount, currency)
        .map_err(|_| resolution::invalid_data("Saved price cannot supply an exact event successor").into_commerce())
}

/// A single authoring command must not choose an arbitrary stored row.
fn unique_existing_price(
    mut rows: Vec<entities::price::Model>,
) -> CommerceResult<Option<entities::price::Model>> {
    if rows.len() > 1 {
        return Err(ResolutionError::Ambiguous.into_commerce());
    }
    Ok(rows.pop())
}

/// The resolved Channel UUID, not the submitted ID/slug representation,
/// determines whether two entries address the same batch price position.
fn admit_batch_price_identity(
    seen: &mut HashSet<(Option<Uuid>, String)>,
    channel_id: Option<Uuid>,
    currency: &str,
) -> CommerceResult<()> {
    if !seen.insert((channel_id, currency.to_owned())) {
        return Err(CommerceError::Validation(
            "Duplicate batch price for the same channel and currency".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PriceWriteIntent {
    Fixed {
        amount: Decimal,
        compare_at_amount: Option<Decimal>,
    },
    PercentageDiscount(Decimal),
}

impl PricingService {
    #[instrument(skip(self))]
    pub async fn set_price_list_percentage_rule(
        &self,
        tenant_id: Uuid,
        _actor_id: Uuid,
        price_list_id: Uuid,
        adjustment_percent: Option<Decimal>,
    ) -> CommerceResult<Option<PriceListRule>> {
        let txn = self.db.begin().await?;
        let price_list = Self::write_price_list_percentage_rule_in_tx(
            &txn, tenant_id, _actor_id, price_list_id, adjustment_percent,
        )
        .await?;
        let rule = resolution::stored_rule(&price_list).map_err(ResolutionError::into_commerce)?;
        txn.commit().await?;
        Ok(rule)
    }

    async fn write_price_list_percentage_rule_in_tx(
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        price_list_id: Uuid,
        adjustment_percent: Option<Decimal>,
    ) -> CommerceResult<entities::price_list::Model> {
        if tenant_id.is_nil() || actor_id.is_nil() || price_list_id.is_nil() {
            return Err(CommerceError::Validation(
                "tenant_id, actor_id, and price_list_id must not be nil UUIDs".to_string(),
            ));
        }
        if let Some(percent) = adjustment_percent {
            validate_price_list_percentage(percent)?;
        }
        let price_list = resolve_active_price_list_tx(txn, tenant_id, price_list_id).await?;
        let mut active: entities::price_list::ActiveModel = price_list.into();
        active.rule_kind = Set(adjustment_percent.map(|_| "percentage_discount".to_string()));
        active.adjustment_percent = Set(adjustment_percent);
        let saved = active.update(txn).await?;
        // Never commit a value that the owner cannot subsequently resolve.
        resolution::stored_rule(&saved).map_err(ResolutionError::into_commerce)?;
        if saved.adjustment_percent != adjustment_percent {
            return Err(resolution::invalid_data("Stored price-list percentage differs from the authored value").into_commerce());
        }
        Ok(saved)
    }

    #[instrument(skip(self))]
    pub async fn set_price_list_percentage_rule_projection(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        price_list_id: Uuid,
        adjustment_percent: Option<Decimal>,
        locale: &str,
        fallback_locale: Option<&str>,
    ) -> CommerceResult<ActivePriceListOption> {
        let txn = self.db.begin().await?;
        let saved = Self::write_price_list_percentage_rule_in_tx(
            &txn, tenant_id, actor_id, price_list_id, adjustment_percent,
        )
        .await?;
        let translations = entities::price_list_translation::Entity::find()
            .filter(entities::price_list_translation::Column::PriceListId.eq(saved.id))
            .all(&txn)
            .await?;
        let name = Self::resolve_price_list_name(&translations, Some(locale), fallback_locale.or(Some(locale)));
        let projection = ActivePriceListOption {
            id: saved.id,
            name,
            list_type: saved.r#type,
            channel_id: saved.channel_id,
            channel_slug: saved.channel_slug,
            rule_kind: saved.rule_kind,
            adjustment_percent: saved.adjustment_percent,
        };
        txn.commit().await?;
        Ok(projection)
    }

    #[instrument(skip(self))]
    pub async fn set_price_list_scope(
        &self,
        tenant_id: Uuid,
        _actor_id: Uuid,
        price_list_id: Uuid,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CommerceResult<ActivePriceListOption> {
        if tenant_id.is_nil() || _actor_id.is_nil() || price_list_id.is_nil() {
            return Err(CommerceError::Validation(
                "tenant_id, actor_id, and price_list_id must not be nil UUIDs".to_string(),
            ));
        }
        let txn = self.db.begin().await?;
        let price_list = resolve_active_price_list_tx(&txn, tenant_id, price_list_id).await?;
        let channel_slug = normalize_channel_slug(channel_slug.as_deref());
        validate_channel_scope_in_tx(&txn, tenant_id, channel_id, channel_slug.as_deref()).await?;

        let mut active_price_list: entities::price_list::ActiveModel = price_list.clone().into();
        active_price_list.channel_id = Set(channel_id);
        active_price_list.channel_slug = Set(channel_slug.clone());
        let updated_price_list = active_price_list.update(&txn).await?;

        let prices = entities::price::Entity::find()
            .filter(entities::price::Column::PriceListId.eq(price_list_id))
            .all(&txn)
            .await?;

        for price in prices {
            let mut active_price: entities::price::ActiveModel = price.into();
            active_price.channel_id = Set(channel_id);
            active_price.channel_slug = Set(channel_slug.clone());
            active_price.update(&txn).await?;
        }

        let translations = entities::price_list_translation::Entity::find()
            .filter(entities::price_list_translation::Column::PriceListId.eq(price_list_id))
            .all(&txn)
            .await?;
        let name = Self::resolve_price_list_name(&translations, None, None);

        txn.commit().await?;

        Ok(ActivePriceListOption {
            id: updated_price_list.id,
            name,
            list_type: updated_price_list.r#type,
            channel_id: updated_price_list.channel_id,
            channel_slug: updated_price_list.channel_slug,
            rule_kind: updated_price_list.rule_kind,
            adjustment_percent: updated_price_list.adjustment_percent,
        })
    }

    #[instrument(skip(self))]
    pub async fn apply_percentage_discount(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        self.apply_percentage_discount_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            currency_code,
            discount_percent,
            None,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    #[allow(clippy::too_many_arguments)]
    pub async fn apply_percentage_discount_with_channel(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        self.apply_scoped_percentage_discount(
            tenant_id, actor_id, variant_id, currency_code, discount_percent,
            None, channel_id, channel_slug,
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn apply_price_list_percentage_discount(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        price_list_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        self.apply_price_list_percentage_discount_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            currency_code,
            discount_percent,
            None,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    #[allow(clippy::too_many_arguments)]
    pub async fn apply_price_list_percentage_discount_with_channel(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        price_list_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        self.apply_scoped_percentage_discount(
            tenant_id, actor_id, variant_id, currency_code, discount_percent,
            Some(price_list_id), channel_id, channel_slug,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn apply_scoped_percentage_discount(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
        price_list_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        self.require_embedded_variant_service_state()?;
        validate_discount_percent(discount_percent)?;
        let txn = self.db.begin().await?;
        let (_, preview) = self.write_scoped_price_tier_in_tx(
            &txn, tenant_id, actor_id, variant_id, currency_code,
            PriceWriteIntent::PercentageDiscount(discount_percent),
            price_list_id, channel_id, channel_slug, None, None,
        ).await?;
        let preview = preview.ok_or_else(|| CommerceError::Core(
            rustok_core::error::Error::Internal("Discount write returned no adjustment".into()),
        ))?;
        txn.commit().await?;
        Ok(preview)
    }

    #[instrument(skip(self))]
    pub async fn set_price(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        amount: Decimal,
        compare_at_amount: Option<Decimal>,
    ) -> CommerceResult<()> {
        self.set_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            currency_code,
            amount,
            compare_at_amount,
            None,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    #[allow(clippy::too_many_arguments)]
    pub async fn set_price_tier(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        amount: Decimal,
        compare_at_amount: Option<Decimal>,
        min_quantity: Option<i32>,
        max_quantity: Option<i32>,
    ) -> CommerceResult<()> {
        self.set_price_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            currency_code,
            amount,
            compare_at_amount,
            None,
            None,
            min_quantity,
            max_quantity,
        )
        .await
    }

    #[instrument(skip(self))]
    #[allow(clippy::too_many_arguments)]
    pub async fn set_price_tier_with_channel(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        amount: Decimal,
        compare_at_amount: Option<Decimal>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
        min_quantity: Option<i32>,
        max_quantity: Option<i32>,
    ) -> CommerceResult<()> {
        self.set_scoped_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            currency_code,
            amount,
            compare_at_amount,
            None,
            channel_id,
            channel_slug,
            min_quantity,
            max_quantity,
        )
        .await
        .map(|_| ())
    }

    #[instrument(skip(self))]
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_admin_variant_price_with_channel(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        price_list_id: Option<Uuid>,
        currency_code: &str,
        amount: Decimal,
        compare_at_amount: Option<Decimal>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
        min_quantity: Option<i32>,
        max_quantity: Option<i32>,
    ) -> CommerceResult<AdminPricingPrice> {
        let saved = self
            .set_scoped_price_tier(
                tenant_id,
                actor_id,
                variant_id,
                currency_code,
                amount,
                compare_at_amount,
                price_list_id,
                channel_id,
                channel_slug,
                min_quantity,
                max_quantity,
            )
            .await?;

        Ok(super::projections::admin_pricing_price_from_model(saved))
    }

    #[instrument(skip(self))]
    #[allow(clippy::too_many_arguments)]
    pub async fn set_price_list_tier(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        price_list_id: Uuid,
        currency_code: &str,
        amount: Decimal,
        compare_at_amount: Option<Decimal>,
        min_quantity: Option<i32>,
        max_quantity: Option<i32>,
    ) -> CommerceResult<()> {
        self.set_price_list_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            price_list_id,
            currency_code,
            amount,
            compare_at_amount,
            None,
            None,
            min_quantity,
            max_quantity,
        )
        .await
    }

    #[instrument(skip(self))]
    #[allow(clippy::too_many_arguments)]
    pub async fn set_price_list_tier_with_channel(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        price_list_id: Uuid,
        currency_code: &str,
        amount: Decimal,
        compare_at_amount: Option<Decimal>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
        min_quantity: Option<i32>,
        max_quantity: Option<i32>,
    ) -> CommerceResult<()> {
        self.set_scoped_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            currency_code,
            amount,
            compare_at_amount,
            Some(price_list_id),
            channel_id,
            channel_slug,
            min_quantity,
            max_quantity,
        )
        .await
        .map(|_| ())
    }

    #[instrument(skip(self))]
    #[allow(clippy::too_many_arguments)]
    async fn set_scoped_price_tier(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        amount: Decimal,
        compare_at_amount: Option<Decimal>,
        price_list_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
        min_quantity: Option<i32>,
        max_quantity: Option<i32>,
    ) -> CommerceResult<entities::price::Model> {
        self.require_embedded_variant_service_state()?;
        let txn = self.db.begin().await?;
        let (saved, _) = self.write_scoped_price_tier_in_tx(
            &txn, tenant_id, actor_id, variant_id, currency_code,
            PriceWriteIntent::Fixed { amount, compare_at_amount },
            price_list_id, channel_id, channel_slug, min_quantity, max_quantity,
        ).await?;
        txn.commit().await?;
        Ok(saved)
    }

    #[allow(clippy::too_many_arguments)]
    async fn write_scoped_price_tier_in_tx(
        &self,
        txn: &sea_orm::DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        intent: PriceWriteIntent,
        price_list_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
        min_quantity: Option<i32>,
        max_quantity: Option<i32>,
    ) -> CommerceResult<(entities::price::Model, Option<PriceAdjustmentPreview>)> {
        self.require_embedded_variant_service_state()?;
        if tenant_id.is_nil() || actor_id.is_nil() || variant_id.is_nil() {
            return Err(CommerceError::Validation(
                "tenant_id, actor_id, and variant_id must not be nil UUIDs".to_string(),
            ));
        }
        if let Some(price_list_id) = price_list_id
            && price_list_id.is_nil()
        {
            return Err(CommerceError::Validation(
                "price_list_id must not be a nil UUID".to_string(),
            ));
        }
        let currency_code = normalize_resolution_currency(currency_code)?;
        let variant = load_variant_for_update_in_tx(txn, tenant_id, variant_id).await?;

        let normalized_channel_slug = normalize_channel_slug(channel_slug.as_deref());


        let (price_list_id, channel_id, channel_slug) = match price_list_id {
            Some(price_list_id) => {
                let price_list =
                    resolve_active_price_list_tx(txn, tenant_id, price_list_id).await?;
                if matches!(&intent, PriceWriteIntent::PercentageDiscount(_))
                    && !channel_scope_matches(
                        price_list.channel_id, price_list.channel_slug.as_deref(),
                        channel_id, normalized_channel_slug.as_deref(),
                    )
                {
                    return Err(CommerceError::Validation(
                        "price_list_id is not available for the requested channel".to_owned(),
                    ));
                }
                validate_channel_scope_in_tx(
                    txn,
                    tenant_id,
                    price_list.channel_id,
                    price_list.channel_slug.as_deref(),
                )
                .await?;
                let (channel_id, channel_slug) =
                    validate_or_inherit_price_list_scope(&price_list, channel_id, channel_slug)?;
                (
                    Some(price_list_id),
                    channel_id,
                    normalize_channel_slug(channel_slug.as_deref()),
                )
            }
            None => (None, channel_id, normalized_channel_slug),
        };

        let resolved_channel = resolve_channel_identity(
            txn, tenant_id, channel_id, channel_slug.as_deref(),
        ).await?;
        let scope = if price_list_id.is_none() {
            super::queries::base_price_channel_filter(resolved_channel.as_ref())
        } else {
            super::queries::exact_price_channel_filter(channel_id, channel_slug.clone())
        };
        validate_price_tier_quantities(min_quantity, max_quantity)?;

        // INVARIANT: the Variant lock and (when selected) price-list lock above
        // protect the same canonical row through calculation, mutation and commit.
        let existing_rows = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .filter(entities::price::Column::CurrencyCode.eq(&currency_code))
            .filter(entities::price::Column::RegionId.is_null())
            .filter(super::queries::optional_uuid_filter(
                entities::price::Column::PriceListId,
                price_list_id,
            ))
            .filter(scope)
            .filter(super::queries::optional_int_filter(
                entities::price::Column::MinQuantity,
                min_quantity,
            ))
            .filter(super::queries::optional_int_filter(
                entities::price::Column::MaxQuantity,
                max_quantity,
            ))
            .limit(2)
            .all(txn)
            .await?;
        let existing = unique_existing_price(existing_rows)?;
        let preview = match intent {
            PriceWriteIntent::Fixed { .. } => None,
            PriceWriteIntent::PercentageDiscount(percent) => {
                let price = existing.as_ref().ok_or_else(|| {
                    CommerceError::InvalidPrice(format!("No canonical price found for currency {currency_code}"))
                })?;
                Some(super::queries::percentage_discount_preview(price, &currency_code, percent)?)
            }
        };
        let (amount, compare_at_amount) = match intent {
            PriceWriteIntent::Fixed { amount, compare_at_amount } => (amount, compare_at_amount),
            PriceWriteIntent::PercentageDiscount(_) => {
                let preview = preview.as_ref().ok_or_else(|| CommerceError::Core(
                    rustok_core::error::Error::Internal("Discount calculation is missing".into()),
                ))?;
                (preview.adjusted_amount, preview.compare_at_amount)
            }
        };
        if amount < Decimal::ZERO {
            return Err(CommerceError::InvalidPrice("Amount cannot be negative".into()));
        }
        if compare_at_amount.is_some_and(|compare_at| compare_at < amount) {
            return Err(CommerceError::InvalidPrice("Compare at price must be greater than amount".into()));
        }
        validate_price_precision(amount, compare_at_amount, &currency_code)?;

        let old_cents = price_event_predecessor(existing.as_ref(), &currency_code)?;

        let saved = match existing {
            Some(price) => {
                let mut price_active: entities::price::ActiveModel = price.into();
                price_active.amount = Set(amount);
                price_active.compare_at_amount = Set(compare_at_amount);
                price_active.legacy_amount =
                    Set(crate::services::bootstrap::legacy_amount_units(amount));
                price_active.legacy_compare_at_amount = Set(
                    compare_at_amount.and_then(crate::services::bootstrap::legacy_amount_units)
                );
                price_active.price_list_id = Set(price_list_id);
                // An alias-based update must preserve the existing row scope.
                // The requested identity was already verified under the SKU lock.
                price_active.min_quantity = Set(min_quantity);
                price_active.max_quantity = Set(max_quantity);
                price_active.update(txn).await?
            }
            None => {
                let price = entities::price::ActiveModel {
                    id: Set(generate_id()),
                    variant_id: Set(variant_id),
                    price_list_id: Set(price_list_id),
                    channel_id: Set(channel_id),
                    channel_slug: Set(channel_slug),
                    currency_code: Set(currency_code.clone()),
                    region_id: Set(None),
                    amount: Set(amount),
                    compare_at_amount: Set(compare_at_amount),
                    legacy_amount: Set(crate::services::bootstrap::legacy_amount_units(amount)),
                    legacy_compare_at_amount: Set(
                        compare_at_amount.and_then(crate::services::bootstrap::legacy_amount_units)
                    ),
                    min_quantity: Set(min_quantity),
                    max_quantity: Set(max_quantity),
                };
                price.insert(txn).await?
            }
        };

        let new_cents = price_event_successor(&saved, &currency_code, amount, compare_at_amount)?;

        let event = DomainEvent::PriceUpdated {
            variant_id,
            product_id: variant.product_id,
            currency: currency_code,
            old_amount: old_cents,
            new_amount: new_cents,
        };
        event
            .validate()
            .map_err(|e| CommerceError::Validation(format!("Invalid price event: {}", e)))?;

        let price_event_id = self.event_bus
            .publish_in_tx_with_envelope_id(txn, tenant_id, Some(actor_id), event)
            .await?;
        if saved.price_list_id.is_none()
            && saved.channel_id.is_none()
            && saved.channel_slug.is_none()
            && saved.region_id.is_none()
            && saved.min_quantity.is_none()
            && saved.max_quantity.is_none()
        {
            record_catalog_price_revision(txn, tenant_id, variant.product_id, variant_id, price_event_id)
                .await?;
        }

        Ok((saved, preview))
    }

    /// Set multiple prices for a variant in a single atomic transaction.
    /// If any price is invalid the whole operation is rolled back.
    #[instrument(skip(self, prices))]
    pub async fn set_prices(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        prices: Vec<PriceInput>,
    ) -> CommerceResult<()> {
        self.require_embedded_variant_service_state()?;
        if tenant_id.is_nil() || actor_id.is_nil() || variant_id.is_nil() {
            return Err(CommerceError::Validation(
                "tenant_id, actor_id, and variant_id must not be nil UUIDs".to_string(),
            ));
        }
        let txn = self.db.begin().await?;
        let variant = load_variant_for_update_in_tx(&txn, tenant_id, variant_id).await?;

        // Admit the complete batch before writing any row or event.
        let mut seen = HashSet::new();
        let mut prepared = Vec::with_capacity(prices.len());
        for price_input in &prices {
            let normalized_currency = normalize_resolution_currency(&price_input.currency_code)?;
            let normalized_channel_slug =
                normalize_channel_slug(price_input.channel_slug.as_deref());
            let resolved_channel = resolve_channel_identity(
                &txn,
                tenant_id,
                price_input.channel_id,
                normalized_channel_slug.as_deref(),
            )
            .await?;
            admit_batch_price_identity(
                &mut seen, resolved_channel.as_ref().map(|channel| channel.id), &normalized_currency,
            )?;
            let scope = super::queries::base_price_channel_filter(resolved_channel.as_ref());

            if price_input.amount < Decimal::ZERO {
                return Err(CommerceError::InvalidPrice(
                    "Amount cannot be negative".into(),
                ));
            }
            if let Some(compare_at) = price_input.compare_at_amount
                && compare_at < price_input.amount
            {
                return Err(CommerceError::InvalidPrice(
                    "Compare at price must be greater than amount".into(),
                ));
            }

            validate_price_precision(
                price_input.amount,
                price_input.compare_at_amount,
                &normalized_currency,
            )?;

            prepared.push((price_input, normalized_currency, normalized_channel_slug, scope));
        }

        let mut last_catalog_price_event = None;
        for (price_input, normalized_currency, normalized_channel_slug, scope) in prepared {
            let existing_rows = entities::price::Entity::find()
                .filter(entities::price::Column::VariantId.eq(variant_id))
                .filter(entities::price::Column::CurrencyCode.eq(&normalized_currency))
                .filter(entities::price::Column::RegionId.is_null())
                .filter(entities::price::Column::PriceListId.is_null())
                .filter(scope)
                .filter(entities::price::Column::MinQuantity.is_null())
                .filter(entities::price::Column::MaxQuantity.is_null())
                .limit(2)
                .all(&txn)
                .await?;
            let existing = unique_existing_price(existing_rows)?;

            let old_cents = price_event_predecessor(existing.as_ref(), &normalized_currency)?;

            let saved = match existing {
                Some(price) => {
                    let mut price_active: entities::price::ActiveModel = price.into();
                    price_active.amount = Set(price_input.amount);
                    price_active.compare_at_amount = Set(price_input.compare_at_amount);
                    // Preserve the stored scope when a different alias selects it.
                    price_active.legacy_amount = Set(
                        crate::services::bootstrap::legacy_amount_units(price_input.amount),
                    );
                    price_active.legacy_compare_at_amount = Set(price_input
                        .compare_at_amount
                        .and_then(crate::services::bootstrap::legacy_amount_units));
                    price_active.update(&txn).await?
                }
                None => {
                    let price = entities::price::ActiveModel {
                        id: Set(generate_id()),
                        variant_id: Set(variant_id),
                        price_list_id: Set(None),
                        channel_id: Set(price_input.channel_id),
                        channel_slug: Set(normalized_channel_slug.clone()),
                        currency_code: Set(normalized_currency.clone()),
                        region_id: Set(None),
                        amount: Set(price_input.amount),
                        compare_at_amount: Set(price_input.compare_at_amount),
                        legacy_amount: Set(crate::services::bootstrap::legacy_amount_units(
                            price_input.amount,
                        )),
                        legacy_compare_at_amount: Set(price_input
                            .compare_at_amount
                            .and_then(crate::services::bootstrap::legacy_amount_units)),
                        min_quantity: Set(None),
                        max_quantity: Set(None),
                    };
                    price.insert(&txn).await?
                }
            };

            let new_cents = price_event_successor(
                &saved, &normalized_currency, price_input.amount, price_input.compare_at_amount,
            )?;

            let event = DomainEvent::PriceUpdated {
                variant_id,
                product_id: variant.product_id,
                currency: normalized_currency,
                old_amount: old_cents,
                new_amount: new_cents,
            };
            event
                .validate()
                .map_err(|e| CommerceError::Validation(format!("Invalid price event: {}", e)))?;

            let price_event_id = self.event_bus
                .publish_in_tx_with_envelope_id(&txn, tenant_id, Some(actor_id), event)
                .await?;
            if saved.price_list_id.is_none()
                && saved.channel_id.is_none()
                && saved.channel_slug.is_none()
                && saved.region_id.is_none()
                && saved.min_quantity.is_none()
                && saved.max_quantity.is_none()
            {
                last_catalog_price_event = Some(price_event_id);
            }
        }
        if let Some(price_event_id) = last_catalog_price_event {
            record_catalog_price_revision(&txn, tenant_id, variant.product_id, variant_id, price_event_id)
                .await?;
        }

        txn.commit().await?;
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn apply_discount(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
    ) -> CommerceResult<Decimal> {
        let preview = self
            .apply_percentage_discount(
                tenant_id,
                actor_id,
                variant_id,
                currency_code,
                discount_percent,
            )
            .await?;

        Ok(preview.adjusted_amount)
    }

    pub(crate) async fn ensure_variant_tenant(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
    ) -> CommerceResult<()> {
        self.require_embedded_variant_service_state()?;
        if tenant_id.is_nil() || variant_id.is_nil() {
            return Err(CommerceError::Validation(
                "tenant_id and variant_id must not be nil UUIDs".to_string(),
            ));
        }
        entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .map(|_| ())
            .ok_or(CommerceError::VariantNotFound(variant_id))
    }
}

async fn record_catalog_price_revision(
    txn: &sea_orm::DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    variant_id: Uuid,
    price_event_id: Uuid,
) -> CommerceResult<()> {
    rustok_product::services::catalog_references::record_catalog_base_price_change_in_tx(
        txn, tenant_id, product_id, variant_id, price_event_id,
    )
    .await
    .map_err(|error| match error {
        rustok_product::CommerceError::Database(error) => CommerceError::Database(error),
        rustok_product::CommerceError::Core(error) => CommerceError::Core(error),
        _ => CommerceError::Core(rustok_core::error::Error::Internal(
            "Product catalog price revision could not be recorded".into(),
        )),
    })
}

#[cfg(test)]
mod batch_price_admission_tests {
    use super::*;

    fn row(id: u128) -> entities::price::Model {
        entities::price::Model {
            id: Uuid::from_u128(id),
            variant_id: Uuid::from_u128(100),
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            currency_code: "USD".to_owned(),
            region_id: None,
            amount: Decimal::from(10),
            compare_at_amount: None,
            legacy_amount: None,
            legacy_compare_at_amount: None,
            min_quantity: None,
            max_quantity: None,
        }
    }

    #[test]
    fn event_predecessor_distinguishes_absence_from_invalid_stored_money() {
        assert_eq!(price_event_predecessor(None, "USD").unwrap(), None);
        let mut price = row(1);
        assert_eq!(price_event_predecessor(Some(&price), "USD").unwrap(), Some(1000));
        price.amount = Decimal::ZERO;
        assert_eq!(price_event_predecessor(Some(&price), "USD").unwrap(), Some(0));
        for invalid in [Decimal::new(10001, 3), Decimal::new(-1, 0), Decimal::new(10_000_000_000, 0)] {
            price.amount = invalid;
            assert!(price_event_predecessor(Some(&price), "USD").is_err());
        }
        price.amount = Decimal::ONE;
        assert!(price_event_predecessor(Some(&price), "EUR").is_err());
    }

    #[test]
    fn event_successor_requires_saved_money_to_match_the_authored_money() {
        let mut price = row(1);
        price.currency_code = "KWD".into();
        price.amount = Decimal::new(79994, 3);
        price.compare_at_amount = Some(Decimal::new(99996, 3));
        assert_eq!(price_event_successor(&price, "KWD", price.amount, price.compare_at_amount).unwrap(), 79994);
        assert!(price_event_successor(&price, "KWD", Decimal::new(79995, 3), price.compare_at_amount).is_err());
        assert!(price_event_successor(&price, "KWD", price.amount, None).is_err());
        assert!(price_event_successor(&price, "USD", price.amount, price.compare_at_amount).is_err());
    }

    #[test]
    fn both_price_writers_refuse_multiple_matching_stored_rows() {
        assert!(unique_existing_price(Vec::new()).unwrap().is_none());
        assert_eq!(unique_existing_price(vec![row(1)]).unwrap().unwrap().id, row(1).id);
        assert!(unique_existing_price(vec![row(1), row(2)]).is_err());
        assert!(unique_existing_price(vec![row(2), row(1)]).is_err());
    }

    #[test]
    fn batch_rejects_duplicate_normalized_currency_and_resolved_channel() {
        let mut seen = HashSet::new();
        let channel = Some(Uuid::from_u128(1));
        let other_channel = Some(Uuid::from_u128(2));
        let usd = normalize_resolution_currency(" usd ").unwrap();
        assert!(admit_batch_price_identity(&mut seen, channel, &usd).is_ok());
        assert!(admit_batch_price_identity(&mut seen, channel, "USD").is_err());
        assert!(admit_batch_price_identity(&mut seen, other_channel, "USD").is_ok());
        assert!(admit_batch_price_identity(&mut seen, channel, "EUR").is_ok());
        assert!(admit_batch_price_identity(&mut seen, None, "USD").is_ok());
        assert!(admit_batch_price_identity(&mut seen, None, "USD").is_err());
    }
}
