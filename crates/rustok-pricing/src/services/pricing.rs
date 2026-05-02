use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use tracing::instrument;
use uuid::Uuid;

use rustok_core::events::ValidateEvent;
use rustok_core::{generate_id, locale_tags_match, normalize_locale_tag};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_product::CatalogService;

use rustok_commerce_foundation::dto::PriceInput;
use rustok_commerce_foundation::entities;
use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceResolutionContext {
    pub currency_code: String,
    pub region_id: Option<Uuid>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub quantity: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedPrice {
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub discount_percent: Option<Decimal>,
    pub on_sale: bool,
    pub region_id: Option<Uuid>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivePriceListOption {
    pub id: Uuid,
    pub name: String,
    pub list_type: String,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub rule_kind: Option<String>,
    pub adjustment_percent: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PriceAdjustmentKind {
    PercentageDiscount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceAdjustmentPreview {
    pub kind: PriceAdjustmentKind,
    pub currency_code: String,
    pub current_amount: Decimal,
    pub base_amount: Decimal,
    pub adjustment_percent: Decimal,
    pub adjusted_amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PriceListRuleKind {
    PercentageDiscount,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceListRule {
    pub kind: PriceListRuleKind,
    pub adjustment_percent: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingProductList {
    pub items: Vec<AdminPricingProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingProductListItem {
    pub id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub title: String,
    pub handle: String,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingProductDetail {
    pub id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub translations: Vec<AdminPricingProductTranslation>,
    pub variants: Vec<AdminPricingVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingVariant {
    pub id: Uuid,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub title: String,
    pub option1: Option<String>,
    pub option2: Option<String>,
    pub option3: Option<String>,
    pub prices: Vec<AdminPricingPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdminPricingPrice {
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub discount_percent: Option<Decimal>,
    pub on_sale: bool,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub min_quantity: Option<i32>,
    pub max_quantity: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingProductList {
    pub items: Vec<StorefrontPricingProductListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
    pub has_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingProductListItem {
    pub id: Uuid,
    pub title: String,
    pub handle: String,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub variant_count: u64,
    pub sale_variant_count: u64,
    pub currencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingProductDetail {
    pub id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub published_at: Option<chrono::DateTime<chrono::Utc>>,
    pub translations: Vec<StorefrontPricingProductTranslation>,
    pub variants: Vec<StorefrontPricingVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingProductTranslation {
    pub locale: String,
    pub title: String,
    pub handle: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingVariant {
    pub id: Uuid,
    pub title: String,
    pub sku: Option<String>,
    pub prices: Vec<StorefrontPricingPrice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorefrontPricingPrice {
    pub currency_code: String,
    pub amount: Decimal,
    pub compare_at_amount: Option<Decimal>,
    pub discount_percent: Option<Decimal>,
    pub on_sale: bool,
}

pub struct PricingService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl PricingService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    #[instrument(skip(self))]
    pub async fn set_price_list_percentage_rule(
        &self,
        tenant_id: Uuid,
        _actor_id: Uuid,
        price_list_id: Uuid,
        adjustment_percent: Option<Decimal>,
    ) -> CommerceResult<Option<PriceListRule>> {
        let mut price_list = entities::price_list::Entity::find_by_id(price_list_id)
            .filter(entities::price_list::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))?;

        if let Some(percent) = adjustment_percent {
            validate_discount_percent(percent)?;
            let mut active: entities::price_list::ActiveModel = price_list.into();
            active.rule_kind = Set(Some("percentage_discount".to_string()));
            active.adjustment_percent = Set(Some(percent));
            price_list = active.update(&self.db).await?;
        } else {
            let mut active: entities::price_list::ActiveModel = price_list.into();
            active.rule_kind = Set(None);
            active.adjustment_percent = Set(None);
            price_list = active.update(&self.db).await?;
        }

        Ok(price_list_rule_from_model(&price_list))
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
        let txn = self.db.begin().await?;
        let price_list = resolve_active_price_list_tx(&txn, tenant_id, price_list_id).await?;
        let channel_slug = normalize_channel_slug(channel_slug.as_deref());

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
    pub async fn preview_percentage_discount(
        &self,
        variant_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        self.preview_percentage_discount_with_channel(
            variant_id,
            currency_code,
            discount_percent,
            None,
            None,
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn preview_percentage_discount_with_channel(
        &self,
        variant_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        self.preview_scoped_percentage_discount(
            variant_id,
            currency_code,
            discount_percent,
            None,
            channel_id,
            channel_slug,
        )
        .await
    }

    #[instrument(skip(self))]
    pub async fn preview_price_list_percentage_discount(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        price_list_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        self.preview_price_list_percentage_discount_with_channel(
            tenant_id,
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
    pub async fn preview_price_list_percentage_discount_with_channel(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        price_list_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        let channel_slug = normalize_channel_slug(channel_slug.as_deref());
        let active_price_list_id = resolve_requested_price_list_id(
            &self.db,
            tenant_id,
            Some(price_list_id),
            channel_id,
            channel_slug.as_deref(),
        )
        .await?;

        self.preview_scoped_percentage_discount(
            variant_id,
            currency_code,
            discount_percent,
            active_price_list_id,
            channel_id,
            channel_slug,
        )
        .await
    }

    #[instrument(skip(self))]
    async fn preview_scoped_percentage_discount(
        &self,
        variant_id: Uuid,
        currency_code: &str,
        discount_percent: Decimal,
        price_list_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<String>,
    ) -> CommerceResult<PriceAdjustmentPreview> {
        validate_discount_percent(discount_percent)?;
        let price = self
            .find_canonical_price_row(
                variant_id,
                currency_code,
                price_list_id,
                channel_id,
                channel_slug.as_deref(),
            )
            .await?;
        let base_amount = price.compare_at_amount.unwrap_or(price.amount);
        let adjusted_amount = (base_amount
            * ((Decimal::from(100) - discount_percent) / Decimal::from(100)))
        .round_dp(2);

        Ok(PriceAdjustmentPreview {
            kind: PriceAdjustmentKind::PercentageDiscount,
            currency_code: price.currency_code,
            current_amount: price.amount,
            base_amount,
            adjustment_percent: discount_percent,
            adjusted_amount,
            compare_at_amount: Some(base_amount),
            price_list_id: price.price_list_id,
            channel_id: price.channel_id,
            channel_slug: price.channel_slug,
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
        let txn = self.db.begin().await?;
        let channel_slug = normalize_channel_slug(channel_slug.as_deref());

        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        if amount < Decimal::ZERO {
            return Err(CommerceError::InvalidPrice(
                "Amount cannot be negative".into(),
            ));
        }
        validate_price_tier_quantities(min_quantity, max_quantity)?;
        if let Some(compare_at) = compare_at_amount {
            if compare_at < amount {
                return Err(CommerceError::InvalidPrice(
                    "Compare at price must be greater than amount".into(),
                ));
            }
        }

        let existing = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .filter(entities::price::Column::CurrencyCode.eq(currency_code))
            .filter(entities::price::Column::RegionId.is_null())
            .filter(optional_uuid_filter(
                entities::price::Column::PriceListId,
                price_list_id,
            ))
            .filter(optional_uuid_filter(
                entities::price::Column::ChannelId,
                channel_id,
            ))
            .filter(optional_string_filter(
                entities::price::Column::ChannelSlug,
                channel_slug.clone(),
            ))
            .filter(optional_int_filter(
                entities::price::Column::MinQuantity,
                min_quantity,
            ))
            .filter(optional_int_filter(
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
                price_active.legacy_amount = Set(decimal_to_cents(amount));
                price_active.legacy_compare_at_amount =
                    Set(compare_at_amount.and_then(decimal_to_cents));
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
                    currency_code: Set(currency_code.to_string()),
                    region_id: Set(None),
                    amount: Set(amount),
                    compare_at_amount: Set(compare_at_amount),
                    legacy_amount: Set(decimal_to_cents(amount)),
                    legacy_compare_at_amount: Set(compare_at_amount.and_then(decimal_to_cents)),
                    min_quantity: Set(min_quantity),
                    max_quantity: Set(max_quantity),
                };
                price.insert(&txn).await?;
            }
        }

        let old_cents = old_amount.and_then(decimal_to_cents);
        let new_cents = decimal_to_cents(amount).unwrap_or(0);

        let event = DomainEvent::PriceUpdated {
            variant_id,
            product_id: variant.product_id,
            currency: currency_code.to_string(),
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
        let txn = self.db.begin().await?;

        let variant = entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        for price_input in &prices {
            if price_input.amount < Decimal::ZERO {
                return Err(CommerceError::InvalidPrice(
                    "Amount cannot be negative".into(),
                ));
            }
            if let Some(compare_at) = price_input.compare_at_amount {
                if compare_at < price_input.amount {
                    return Err(CommerceError::InvalidPrice(
                        "Compare at price must be greater than amount".into(),
                    ));
                }
            }

            let existing = entities::price::Entity::find()
                .filter(entities::price::Column::VariantId.eq(variant_id))
                .filter(entities::price::Column::CurrencyCode.eq(&price_input.currency_code))
                .filter(entities::price::Column::RegionId.is_null())
                .filter(entities::price::Column::PriceListId.is_null())
                .filter(optional_uuid_filter(
                    entities::price::Column::ChannelId,
                    price_input.channel_id,
                ))
                .filter(optional_string_filter(
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
                    price_active.legacy_amount = Set(decimal_to_cents(price_input.amount));
                    price_active.legacy_compare_at_amount =
                        Set(price_input.compare_at_amount.and_then(decimal_to_cents));
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
                        currency_code: Set(price_input.currency_code.clone()),
                        region_id: Set(None),
                        amount: Set(price_input.amount),
                        compare_at_amount: Set(price_input.compare_at_amount),
                        legacy_amount: Set(decimal_to_cents(price_input.amount)),
                        legacy_compare_at_amount: Set(price_input
                            .compare_at_amount
                            .and_then(decimal_to_cents)),
                        min_quantity: Set(None),
                        max_quantity: Set(None),
                    };
                    price.insert(&txn).await?;
                }
            }

            let old_cents = old_amount.and_then(decimal_to_cents);
            let new_cents = decimal_to_cents(price_input.amount).unwrap_or(0);

            let event = DomainEvent::PriceUpdated {
                variant_id,
                product_id: variant.product_id,
                currency: price_input.currency_code.clone(),
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
    pub async fn get_price(
        &self,
        variant_id: Uuid,
        currency_code: &str,
    ) -> CommerceResult<Option<Decimal>> {
        let price = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .filter(entities::price::Column::CurrencyCode.eq(currency_code))
            .filter(entities::price::Column::RegionId.is_null())
            .filter(entities::price::Column::PriceListId.is_null())
            .filter(entities::price::Column::MinQuantity.is_null())
            .filter(entities::price::Column::MaxQuantity.is_null())
            .one(&self.db)
            .await?;

        Ok(price.map(|price| price.amount))
    }

    #[instrument(skip(self))]
    pub async fn get_variant_prices(
        &self,
        variant_id: Uuid,
    ) -> CommerceResult<Vec<entities::price::Model>> {
        let prices = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .all(&self.db)
            .await?;

        Ok(prices)
    }

    #[instrument(skip(self, context), fields(tenant_id = %tenant_id, variant_id = %variant_id))]
    pub async fn resolve_variant_price(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        context: PriceResolutionContext,
    ) -> CommerceResult<Option<ResolvedPrice>> {
        let quantity = normalize_resolution_quantity(context.quantity)?;
        let currency_code = normalize_resolution_currency(&context.currency_code)?;
        let channel_slug = normalize_channel_slug(context.channel_slug.as_deref());
        let active_price_list_id = resolve_requested_price_list_id(
            &self.db,
            tenant_id,
            context.price_list_id,
            context.channel_id,
            channel_slug.as_deref(),
        )
        .await?;
        let active_price_list_rule =
            resolve_price_list_rule(&self.db, tenant_id, active_price_list_id).await?;

        entities::product_variant::Entity::find_by_id(variant_id)
            .filter(entities::product_variant::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(CommerceError::VariantNotFound(variant_id))?;

        let prices = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .filter(entities::price::Column::CurrencyCode.eq(&currency_code))
            .all(&self.db)
            .await?;

        Ok(select_best_price(
            prices,
            context.region_id,
            active_price_list_id,
            context.channel_id,
            channel_slug.as_deref(),
            quantity,
        )
        .map(|price| {
            if price.price_list_id.is_none() {
                if let (Some(price_list_id), Some(rule)) =
                    (active_price_list_id, active_price_list_rule.as_ref())
                {
                    return apply_price_list_rule_to_resolved_price(
                        currency_code,
                        price,
                        price_list_id,
                        rule,
                    );
                }
            }

            ResolvedPrice {
                currency_code,
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
            }
        }))
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn list_active_price_lists(
        &self,
        tenant_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> CommerceResult<Vec<ActivePriceListOption>> {
        self.list_active_price_lists_for_channel(
            tenant_id,
            None,
            None,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, channel_id = ?channel_id, channel_slug = ?channel_slug))]
    pub async fn list_active_price_lists_for_channel(
        &self,
        tenant_id: Uuid,
        channel_id: Option<Uuid>,
        channel_slug: Option<&str>,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> CommerceResult<Vec<ActivePriceListOption>> {
        let now = chrono::Utc::now();
        let channel_slug = normalize_channel_slug(channel_slug);
        let price_lists = entities::price_list::Entity::find()
            .filter(entities::price_list::Column::TenantId.eq(tenant_id))
            .order_by_desc(entities::price_list::Column::UpdatedAt)
            .all(&self.db)
            .await?;
        let translations = if price_lists.is_empty() {
            Vec::new()
        } else {
            let ids = price_lists
                .iter()
                .map(|price_list| price_list.id)
                .collect::<Vec<_>>();
            entities::price_list_translation::Entity::find()
                .filter(entities::price_list_translation::Column::PriceListId.is_in(ids))
                .all(&self.db)
                .await?
        };
        let mut translations_by_list: HashMap<Uuid, Vec<entities::price_list_translation::Model>> =
            HashMap::new();
        for translation in translations {
            translations_by_list
                .entry(translation.price_list_id)
                .or_default()
                .push(translation);
        }

        Ok(price_lists
            .into_iter()
            .filter(|price_list| price_list.status.eq_ignore_ascii_case("active"))
            .filter(|price_list| {
                price_list
                    .starts_at
                    .as_ref()
                    .is_none_or(|starts_at| *starts_at <= now)
            })
            .filter(|price_list| {
                price_list
                    .ends_at
                    .as_ref()
                    .is_none_or(|ends_at| *ends_at >= now)
            })
            .filter(|price_list| {
                channel_scope_matches(
                    price_list.channel_id,
                    price_list.channel_slug.as_deref(),
                    channel_id,
                    channel_slug.as_deref(),
                )
            })
            .map(|price_list| {
                let translations = translations_by_list
                    .remove(&price_list.id)
                    .unwrap_or_default();
                ActivePriceListOption {
                    id: price_list.id,
                    name: Self::resolve_price_list_name(
                        &translations,
                        requested_locale,
                        tenant_default_locale,
                    ),
                    list_type: price_list.r#type,
                    channel_id: price_list.channel_id,
                    channel_slug: price_list.channel_slug,
                    rule_kind: price_list.rule_kind,
                    adjustment_percent: price_list.adjustment_percent,
                }
            })
            .collect())
    }

    fn resolve_price_list_name(
        translations: &[entities::price_list_translation::Model],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> String {
        if translations.is_empty() {
            return String::new();
        }

        let mut lookup = HashMap::new();
        for translation in translations {
            if let Some(normalized) = normalize_locale_tag(&translation.locale) {
                lookup.insert(normalized, translation);
            }
        }

        if let Some(locale) = requested_locale.and_then(normalize_locale_tag) {
            if let Some(found) = lookup.get(&locale) {
                return found.name.clone();
            }
        }
        if let Some(locale) = tenant_default_locale.and_then(normalize_locale_tag) {
            if let Some(found) = lookup.get(&locale) {
                return found.name.clone();
            }
        }
        translations
            .first()
            .map(|translation| translation.name.clone())
            .unwrap_or_default()
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    #[allow(clippy::too_many_arguments)]
    pub async fn list_admin_product_pricing_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        search: Option<&str>,
        status: Option<ProductStatus>,
        page: u64,
        per_page: u64,
    ) -> CommerceResult<AdminPricingProductList> {
        let catalog = CatalogService::new(self.db.clone(), self.event_bus.clone());
        let fallback_locale = fallback_locale.unwrap_or(locale);
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
        let offset = (page.saturating_sub(1)) * per_page;

        let mut query = entities::product::Entity::find()
            .filter(entities::product::Column::TenantId.eq(tenant_id));

        if let Some(status) = status {
            query = query.filter(entities::product::Column::Status.eq(status));
        }

        if let Some(search) = search.map(str::trim).filter(|value| !value.is_empty()) {
            let matched_ids = entities::product_translation::Entity::find()
                .filter(entities::product_translation::Column::Title.contains(search))
                .all(&self.db)
                .await?
                .into_iter()
                .map(|translation| translation.product_id)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            if matched_ids.is_empty() {
                return Ok(AdminPricingProductList {
                    items: Vec::new(),
                    total: 0,
                    page,
                    per_page,
                    has_next: false,
                });
            }

            query = query.filter(entities::product::Column::Id.is_in(matched_ids));
        }

        let total = query.clone().count(&self.db).await?;
        let products = query
            .order_by_desc(entities::product::Column::CreatedAt)
            .offset(offset)
            .limit(per_page)
            .all(&self.db)
            .await?;

        let mut items = Vec::with_capacity(products.len());
        for product in products {
            let product = catalog
                .get_product_with_locale_fallback(
                    tenant_id,
                    product.id,
                    locale,
                    Some(fallback_locale),
                )
                .await?;
            items.push(map_admin_list_item(product, locale, fallback_locale));
        }

        Ok(AdminPricingProductList {
            items,
            total,
            page,
            per_page,
            has_next: page.saturating_mul(per_page) < total,
        })
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, product_id = %product_id))]
    pub async fn get_admin_product_pricing_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        selected_price_list_id: Option<Uuid>,
    ) -> CommerceResult<AdminPricingProductDetail> {
        let catalog = CatalogService::new(self.db.clone(), self.event_bus.clone());
        let product = catalog
            .get_product_with_locale_fallback(
                tenant_id,
                product_id,
                locale,
                Some(fallback_locale.unwrap_or(locale)),
            )
            .await?;
        let variant_ids = product
            .variants
            .iter()
            .map(|variant| variant.id)
            .collect::<Vec<_>>();
        let mut prices_by_variant = HashMap::<Uuid, Vec<entities::price::Model>>::new();
        if !variant_ids.is_empty() {
            let mut query = entities::price::Entity::find()
                .filter(entities::price::Column::VariantId.is_in(variant_ids));
            if let Some(selected_price_list_id) = selected_price_list_id {
                query = query.filter(
                    sea_orm::Condition::any()
                        .add(entities::price::Column::PriceListId.is_null())
                        .add(entities::price::Column::PriceListId.eq(selected_price_list_id)),
                );
            } else {
                query = query.filter(entities::price::Column::PriceListId.is_null());
            }

            for price in query.all(&self.db).await? {
                prices_by_variant
                    .entry(price.variant_id)
                    .or_default()
                    .push(price);
            }
        }
        for prices in prices_by_variant.values_mut() {
            prices.sort_by(price_specificity_cmp);
        }

        Ok(map_admin_detail(product, prices_by_variant))
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

    async fn find_canonical_price_row(
        &self,
        variant_id: Uuid,
        currency_code: &str,
        price_list_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<&str>,
    ) -> CommerceResult<entities::price::Model> {
        entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .filter(entities::price::Column::CurrencyCode.eq(currency_code))
            .filter(entities::price::Column::RegionId.is_null())
            .filter(optional_uuid_filter(
                entities::price::Column::PriceListId,
                price_list_id,
            ))
            .filter(optional_uuid_filter(
                entities::price::Column::ChannelId,
                channel_id,
            ))
            .filter(optional_string_filter(
                entities::price::Column::ChannelSlug,
                normalize_channel_slug(channel_slug),
            ))
            .filter(entities::price::Column::MinQuantity.is_null())
            .filter(entities::price::Column::MaxQuantity.is_null())
            .one(&self.db)
            .await?
            .ok_or_else(|| {
                CommerceError::InvalidPrice(format!(
                    "No canonical price found for currency {}{}{}",
                    currency_code,
                    price_list_id
                        .map(|id| format!(" and price_list_id {id}"))
                        .unwrap_or_default(),
                    channel_slug
                        .map(|slug| format!(" and channel_slug {slug}"))
                        .unwrap_or_default(),
                ))
            })
    }

    #[instrument(skip(self))]
    pub async fn list_published_product_pricing_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
        page: u64,
        per_page: u64,
    ) -> CommerceResult<StorefrontPricingProductList> {
        let catalog = CatalogService::new(self.db.clone(), self.event_bus.clone());
        let products = catalog
            .list_published_products_with_locale_fallback(
                tenant_id,
                locale,
                fallback_locale,
                public_channel_slug,
                page,
                per_page,
            )
            .await?;
        let product_ids = products
            .items
            .iter()
            .map(|product| product.id)
            .collect::<Vec<_>>();
        let variants = if product_ids.is_empty() {
            Vec::new()
        } else {
            entities::product_variant::Entity::find()
                .filter(entities::product_variant::Column::ProductId.is_in(product_ids.clone()))
                .all(&self.db)
                .await?
        };
        let mut variant_counts_by_product = HashMap::<Uuid, u64>::new();
        let mut variant_to_product = HashMap::<Uuid, Uuid>::new();
        for variant in variants {
            variant_to_product.insert(variant.id, variant.product_id);
            *variant_counts_by_product
                .entry(variant.product_id)
                .or_insert(0) += 1;
        }
        let variant_ids = variant_to_product.keys().copied().collect::<Vec<_>>();
        let prices = if variant_ids.is_empty() {
            Vec::new()
        } else {
            entities::price::Entity::find()
                .filter(entities::price::Column::VariantId.is_in(variant_ids))
                .all(&self.db)
                .await?
        };
        let mut currencies_by_product = HashMap::<Uuid, BTreeSet<String>>::new();
        let mut sale_variants_by_product = HashMap::<Uuid, BTreeSet<Uuid>>::new();
        for price in prices {
            let Some(product_id) = variant_to_product.get(&price.variant_id).copied() else {
                continue;
            };
            currencies_by_product
                .entry(product_id)
                .or_default()
                .insert(price.currency_code);
            if price
                .compare_at_amount
                .map(|compare| compare > price.amount)
                .unwrap_or(false)
            {
                sale_variants_by_product
                    .entry(product_id)
                    .or_default()
                    .insert(price.variant_id);
            }
        }

        Ok(StorefrontPricingProductList {
            items: products
                .items
                .into_iter()
                .map(|product| StorefrontPricingProductListItem {
                    id: product.id,
                    title: product.title,
                    handle: product.handle,
                    seller_id: product.seller_id,
                    vendor: product.vendor,
                    product_type: product.product_type,
                    created_at: product.created_at,
                    published_at: product.published_at,
                    variant_count: variant_counts_by_product.remove(&product.id).unwrap_or(0),
                    sale_variant_count: sale_variants_by_product
                        .remove(&product.id)
                        .map(|variants| variants.len() as u64)
                        .unwrap_or(0),
                    currencies: currencies_by_product
                        .remove(&product.id)
                        .unwrap_or_default()
                        .into_iter()
                        .collect(),
                })
                .collect(),
            total: products.total,
            page: products.page,
            per_page: products.per_page,
            has_next: products.has_next,
        })
    }

    #[instrument(skip(self))]
    pub async fn get_published_product_pricing_by_handle_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        handle: &str,
        locale: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> CommerceResult<Option<StorefrontPricingProductDetail>> {
        let catalog = CatalogService::new(self.db.clone(), self.event_bus.clone());
        let product = catalog
            .get_published_product_by_handle_with_locale_fallback(
                tenant_id,
                handle,
                locale,
                fallback_locale,
                public_channel_slug,
            )
            .await?;

        Ok(product.map(map_product_detail))
    }
}

fn decimal_to_cents(amount: Decimal) -> Option<i64> {
    (amount * Decimal::from(100)).round_dp(0).to_i64()
}

#[allow(clippy::result_large_err)]
fn normalize_resolution_currency(currency_code: &str) -> CommerceResult<String> {
    let normalized = currency_code.trim().to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(CommerceError::Validation(
            "currency_code must be a 3-letter code".to_string(),
        ));
    }
    Ok(normalized)
}

fn normalize_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
}

#[allow(clippy::result_large_err)]
fn normalize_resolution_quantity(quantity: Option<i32>) -> CommerceResult<i32> {
    match quantity {
        Some(value) if value < 1 => Err(CommerceError::Validation(
            "quantity must be at least 1".to_string(),
        )),
        Some(value) => Ok(value),
        None => Ok(1),
    }
}

fn channel_scope_matches(
    scoped_channel_id: Option<Uuid>,
    scoped_channel_slug: Option<&str>,
    requested_channel_id: Option<Uuid>,
    requested_channel_slug: Option<&str>,
) -> bool {
    let scoped_channel_slug = normalize_channel_slug(scoped_channel_slug);
    let requested_channel_slug = normalize_channel_slug(requested_channel_slug);

    if scoped_channel_id.is_none() && scoped_channel_slug.is_none() {
        return true;
    }

    if let Some(scoped_channel_id) = scoped_channel_id {
        if Some(scoped_channel_id) != requested_channel_id {
            return false;
        }
    }

    if let Some(scoped_channel_slug) = scoped_channel_slug {
        if Some(scoped_channel_slug.as_str()) != requested_channel_slug.as_deref() {
            return false;
        }
    }

    true
}

fn channel_specificity(
    scoped_channel_id: Option<Uuid>,
    scoped_channel_slug: Option<&str>,
    requested_channel_id: Option<Uuid>,
    requested_channel_slug: Option<&str>,
) -> i32 {
    let scoped_channel_slug = normalize_channel_slug(scoped_channel_slug);
    let requested_channel_slug = normalize_channel_slug(requested_channel_slug);

    if scoped_channel_id.is_some() && scoped_channel_id == requested_channel_id {
        0
    } else if scoped_channel_slug.is_some()
        && scoped_channel_slug.as_deref() == requested_channel_slug.as_deref()
    {
        1
    } else {
        2
    }
}

async fn resolve_requested_price_list_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    let Some(price_list_id) = price_list_id else {
        return Ok(None);
    };

    let price_list = resolve_active_price_list(db, tenant_id, price_list_id).await?;

    if !channel_scope_matches(
        price_list.channel_id,
        price_list.channel_slug.as_deref(),
        channel_id,
        channel_slug,
    ) {
        return Err(CommerceError::Validation(
            "price_list_id is not available for the requested channel".to_string(),
        ));
    }

    Ok(Some(price_list_id))
}

async fn resolve_active_price_list(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> CommerceResult<entities::price_list::Model> {
    let price_list = entities::price_list::Entity::find_by_id(price_list_id)
        .filter(entities::price_list::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))?;

    validate_active_price_list(&price_list)?;
    Ok(price_list)
}

async fn resolve_active_price_list_tx<C>(
    db: &C,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> CommerceResult<entities::price_list::Model>
where
    C: sea_orm::ConnectionTrait,
{
    let price_list = entities::price_list::Entity::find_by_id(price_list_id)
        .filter(entities::price_list::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))?;

    validate_active_price_list(&price_list)?;
    Ok(price_list)
}

#[allow(clippy::result_large_err)]
fn validate_active_price_list(price_list: &entities::price_list::Model) -> CommerceResult<()> {
    if !price_list.status.eq_ignore_ascii_case("active") {
        return Err(CommerceError::Validation(
            "price_list_id must reference an active price list".to_string(),
        ));
    }

    let now = chrono::Utc::now();
    if price_list
        .starts_at
        .as_ref()
        .is_some_and(|starts_at| *starts_at > now)
    {
        return Err(CommerceError::Validation(
            "price_list_id is not active yet".to_string(),
        ));
    }
    if price_list
        .ends_at
        .as_ref()
        .is_some_and(|ends_at| *ends_at < now)
    {
        return Err(CommerceError::Validation(
            "price_list_id is already expired".to_string(),
        ));
    }

    Ok(())
}

#[allow(clippy::result_large_err)]
fn validate_or_inherit_price_list_scope(
    price_list: &entities::price_list::Model,
    channel_id: Option<Uuid>,
    channel_slug: Option<String>,
) -> CommerceResult<(Option<Uuid>, Option<String>)> {
    let channel_slug = normalize_channel_slug(channel_slug.as_deref());

    if channel_id.is_none() && channel_slug.is_none() {
        return Ok((price_list.channel_id, price_list.channel_slug.clone()));
    }

    if channel_id != price_list.channel_id
        || channel_slug.as_deref()
            != normalize_channel_slug(price_list.channel_slug.as_deref()).as_deref()
    {
        return Err(CommerceError::Validation(
            "price rows for a selected price_list_id must match the price list channel scope"
                .to_string(),
        ));
    }

    Ok((channel_id, channel_slug))
}

fn price_matches_context(
    price: &entities::price::Model,
    region_id: Option<Uuid>,
    requested_price_list_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
    quantity: i32,
) -> bool {
    match requested_price_list_id {
        Some(requested_price_list_id) => {
            if price.price_list_id.is_some() && price.price_list_id != Some(requested_price_list_id)
            {
                return false;
            }
        }
        None => {
            if price.price_list_id.is_some() {
                return false;
            }
        }
    }

    if let Some(requested_region_id) = region_id {
        if price.region_id.is_some() && price.region_id != Some(requested_region_id) {
            return false;
        }
    } else if price.region_id.is_some() {
        return false;
    }

    if !channel_scope_matches(
        price.channel_id,
        price.channel_slug.as_deref(),
        channel_id,
        channel_slug,
    ) {
        return false;
    }

    if let Some(min_quantity) = price.min_quantity {
        if quantity < min_quantity {
            return false;
        }
    }

    if let Some(max_quantity) = price.max_quantity {
        if quantity > max_quantity {
            return false;
        }
    }

    true
}

fn select_best_price(
    prices: Vec<entities::price::Model>,
    region_id: Option<Uuid>,
    requested_price_list_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
    quantity: i32,
) -> Option<entities::price::Model> {
    let mut candidates = prices
        .into_iter()
        .filter(|price| {
            price_matches_context(
                price,
                region_id,
                requested_price_list_id,
                channel_id,
                channel_slug,
                quantity,
            )
        })
        .collect::<Vec<_>>();

    candidates.sort_by_key(|price| {
        let price_list_specificity = match (requested_price_list_id, price.price_list_id) {
            (Some(requested), Some(candidate)) if requested == candidate => 0,
            (Some(_), None) => 1,
            _ => 2,
        };
        let region_specificity = match (region_id, price.region_id) {
            (Some(requested), Some(candidate)) if requested == candidate => 0,
            (_, None) => 1,
            _ => 2,
        };
        let channel_specificity = channel_specificity(
            price.channel_id,
            price.channel_slug.as_deref(),
            channel_id,
            channel_slug,
        );
        let min_quantity_specificity = std::cmp::Reverse(price.min_quantity.unwrap_or(0));
        let max_quantity_specificity = price.max_quantity.unwrap_or(i32::MAX);
        (
            price_list_specificity,
            channel_specificity,
            region_specificity,
            min_quantity_specificity,
            max_quantity_specificity,
            price.id,
        )
    });

    candidates.into_iter().next()
}

fn price_list_rule_from_model(price_list: &entities::price_list::Model) -> Option<PriceListRule> {
    match (
        price_list.rule_kind.as_deref(),
        price_list.adjustment_percent,
    ) {
        (Some("percentage_discount"), Some(adjustment_percent)) => Some(PriceListRule {
            kind: PriceListRuleKind::PercentageDiscount,
            adjustment_percent,
        }),
        _ => None,
    }
}

async fn resolve_price_list_rule(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Option<Uuid>,
) -> CommerceResult<Option<PriceListRule>> {
    let Some(price_list_id) = price_list_id else {
        return Ok(None);
    };

    let price_list = entities::price_list::Entity::find_by_id(price_list_id)
        .filter(entities::price_list::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))?;

    Ok(price_list_rule_from_model(&price_list))
}

fn apply_price_list_rule_to_resolved_price(
    currency_code: String,
    price: entities::price::Model,
    price_list_id: Uuid,
    rule: &PriceListRule,
) -> ResolvedPrice {
    match rule.kind {
        PriceListRuleKind::PercentageDiscount => {
            let base_amount = price.compare_at_amount.unwrap_or(price.amount);
            let amount = (base_amount
                * ((Decimal::from(100) - rule.adjustment_percent) / Decimal::from(100)))
            .round_dp(2);

            ResolvedPrice {
                currency_code,
                amount,
                compare_at_amount: Some(base_amount),
                discount_percent: Some(rule.adjustment_percent),
                on_sale: true,
                region_id: price.region_id,
                min_quantity: price.min_quantity,
                max_quantity: price.max_quantity,
                price_list_id: Some(price_list_id),
                channel_id: price.channel_id,
                channel_slug: price.channel_slug,
            }
        }
    }
}

fn map_product_detail(
    product: rustok_commerce_foundation::dto::ProductResponse,
) -> StorefrontPricingProductDetail {
    StorefrontPricingProductDetail {
        id: product.id,
        status: product.status,
        seller_id: product.seller_id,
        vendor: product.vendor,
        product_type: product.product_type,
        published_at: product.published_at,
        translations: product
            .translations
            .into_iter()
            .map(|translation| StorefrontPricingProductTranslation {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
        variants: product
            .variants
            .into_iter()
            .map(|variant| StorefrontPricingVariant {
                id: variant.id,
                title: variant.title,
                sku: variant.sku,
                prices: variant
                    .prices
                    .into_iter()
                    .map(|price| StorefrontPricingPrice {
                        currency_code: price.currency_code,
                        amount: price.amount,
                        compare_at_amount: price.compare_at_amount,
                        discount_percent: calculate_discount_percent(
                            price.amount,
                            price.compare_at_amount,
                        ),
                        on_sale: price.on_sale,
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn map_admin_list_item(
    product: rustok_commerce_foundation::dto::ProductResponse,
    locale: &str,
    fallback_locale: &str,
) -> AdminPricingProductListItem {
    let translation =
        pick_product_translation(product.translations.as_slice(), locale, fallback_locale);

    AdminPricingProductListItem {
        id: product.id,
        status: product.status,
        seller_id: product.seller_id,
        title: translation
            .map(|translation| translation.title.clone())
            .unwrap_or_else(|| "Untitled".to_string()),
        handle: translation
            .map(|translation| translation.handle.clone())
            .unwrap_or_default(),
        vendor: product.vendor,
        product_type: product.product_type,
        shipping_profile_slug: product.shipping_profile_slug,
        tags: product.tags,
        created_at: product.created_at,
        published_at: product.published_at,
    }
}

fn map_admin_detail(
    product: rustok_commerce_foundation::dto::ProductResponse,
    mut prices_by_variant: HashMap<Uuid, Vec<entities::price::Model>>,
) -> AdminPricingProductDetail {
    AdminPricingProductDetail {
        id: product.id,
        status: product.status,
        seller_id: product.seller_id,
        vendor: product.vendor,
        product_type: product.product_type,
        shipping_profile_slug: product.shipping_profile_slug,
        created_at: product.created_at,
        updated_at: product.updated_at,
        published_at: product.published_at,
        translations: product
            .translations
            .into_iter()
            .map(|translation| AdminPricingProductTranslation {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
        variants: product
            .variants
            .into_iter()
            .map(|variant| AdminPricingVariant {
                id: variant.id,
                sku: variant.sku,
                barcode: variant.barcode,
                shipping_profile_slug: variant.shipping_profile_slug,
                title: variant.title,
                option1: variant.option1,
                option2: variant.option2,
                option3: variant.option3,
                prices: prices_by_variant
                    .remove(&variant.id)
                    .map(|prices| {
                        prices
                            .into_iter()
                            .map(|price| AdminPricingPrice {
                                currency_code: price.currency_code,
                                amount: price.amount,
                                compare_at_amount: price.compare_at_amount,
                                discount_percent: calculate_discount_percent(
                                    price.amount,
                                    price.compare_at_amount,
                                ),
                                on_sale: is_sale_price(price.amount, price.compare_at_amount),
                                price_list_id: price.price_list_id,
                                channel_id: price.channel_id,
                                channel_slug: price.channel_slug,
                                min_quantity: price.min_quantity,
                                max_quantity: price.max_quantity,
                            })
                            .collect()
                    })
                    .unwrap_or_else(|| {
                        variant
                            .prices
                            .into_iter()
                            .map(|price| AdminPricingPrice {
                                currency_code: price.currency_code,
                                amount: price.amount,
                                compare_at_amount: price.compare_at_amount,
                                discount_percent: calculate_discount_percent(
                                    price.amount,
                                    price.compare_at_amount,
                                ),
                                on_sale: price.on_sale,
                                price_list_id: None,
                                channel_id: None,
                                channel_slug: None,
                                min_quantity: None,
                                max_quantity: None,
                            })
                            .collect()
                    }),
            })
            .collect(),
    }
}

#[allow(clippy::result_large_err)]
fn validate_price_tier_quantities(
    min_quantity: Option<i32>,
    max_quantity: Option<i32>,
) -> CommerceResult<()> {
    if let Some(min_quantity) = min_quantity {
        if min_quantity <= 0 {
            return Err(CommerceError::InvalidPrice(
                "Minimum quantity must be positive".into(),
            ));
        }
    }

    if let Some(max_quantity) = max_quantity {
        if max_quantity <= 0 {
            return Err(CommerceError::InvalidPrice(
                "Maximum quantity must be positive".into(),
            ));
        }
    }

    if let (Some(min_quantity), Some(max_quantity)) = (min_quantity, max_quantity) {
        if max_quantity < min_quantity {
            return Err(CommerceError::InvalidPrice(
                "Maximum quantity must be greater than or equal to minimum quantity".into(),
            ));
        }
    }

    Ok(())
}

#[allow(clippy::result_large_err)]
fn validate_discount_percent(discount_percent: Decimal) -> CommerceResult<()> {
    if discount_percent <= Decimal::ZERO || discount_percent > Decimal::from(100) {
        return Err(CommerceError::InvalidPrice(
            "discount_percent must be greater than 0 and at most 100".into(),
        ));
    }

    Ok(())
}

fn optional_int_filter(column: entities::price::Column, value: Option<i32>) -> sea_orm::Condition {
    match value {
        Some(value) => sea_orm::Condition::all().add(column.eq(value)),
        None => sea_orm::Condition::all().add(column.is_null()),
    }
}

fn optional_uuid_filter(
    column: entities::price::Column,
    value: Option<Uuid>,
) -> sea_orm::Condition {
    match value {
        Some(value) => sea_orm::Condition::all().add(column.eq(value)),
        None => sea_orm::Condition::all().add(column.is_null()),
    }
}

fn optional_string_filter(
    column: entities::price::Column,
    value: Option<String>,
) -> sea_orm::Condition {
    match value {
        Some(value) => sea_orm::Condition::all().add(column.eq(value)),
        None => sea_orm::Condition::all().add(column.is_null()),
    }
}

fn is_sale_price(amount: Decimal, compare_at_amount: Option<Decimal>) -> bool {
    compare_at_amount
        .filter(|compare_at| *compare_at > Decimal::ZERO)
        .map(|compare_at| compare_at > amount)
        .unwrap_or(false)
}

fn calculate_discount_percent(
    amount: Decimal,
    compare_at_amount: Option<Decimal>,
) -> Option<Decimal> {
    let compare_at_amount = compare_at_amount.filter(|compare_at| *compare_at > Decimal::ZERO)?;
    if compare_at_amount <= amount {
        return None;
    }

    Some((((compare_at_amount - amount) / compare_at_amount) * Decimal::from(100)).round_dp(2))
}

fn price_specificity_cmp(
    left: &entities::price::Model,
    right: &entities::price::Model,
) -> std::cmp::Ordering {
    (
        left.currency_code.as_str(),
        left.price_list_id.is_some(),
        left.min_quantity.unwrap_or(0),
        left.max_quantity.unwrap_or(i32::MAX),
    )
        .cmp(&(
            right.currency_code.as_str(),
            right.price_list_id.is_some(),
            right.min_quantity.unwrap_or(0),
            right.max_quantity.unwrap_or(i32::MAX),
        ))
}

fn pick_product_translation<'a>(
    translations: &'a [rustok_commerce_foundation::dto::ProductTranslationResponse],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a rustok_commerce_foundation::dto::ProductTranslationResponse> {
    translations
        .iter()
        .find(|translation| locale_tags_match(&translation.locale, locale))
        .or_else(|| {
            (!locale_tags_match(fallback_locale, locale)).then(|| {
                translations
                    .iter()
                    .find(|translation| locale_tags_match(&translation.locale, fallback_locale))
            })?
        })
        .or_else(|| translations.first())
}
