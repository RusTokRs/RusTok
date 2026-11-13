use rust_decimal::Decimal;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
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

impl PricingService {
    #[instrument(skip(self))]
    pub async fn set_price_list_percentage_rule(
        &self,
        tenant_id: Uuid,
        _actor_id: Uuid,
        price_list_id: Uuid,
        adjustment_percent: Option<Decimal>,
    ) -> CommerceResult<Option<PriceListRule>> {
        if tenant_id.is_nil() || _actor_id.is_nil() || price_list_id.is_nil() {
            return Err(CommerceError::Validation(
                "tenant_id, actor_id, and price_list_id must not be nil UUIDs".to_string(),
            ));
        }
        let txn = self.db.begin().await?;
        let price_list = resolve_active_price_list_tx(&txn, tenant_id, price_list_id).await?;

        if let Some(percent) = adjustment_percent {
            validate_discount_percent(percent)?;
        }

        let mut active: entities::price_list::ActiveModel = price_list.into();
        active.rule_kind = Set(adjustment_percent.map(|_| "percentage_discount".to_string()));
        active.adjustment_percent = Set(adjustment_percent);
        let price_list = active.update(&txn).await?;

        txn.commit().await?;
        resolution::stored_rule(&price_list).map_err(ResolutionError::into_commerce)
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
        resolve_active_price_list(&self.db, tenant_id, price_list_id).await?;
        self.set_price_list_percentage_rule(tenant_id, actor_id, price_list_id, adjustment_percent)
            .await?;

        self.list_active_price_lists(tenant_id, Some(locale), fallback_locale.or(Some(locale)))
            .await?
            .into_iter()
            .find(|price_list| price_list.id == price_list_id)
            .ok_or_else(|| {
                CommerceError::Validation(
                    "price_list_id must reference an active price list".into(),
                )
            })
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
        if tenant_id.is_nil() || actor_id.is_nil() || variant_id.is_nil() {
            return Err(CommerceError::Validation(
                "tenant_id, actor_id, and variant_id must not be nil UUIDs".to_string(),
            ));
        }
        self.ensure_variant_tenant(tenant_id, variant_id).await?;
        let preview = self
            .preview_percentage_discount_with_channel(
                variant_id,
                currency_code,
                discount_percent,
                channel_id,
                channel_slug,
            )
            .await?;

        self.set_price_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            currency_code,
            preview.adjusted_amount,
            preview.compare_at_amount,
            preview.channel_id,
            preview.channel_slug.clone(),
            None,
            None,
        )
        .await?;

        Ok(preview)
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
        if tenant_id.is_nil() || actor_id.is_nil() || variant_id.is_nil() || price_list_id.is_nil() {
            return Err(CommerceError::Validation(
                "tenant_id, actor_id, variant_id, and price_list_id must not be nil UUIDs".to_string(),
            ));
        }
        self.ensure_variant_tenant(tenant_id, variant_id).await?;
        let preview = self
            .preview_price_list_percentage_discount_with_channel(
                tenant_id,
                variant_id,
                price_list_id,
                currency_code,
                discount_percent,
                channel_id,
                channel_slug,
            )
            .await?;

        let active_price_list_id = preview.price_list_id.ok_or_else(|| {
            CommerceError::Validation("price_list_id is required for price-list adjustments".into())
        })?;

        self.set_price_list_tier_with_channel(
            tenant_id,
            actor_id,
            variant_id,
            active_price_list_id,
            currency_code,
            preview.adjusted_amount,
            preview.compare_at_amount,
            preview.channel_id,
            preview.channel_slug.clone(),
            None,
            None,
        )
        .await?;

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
        self.require_embedded_variant_service_state()?;
        let (effective_channel_id, effective_channel_slug) = match price_list_id {
            Some(price_list_id) => {
                let price_list =
                    resolve_active_price_list(&self.db, tenant_id, price_list_id).await?;
                validate_or_inherit_price_list_scope(&price_list, channel_id, channel_slug)?
            }
            None => (channel_id, normalize_channel_slug(channel_slug.as_deref())),
        };

        if let Some(price_list_id) = price_list_id {
            self.set_price_list_tier_with_channel(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                currency_code,
                amount,
                compare_at_amount,
                effective_channel_id,
                effective_channel_slug.clone(),
                min_quantity,
                max_quantity,
            )
            .await?;
        } else {
            self.set_price_tier_with_channel(
                tenant_id,
                actor_id,
                variant_id,
                currency_code,
                amount,
                compare_at_amount,
                effective_channel_id,
                effective_channel_slug.clone(),
                min_quantity,
                max_quantity,
            )
            .await?;
        }

        self.get_variant_prices(tenant_id, variant_id)
            .await?
            .into_iter()
            .find(|price| {
                price.currency_code.eq_ignore_ascii_case(currency_code)
                    && price.price_list_id == price_list_id
                    && price.channel_id == effective_channel_id
                    && normalize_channel_slug(price.channel_slug.as_deref())
                        == effective_channel_slug
                    && price.min_quantity == min_quantity
                    && price.max_quantity == max_quantity
            })
            .map(super::projections::admin_pricing_price_from_model)
            .ok_or_else(|| CommerceError::Validation("updated pricing row was not found".into()))
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
        let price_list = resolve_active_price_list(&self.db, tenant_id, price_list_id).await?;
        let (channel_id, channel_slug) =
            validate_or_inherit_price_list_scope(&price_list, channel_id, channel_slug)?;
        self.set_scoped_price_tier(
            tenant_id,
            actor_id,
            variant_id,
            currency_code,
            amount,
            compare_at_amount,
            Some(price_list.id),
            channel_id,
            channel_slug,
            min_quantity,
            max_quantity,
        )
        .await
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
    ) -> CommerceResult<()> {
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
        let txn = self.db.begin().await?;
        let variant = load_variant_for_update_in_tx(&txn, tenant_id, variant_id).await?;

        let normalized_channel_slug = normalize_channel_slug(channel_slug.as_deref());
        validate_channel_scope_in_tx(
            &txn,
            tenant_id,
            channel_id,
            normalized_channel_slug.as_deref(),
        )
        .await?;

        let (price_list_id, channel_id, channel_slug) = match price_list_id {
            Some(price_list_id) => {
                let price_list =
                    resolve_active_price_list_tx(&txn, tenant_id, price_list_id).await?;
                validate_channel_scope_in_tx(
                    &txn,
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

        if amount < Decimal::ZERO {
            return Err(CommerceError::InvalidPrice(
                "Amount cannot be negative".into(),
            ));
        }
        validate_price_tier_quantities(min_quantity, max_quantity)?;
        if let Some(compare_at) = compare_at_amount
            && compare_at < amount
        {
            return Err(CommerceError::InvalidPrice(
                "Compare at price must be greater than amount".into(),
            ));
        }

        let existing = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .filter(entities::price::Column::CurrencyCode.eq(&currency_code))
            .filter(entities::price::Column::RegionId.is_null())
            .filter(super::queries::optional_uuid_filter(
                entities::price::Column::PriceListId,
                price_list_id,
            ))
            .filter(super::queries::optional_uuid_filter(
                entities::price::Column::ChannelId,
                channel_id,
            ))
            .filter(super::queries::optional_string_filter(
                entities::price::Column::ChannelSlug,
                channel_slug.clone(),
            ))
            .filter(super::queries::optional_int_filter(
                entities::price::Column::MinQuantity,
                min_quantity,
            ))
            .filter(super::queries::optional_int_filter(
                entities::price::Column::MaxQuantity,
                max_quantity,
            ))
            .one(&txn)
            .await?;

        let old_amount = existing.as_ref().map(|price| price.amount);

        match existing {
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
                price_active.channel_id = Set(channel_id);
                price_active.channel_slug = Set(channel_slug.clone());
                price_active.min_quantity = Set(min_quantity);
                price_active.max_quantity = Set(max_quantity);
                price_active.update(&txn).await?;
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
                price.insert(&txn).await?;
            }
        }

        let old_cents =
            old_amount.and_then(|amount| money::to_minor_units(amount, &currency_code).ok());
        let new_cents = money::to_minor_units(amount, &currency_code).map_err(|error| {
            CommerceError::Validation(format!(
                "price amount {amount} has no minor-unit form in {currency_code}: {error}"
            ))
        })?;

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

        self.event_bus
            .publish_in_tx(&txn, tenant_id, Some(actor_id), event)
            .await?;

        txn.commit().await?;
        Ok(())
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

        for price_input in &prices {
            let normalized_currency = normalize_resolution_currency(&price_input.currency_code)?;
            let normalized_channel_slug =
                normalize_channel_slug(price_input.channel_slug.as_deref());
            validate_channel_scope_in_tx(
                &txn,
                tenant_id,
                price_input.channel_id,
                normalized_channel_slug.as_deref(),
            )
            .await?;

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

            let existing = entities::price::Entity::find()
                .filter(entities::price::Column::VariantId.eq(variant_id))
                .filter(entities::price::Column::CurrencyCode.eq(&normalized_currency))
                .filter(entities::price::Column::RegionId.is_null())
                .filter(entities::price::Column::PriceListId.is_null())
                .filter(super::queries::optional_uuid_filter(
                    entities::price::Column::ChannelId,
                    price_input.channel_id,
                ))
                .filter(super::queries::optional_string_filter(
                    entities::price::Column::ChannelSlug,
                    normalize_channel_slug(price_input.channel_slug.as_deref()),
                ))
                .filter(entities::price::Column::MinQuantity.is_null())
                .filter(entities::price::Column::MaxQuantity.is_null())
                .one(&txn)
                .await?;

            let old_amount = existing.as_ref().map(|p| p.amount);

            match existing {
                Some(price) => {
                    let mut price_active: entities::price::ActiveModel = price.into();
                    price_active.amount = Set(price_input.amount);
                    price_active.compare_at_amount = Set(price_input.compare_at_amount);
                    price_active.channel_id = Set(price_input.channel_id);
                    price_active.channel_slug =
                        Set(normalize_channel_slug(price_input.channel_slug.as_deref()));
                    price_active.legacy_amount = Set(
                        crate::services::bootstrap::legacy_amount_units(price_input.amount),
                    );
                    price_active.legacy_compare_at_amount = Set(price_input
                        .compare_at_amount
                        .and_then(crate::services::bootstrap::legacy_amount_units));
                    price_active.update(&txn).await?;
                }
                None => {
                    let price = entities::price::ActiveModel {
                        id: Set(generate_id()),
                        variant_id: Set(variant_id),
                        price_list_id: Set(None),
                        channel_id: Set(price_input.channel_id),
                        channel_slug: Set(normalize_channel_slug(
                            price_input.channel_slug.as_deref(),
                        )),
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
                    price.insert(&txn).await?;
                }
            }

            let old_cents = old_amount
                .and_then(|amount| money::to_minor_units(amount, &normalized_currency).ok());
            let new_cents = money::to_minor_units(price_input.amount, &normalized_currency)
                .map_err(|error| {
                    CommerceError::Validation(format!(
                        "price amount {} has no minor-unit form in {normalized_currency}: {error}",
                        price_input.amount
                    ))
                })?;

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

            self.event_bus
                .publish_in_tx(&txn, tenant_id, Some(actor_id), event)
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
