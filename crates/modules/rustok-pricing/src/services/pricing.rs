use rust_decimal::Decimal;
use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    AccessMode, ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseBackend,
    DatabaseConnection, DatabaseTransaction, EntityTrait, IsolationLevel, QueryFilter, QueryOrder,
    QuerySelect, Set, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{PortContext, PortError, PortErrorKind, locale_tags_match, normalize_locale_tag};
use rustok_channel::entities::channel;
use rustok_core::events::ValidateEvent;
use rustok_core::{generate_id, money};
use rustok_events::DomainEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_product::dto::{ProductResponse, ProductTranslationResponse};

use rustok_commerce_foundation::dto::PriceInput;
use rustok_commerce_foundation::entities::product::ProductStatus;
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};

mod entities {
    pub use rustok_commerce_foundation::entities::product_variant;
    pub use rustok_pricing_persistence::entities::{price, price_list, price_list_translation};
}

mod resolution;
pub(crate) use resolution::ResolutionError;

pub const MAX_PRICE_RESOLUTION_ROWS: usize = 4096;
const MAX_PUBLIC_PRICE_CATALOG_ROWS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceResolutionContext {
    pub currency_code: String,
    pub region_id: Option<Uuid>,
    pub price_list_id: Option<Uuid>,
    pub channel_id: Option<Uuid>,
    pub channel_slug: Option<String>,
    pub quantity: Option<i32>,
}

impl PriceResolutionContext {
    /// Syntactic/money normalization only. Identity and channel completion stay in owner ports.
    #[allow(clippy::result_large_err)]
    pub fn normalized(mut self) -> CommerceResult<Self> {
        self.currency_code = normalize_resolution_currency(&self.currency_code)?;
        self.quantity = Some(normalize_resolution_quantity(self.quantity)?);
        for (field, id) in [
            ("region_id", self.region_id),
            ("price_list_id", self.price_list_id),
            ("channel_id", self.channel_id),
        ] {
            if id.is_some_and(|id| id.is_nil()) {
                return Err(CommerceError::Validation(format!(
                    "{field} must not be a nil UUID"
                )));
            }
        }
        if self
            .channel_slug
            .as_deref()
            .is_some_and(|slug| slug.trim().is_empty())
        {
            return Err(CommerceError::Validation(
                "channel_slug must not be empty".into(),
            ));
        }
        self.channel_slug = normalize_channel_slug(self.channel_slug.as_deref());
        Ok(self)
    }
}

/// Provenance is an observation, not a price reservation or an optimistic write token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PriceResolutionSource {
    Base {
        price_id: Uuid,
    },
    PriceListOverride {
        price_id: Uuid,
        price_list_id: Uuid,
    },
    PriceListRule {
        price_id: Uuid,
        price_list_id: Uuid,
        rule: PriceListRule,
    },
}

impl PriceResolutionSource {
    pub fn price_id(&self) -> Uuid {
        match self {
            Self::Base { price_id }
            | Self::PriceListOverride { price_id, .. }
            | Self::PriceListRule { price_id, .. } => *price_id,
        }
    }

    pub fn rule(&self) -> Option<&PriceListRule> {
        match self {
            Self::PriceListRule { rule, .. } => Some(rule),
            Self::Base { .. } | Self::PriceListOverride { .. } => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Base { .. } => "base",
            Self::PriceListOverride { .. } => "price_list_override",
            Self::PriceListRule { .. } => "price_list_rule",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedPrice {
    pub source: PriceResolutionSource,
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

impl PriceListRuleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PercentageDiscount => "percentage_discount",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    pub combination_identity: Option<String>,
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

pub(crate) struct PriceResolutionBatch {
    pub context: PriceResolutionContext,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
    pub prices: Vec<(Uuid, Option<ResolvedPrice>)>,
}

pub struct PricingService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    product_catalog_read_runtime: rustok_product::ProductCatalogReadRuntime,
}

impl PricingService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        // This constructor is an explicit embedded baseline. Mounted reads install their
        // already selected Product runtime before use; failures never select this baseline.
        let product_catalog_read_runtime =
            rustok_product::ProductCatalogReadRuntime::in_process(db.clone(), event_bus.clone());
        Self {
            db,
            event_bus,
            product_catalog_read_runtime,
        }
    }

    pub fn with_product_catalog_read_runtime(
        mut self,
        runtime: rustok_product::ProductCatalogReadRuntime,
    ) -> Self {
        self.product_catalog_read_runtime = runtime;
        self
    }

    #[instrument(skip(self))]
    pub async fn set_price_list_percentage_rule(
        &self,
        tenant_id: Uuid,
        _actor_id: Uuid,
        price_list_id: Uuid,
        adjustment_percent: Option<Decimal>,
    ) -> CommerceResult<Option<PriceListRule>> {
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
        self.require_embedded_variant_service_state()?;
        let currency_code = normalize_resolution_currency(currency_code)?;
        validate_discount_percent(discount_percent)?;
        let price = self
            .find_canonical_price_row(
                variant_id,
                &currency_code,
                price_list_id,
                channel_id,
                channel_slug.as_deref(),
            )
            .await?;
        let base_amount = price.compare_at_amount.unwrap_or(price.amount);
        // ECOM-MONEY-04: a preview must round at the currency's precision, not always to two
        // decimals, so the value it shows is the value the owner can represent.
        let adjusted_amount = money::round_to_currency(
            base_amount * ((Decimal::from(100) - discount_percent) / Decimal::from(100)),
            &currency_code,
        )
        .map_err(|error| {
            CommerceError::Validation(format!(
                "discounted preview for {currency_code} cannot be rounded: {error}"
            ))
        })?;

        Ok(PriceAdjustmentPreview {
            kind: PriceAdjustmentKind::PercentageDiscount,
            currency_code,
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
            .map(admin_pricing_price_from_model)
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
        validate_price_precision(amount, compare_at_amount, &currency_code)?;

        let existing = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .filter(entities::price::Column::CurrencyCode.eq(&currency_code))
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
                price_active.legacy_amount =
                    Set(rustok_pricing_persistence::legacy_amount_units(amount));
                price_active.legacy_compare_at_amount = Set(
                    compare_at_amount.and_then(rustok_pricing_persistence::legacy_amount_units)
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
                    legacy_amount: Set(rustok_pricing_persistence::legacy_amount_units(amount)),
                    legacy_compare_at_amount: Set(
                        compare_at_amount.and_then(rustok_pricing_persistence::legacy_amount_units)
                    ),
                    min_quantity: Set(min_quantity),
                    max_quantity: Set(max_quantity),
                };
                price.insert(&txn).await?;
            }
        }

        let old_cents =
            old_amount.and_then(|amount| money::to_minor_units(amount, &currency_code).ok());
        // The event carries minor units, so the amount must be scaled by the currency exponent and
        // must never be silently replaced by zero.
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
            validate_price_precision(
                price_input.amount,
                price_input.compare_at_amount,
                &normalized_currency,
            )?;

            let existing = entities::price::Entity::find()
                .filter(entities::price::Column::VariantId.eq(variant_id))
                .filter(entities::price::Column::CurrencyCode.eq(&normalized_currency))
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
                    price_active.legacy_amount = Set(
                        rustok_pricing_persistence::legacy_amount_units(price_input.amount),
                    );
                    price_active.legacy_compare_at_amount = Set(price_input
                        .compare_at_amount
                        .and_then(rustok_pricing_persistence::legacy_amount_units));
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
                        legacy_amount: Set(rustok_pricing_persistence::legacy_amount_units(
                            price_input.amount,
                        )),
                        legacy_compare_at_amount: Set(price_input
                            .compare_at_amount
                            .and_then(rustok_pricing_persistence::legacy_amount_units)),
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

    /// Tenant boundary of the price readers below.
    ///
    /// `price` rows carry no tenant column, so the variant row — which does —
    /// owns the scope (AGENTS.md §7): a read addressed by `variant_id` alone
    /// would answer for any tenant that names the identifier, and the caller has
    /// no second check to lean on. `resolve_variant_price` states the same rule
    /// inline; both now share this loader.
    async fn ensure_variant_tenant(&self, tenant_id: Uuid, variant_id: Uuid) -> CommerceResult<()> {
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

    #[instrument(skip(self))]
    pub async fn get_price(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
    ) -> CommerceResult<Option<Decimal>> {
        self.ensure_variant_tenant(tenant_id, variant_id).await?;
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
        tenant_id: Uuid,
        variant_id: Uuid,
    ) -> CommerceResult<Vec<entities::price::Model>> {
        self.ensure_variant_tenant(tenant_id, variant_id).await?;
        let prices = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .all(&self.db)
            .await?;

        Ok(prices)
    }

    /// Owner SDK convenience entry. A real caller context is required; it must not be
    /// reconstructed with a fabricated locale, actor or channel inside Pricing.
    #[instrument(skip(self, context, pricing, variant_id))]
    pub async fn resolve_variant_price(
        &self,
        context: PortContext,
        variant_id: Uuid,
        pricing: PriceResolutionContext,
    ) -> Result<Option<ResolvedPrice>, PortError> {
        let mut batch = crate::PricingReadPort::resolve_product_prices(
            self,
            context,
            crate::ResolveProductPricesRequest {
                product_id: None,
                variant_ids: vec![variant_id],
                pricing,
            },
        )
        .await?;
        let line = batch.items.pop().ok_or_else(|| {
            PortError::invariant_violation(
                "pricing.resolution_incomplete",
                "SKU quote could not be completed safely",
            )
        })?;
        Ok(line.price)
    }

    /// Current SKU mutation/preview helpers lock local Product variant rows. They cannot
    /// safely execute against a differently selected Product owner until that write protocol exists.
    pub(crate) fn require_embedded_variant_state(&self) -> Result<(), PortError> {
        if self.product_catalog_read_runtime.profile()
            != rustok_product::ProductCatalogReadProfile::EmbeddedNative
        {
            return Err(PortError::new(
                rustok_api::PortErrorKind::Unavailable,
                "pricing.product_write_capability_unavailable",
                "SKU price authoring is unavailable for the selected external Product owner",
                false,
            ));
        }
        Ok(())
    }

    fn require_embedded_variant_service_state(&self) -> CommerceResult<()> {
        self.require_embedded_variant_state().map_err(|_| CommerceError::Validation(
            "SKU price authoring requires the embedded Product write protocol; no local fallback is allowed".to_owned()))
    }

    pub(crate) fn variant_identity_read_port(
        &self,
    ) -> Result<std::sync::Arc<dyn rustok_product::ProductVariantIdentityReadPort>, PortError> {
        self.product_catalog_read_runtime
            .variant_identity_read_port()
            .ok_or_else(|| {
                PortError::new(
                    PortErrorKind::Unavailable,
                    "pricing.product_identity_capability_unavailable",
                    "The selected Product owner has not supplied SKU identity verification",
                    false,
                )
            })
    }

    pub(crate) async fn read_product_variant_identities(
        &self,
        context: &PortContext,
        request: &rustok_product::ProductVariantIdentitiesRequest,
    ) -> Result<Vec<rustok_product::ProductVariantIdentitySnapshot>, PortError> {
        let identities = self
            .variant_identity_read_port()?
            .read_variant_identities(context.clone(), request.clone())
            .await
            .map_err(pricing_identity_port_error)?;

        rustok_product::validate_variant_identities_response(context, request, &identities)
            .map_err(|error| {
                PortError::new(
                    error.kind,
                    "pricing.variant_identity_invalid",
                    "The selected Product owner returned inconsistent SKU ownership",
                    error.retryable,
                )
            })?;

        Ok(identities)
    }

    pub(crate) fn channel_read_port(&self) -> std::sync::Arc<dyn rustok_channel::ChannelReadPort> {
        rustok_channel::in_process_channel_read_port(self.db.clone())
    }

    pub(crate) fn region_read_port(&self) -> std::sync::Arc<dyn rustok_region::RegionReadPort> {
        rustok_region::in_process_region_read_port(self.db.clone())
    }

    /// Only called after the Product owner has verified every requested identity. Price rows
    /// have no tenant column: the verified SKU set is the tenant boundary of this query.
    pub(crate) async fn resolve_price_snapshot(
        &self,
        tenant_id: Uuid,
        variant_ids: Vec<Uuid>,
        context: PriceResolutionContext,
    ) -> Result<PriceResolutionBatch, ResolutionError> {
        let context = context.normalized()?;
        let txn = match self.db.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::MySql => {
                self.db
                    .begin_with_config(
                        Some(IsolationLevel::RepeatableRead),
                        Some(AccessMode::ReadOnly),
                    )
                    .await?
            }
            DatabaseBackend::Sqlite => self.db.begin().await?,
            _ => {
                return Err(CommerceError::Validation(
                    "Pricing snapshot backend is unsupported".into(),
                )
                .into());
            }
        };
        let resolved_at = chrono::Utc::now();
        let selected_list = if let Some(id) = context.price_list_id {
            let list = entities::price_list::Entity::find_by_id(id)
                .filter(entities::price_list::Column::TenantId.eq(tenant_id))
                .one(&txn)
                .await?
                .ok_or_else(|| CommerceError::Validation("price_list_id was not found".into()))?;
            validate_active_price_list_at(&list, resolved_at)?;
            if !channel_scope_matches(
                list.channel_id,
                list.channel_slug.as_deref(),
                context.channel_id,
                context.channel_slug.as_deref(),
            ) {
                return Err(CommerceError::Validation(
                    "price_list_id is not available for the requested channel".into(),
                )
                .into());
            }
            Some(list)
        } else {
            None
        };
        let rule = selected_list
            .as_ref()
            .map(resolution::stored_rule)
            .transpose()?
            .flatten();
        let mut query = entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.is_in(variant_ids.clone()))
            .filter(entities::price::Column::CurrencyCode.eq(&context.currency_code));
        query = match context.price_list_id {
            Some(id) => query.filter(
                Condition::any()
                    .add(entities::price::Column::PriceListId.is_null())
                    .add(entities::price::Column::PriceListId.eq(id)),
            ),
            None => query.filter(entities::price::Column::PriceListId.is_null()),
        };
        query = match context.region_id {
            Some(id) => query.filter(
                Condition::any()
                    .add(entities::price::Column::RegionId.is_null())
                    .add(entities::price::Column::RegionId.eq(id)),
            ),
            None => query.filter(entities::price::Column::RegionId.is_null()),
        };
        // Keep slug matching in the canonical Rust policy: SQL LOWER/TRIM do not
        // share its ASCII-case / Unicode-whitespace semantics for retained rows.
        query = match context.channel_id {
            Some(id) => query.filter(
                Condition::any()
                    .add(entities::price::Column::ChannelId.is_null())
                    .add(entities::price::Column::ChannelId.eq(id)),
            ),
            None => query.filter(entities::price::Column::ChannelId.is_null()),
        };
        let quantity = context.quantity.unwrap_or(1);
        let active_window = Condition::all()
            .add(
                Condition::any()
                    .add(entities::price::Column::MinQuantity.is_null())
                    .add(entities::price::Column::MinQuantity.lte(quantity)),
            )
            .add(
                Condition::any()
                    .add(entities::price::Column::MaxQuantity.is_null())
                    .add(entities::price::Column::MaxQuantity.gte(quantity)),
            );
        // Invalid windows must still reach owner validation, not disappear into unpriced/base.
        let invalid_window = Condition::any()
            .add(entities::price::Column::MinQuantity.lte(0))
            .add(entities::price::Column::MaxQuantity.lte(0))
            .add(
                Expr::col(entities::price::Column::MinQuantity)
                    .gt(Expr::col(entities::price::Column::MaxQuantity)),
            );
        query = query.filter(Condition::any().add(active_window).add(invalid_window));
        let rows = query
            .limit((MAX_PRICE_RESOLUTION_ROWS + 1) as u64)
            .all(&txn)
            .await?;
        if rows.len() > MAX_PRICE_RESOLUTION_ROWS {
            return Err(ResolutionError::RowBudget);
        }
        let mut by_variant = HashMap::<Uuid, Vec<entities::price::Model>>::new();
        for row in rows {
            by_variant.entry(row.variant_id).or_default().push(row);
        }
        let mut prices = Vec::with_capacity(variant_ids.len());
        for variant_id in variant_ids {
            let resolved = resolution::resolve(
                by_variant.remove(&variant_id).unwrap_or_default(),
                &context,
                selected_list.as_ref(),
                rule.as_ref(),
            )?;
            prices.push((variant_id, resolved));
        }
        txn.commit().await?;
        Ok(PriceResolutionBatch {
            context,
            resolved_at,
            prices,
        })
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
                    .is_none_or(|ends_at| *ends_at > now)
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

        if let Some(locale) = requested_locale.and_then(normalize_locale_tag)
            && let Some(found) = lookup.get(&locale)
        {
            return found.name.clone();
        }
        if let Some(locale) = tenant_default_locale.and_then(normalize_locale_tag)
            && let Some(found) = lookup.get(&locale)
        {
            return found.name.clone();
        }
        translations
            .first()
            .map(|translation| translation.name.clone())
            .unwrap_or_default()
    }

    #[instrument(skip(self, context))]
    #[allow(clippy::too_many_arguments)]
    pub async fn list_admin_product_pricing_with_locale_fallback(
        &self,
        context: PortContext,
        fallback_locale: Option<&str>,
        search: Option<&str>,
        status: Option<ProductStatus>,
        page: u64,
        per_page: u64,
    ) -> Result<AdminPricingProductList, PortError> {
        let budget_context = context.clone();
        crate::ports::read_with_deadline(&budget_context, async {
            context.require_policy(rustok_api::PortCallPolicy::read())?;
            crate::ports::parse_port_tenant_id(&context, "list_admin_product_pricing")?;
            let page = Ord::max(page, 1);
            let per_page = per_page.clamp(1, 100);
            let catalog = self.product_catalog_read_runtime.read_port();
            let products = catalog
                .list_legacy_admin_products(
                    context.clone(),
                    rustok_product::LegacyAdminProductsRequest {
                        locale: Some(context.locale.clone()),
                        fallback_locale: fallback_locale.map(str::to_owned),
                        search: search.map(str::to_owned),
                        status: status.map(map_foundation_product_status),
                        vendor: None,
                        page,
                        per_page,
                    },
                )
                .await
                .map_err(pricing_catalog_port_error)?;
            let ids = products
                .items
                .iter()
                .map(|item| item.id)
                .collect::<BTreeSet<_>>();
            if products.page != page
                || products.per_page != per_page
                || products.items.len() > per_page as usize
                || ids.len() != products.items.len()
                || ids.iter().any(Uuid::is_nil)
            {
                return Err(PortError::invariant_violation(
                    "pricing.catalog_page_invalid",
                    "The selected Product owner returned an inconsistent pricing inspection page",
                ));
            }
            let mut items = Vec::with_capacity(products.items.len());
            for item in products.items {
                let product = catalog
                    .read_product_projection(
                        context.clone(),
                        rustok_product::ProductProjectionRequest {
                            product_id: item.id,
                            locale: Some(context.locale.clone()),
                            fallback_locale: fallback_locale.map(str::to_owned),
                        },
                    )
                    .await
                    .map_err(pricing_catalog_port_error)?;
                validate_pricing_product_projection(&context, Some(item.id), &product)?;
                items.push(map_admin_list_item(
                    product,
                    &context.locale,
                    fallback_locale.unwrap_or(&context.locale),
                ));
            }
            Ok(AdminPricingProductList {
                items,
                total: products.total,
                page: products.page,
                per_page: products.per_page,
                has_next: products.has_next,
            })
        })
        .await
    }

    #[instrument(skip(self, context), fields(product_id = %product_id))]
    pub async fn get_admin_product_pricing_with_locale_fallback(
        &self,
        context: PortContext,
        product_id: Uuid,
        fallback_locale: Option<&str>,
        selected_price_list_id: Option<Uuid>,
    ) -> Result<AdminPricingProductDetail, PortError> {
        let budget_context = context.clone();
        crate::ports::read_with_deadline(&budget_context, async {
            let operation = "read_admin_product_pricing_projection";
            context.require_policy(rustok_api::PortCallPolicy::read())?;
            let tenant_id = crate::ports::parse_port_tenant_id(&context, operation)?;
            if product_id.is_nil() || selected_price_list_id.is_some_and(|id| id.is_nil()) {
                return Err(PortError::validation(
                    "pricing.catalog_identity_invalid",
                    "Use non-nil Product and Price List identities",
                ));
            }
            let product = self
                .product_catalog_read_runtime
                .read_port()
                .read_product_projection(
                    context.clone(),
                    rustok_product::ProductProjectionRequest {
                        product_id,
                        locale: Some(context.locale.clone()),
                        fallback_locale: fallback_locale.map(str::to_owned),
                    },
                )
                .await
                .map_err(pricing_catalog_port_error)?;
            validate_pricing_product_projection(&context, Some(product_id), &product)?;
            if let Some(price_list_id) = selected_price_list_id {
                let list = entities::price_list::Entity::find_by_id(price_list_id)
                    .filter(entities::price_list::Column::TenantId.eq(tenant_id))
                    .one(&self.db)
                    .await
                    .map_err(|error| {
                        crate::ports::pricing_error_to_port_error(&context, operation, error.into())
                    })?;
                if list.is_none() {
                    return Err(PortError::not_found(
                        "pricing.price_list_not_found",
                        "The selected Price List is not available to this tenant",
                    ));
                }
            }
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

                let prices = query
                    .limit((MAX_PUBLIC_PRICE_CATALOG_ROWS + 1) as u64)
                    .all(&self.db)
                    .await
                    .map_err(|error| {
                        crate::ports::pricing_error_to_port_error(&context, operation, error.into())
                    })?;
                if prices.len() > MAX_PUBLIC_PRICE_CATALOG_ROWS {
                    return Err(PortError::new(
                        rustok_api::PortErrorKind::Conflict,
                        "pricing.catalog_price_row_limit",
                        "Pricing inspection exceeds its row budget; narrow the selected Price List",
                        false,
                    ));
                }
                for price in prices {
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
        })
        .await
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
        let currency_code = normalize_resolution_currency(currency_code)?;
        entities::price::Entity::find()
            .filter(entities::price::Column::VariantId.eq(variant_id))
            .filter(entities::price::Column::CurrencyCode.eq(&currency_code))
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

    /// Published catalog inspection counts are not executable price quotes. Both the page
    /// and its SKU identities come from Product; only unscoped base-price rows are counted.
    /// The existing thin merchandising-summary port (historically named `legacy`) intentionally
    /// avoids loading Product's catalog-price snapshots; it is not a pricing fallback.
    pub(crate) async fn list_storefront_product_pricing(
        &self,
        context: PortContext,
        request: crate::StorefrontProductPricingListRequest,
    ) -> Result<StorefrontPricingProductList, PortError> {
        let identity_port = self.variant_identity_read_port()?;
        let products = self
            .product_catalog_read_runtime
            .read_port()
            .list_legacy_storefront_products(
                context.clone(),
                rustok_product::LegacyStorefrontProductsRequest {
                    vendor: None,
                    product_type: None,
                    search: None,
                    locale: Some(context.locale.clone()),
                    fallback_locale: request.fallback_locale,
                    public_channel_slug: request.channel_slug,
                    page: request.page,
                    per_page: request.per_page,
                },
            )
            .await
            .map_err(pricing_catalog_port_error)?;
        let selected_product_ids = products
            .items
            .iter()
            .map(|product| product.id)
            .collect::<BTreeSet<_>>();
        if products.items.len() > request.per_page as usize
            || selected_product_ids.len() != products.items.len()
            || selected_product_ids.iter().any(Uuid::is_nil)
            || products.page != request.page
            || products.per_page != request.per_page
        {
            return Err(PortError::invariant_violation(
                "pricing.catalog_page_invalid",
                "The selected Product owner returned an inconsistent pricing inspection page",
            ));
        }
        let identity_request = rustok_product::ProductVariantIdentityListRequest {
            product_ids: products.items.iter().map(|product| product.id).collect(),
        };
        let variants = identity_port
            .list_product_variant_identities(context.clone(), identity_request.clone())
            .await
            .map_err(pricing_identity_port_error)?;
        rustok_product::validate_product_variant_identity_list_response(
            &context,
            &identity_request,
            &variants,
        )
        .map_err(|error| {
            PortError::new(
                error.kind,
                "pricing.variant_identity_invalid",
                "The selected Product owner returned inconsistent page SKU ownership",
                error.retryable,
            )
        })?;
        let mut variant_counts_by_product = HashMap::<Uuid, u64>::new();
        let mut variant_to_product = HashMap::<Uuid, Uuid>::new();
        for variant in variants {
            variant_to_product.insert(variant.variant_id, variant.product_id);
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
                .filter(rustok_pricing_persistence::catalog_base_scope())
                .limit((MAX_PUBLIC_PRICE_CATALOG_ROWS + 1) as u64)
                .all(&self.db)
                .await
                .map_err(|error| {
                    crate::ports::pricing_error_to_port_error(
                        &context,
                        "list_storefront_product_pricing",
                        CommerceError::Database(error),
                    )
                })?
        };
        if prices.len() > MAX_PUBLIC_PRICE_CATALOG_ROWS {
            return Err(PortError::new(
                rustok_api::PortErrorKind::Conflict,
                "pricing.catalog_price_row_limit",
                "Pricing inspection page exceeds its row budget; reduce page size",
                false,
            ));
        }
        let mut currencies_by_product = HashMap::<Uuid, BTreeSet<String>>::new();
        let mut sale_variants_by_product = HashMap::<Uuid, BTreeSet<Uuid>>::new();
        for price in prices {
            resolution::validate_row(&price, &price.currency_code, None).map_err(|error| {
                crate::ports::resolution_error_to_port_error(
                    &context,
                    "list_storefront_product_pricing",
                    error,
                )
            })?;
            let product_id = variant_to_product
                .get(&price.variant_id)
                .copied()
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "pricing.catalog_identity_incomplete",
                        "Pricing catalog identity projection is incomplete",
                    )
                })?;
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

    #[instrument(skip(self, context))]
    pub async fn get_published_product_pricing_by_handle_with_locale_fallback(
        &self,
        context: PortContext,
        handle: &str,
        fallback_locale: Option<&str>,
        public_channel_slug: Option<&str>,
    ) -> Result<Option<StorefrontPricingProductDetail>, PortError> {
        let budget_context = context.clone();
        crate::ports::read_with_deadline(&budget_context, async {
            context.require_policy(rustok_api::PortCallPolicy::read())?;
            crate::ports::parse_port_tenant_id(
                &context,
                "read_storefront_product_pricing_projection",
            )?;
            let product = self
                .product_catalog_read_runtime
                .read_port()
                .read_storefront_product_projection(
                    context.clone(),
                    rustok_product::StorefrontProductProjectionRequest {
                        subject: rustok_product::StorefrontProductProjectionSubject::Handle {
                            handle: handle.to_owned(),
                        },
                        locale: Some(context.locale.clone()),
                        fallback_locale: fallback_locale.map(str::to_owned),
                        public_channel_slug: public_channel_slug.map(str::to_owned),
                    },
                )
                .await
                .map_err(pricing_catalog_port_error)?;
            if let Some(product) = product.as_ref() {
                validate_pricing_product_projection(&context, None, product)?;
                if !product
                    .translations
                    .iter()
                    .any(|translation| translation.handle == handle)
                {
                    return Err(PortError::invariant_violation(
                        "pricing.catalog_identity_invalid",
                        "The selected Product owner returned a different storefront link",
                    ));
                }
            }
            Ok(product.map(map_product_detail))
        })
        .await
    }
}

fn pricing_identity_port_error(error: PortError) -> PortError {
    let code = if error.code == "product.variant_product_mismatch" {
        "pricing.variant_product_mismatch"
    } else if error.kind == PortErrorKind::NotFound {
        "pricing.variant_not_found"
    } else {
        "pricing.variant_identity_unavailable"
    };
    PortError::new(
        error.kind,
        code,
        "SKU ownership could not be verified for this pricing request",
        error.retryable,
    )
}

fn pricing_catalog_port_error(error: PortError) -> PortError {
    let code = match error.kind {
        rustok_api::PortErrorKind::NotFound => "pricing.product_not_found",
        rustok_api::PortErrorKind::Validation => "pricing.catalog_request_invalid",
        _ => "pricing.catalog_unavailable",
    };
    PortError::new(
        error.kind,
        code,
        "The selected Product projection could not be established",
        error.retryable,
    )
}

fn validate_pricing_product_projection(
    context: &PortContext,
    expected_product_id: Option<Uuid>,
    product: &ProductResponse,
) -> Result<(), PortError> {
    let tenant_id = crate::ports::parse_port_tenant_id(context, "pricing_product_projection")?;
    let variants = product
        .variants
        .iter()
        .map(|variant| variant.id)
        .collect::<BTreeSet<_>>();
    if product.tenant_id != tenant_id
        || product.id.is_nil()
        || expected_product_id.is_some_and(|id| id != product.id)
        || product.variants.len() > rustok_product::MAX_VARIANT_IDENTITY_BATCH
        || variants.len() != product.variants.len()
        || variants.iter().any(Uuid::is_nil)
        || product
            .variants
            .iter()
            .any(|variant| variant.product_id != product.id)
    {
        return Err(PortError::invariant_violation(
            "pricing.catalog_identity_invalid",
            "The selected Product owner returned an inconsistent Product/SKU projection",
        ));
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn normalize_resolution_currency(currency_code: &str) -> CommerceResult<String> {
    let normalized = money::normalize_currency_code(currency_code)
        .map_err(|_| CommerceError::Validation("currency_code must be a 3-letter code".into()))?;
    money::currency_exponent(&normalized).map_err(|_| {
        CommerceError::Validation("currency_code has no supported decimal minor unit".into())
    })?;
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

    if let Some(scoped_channel_id) = scoped_channel_id
        && Some(scoped_channel_id) != requested_channel_id
    {
        return false;
    }

    if let Some(scoped_channel_slug) = scoped_channel_slug
        && Some(scoped_channel_slug.as_str()) != requested_channel_slug.as_deref()
    {
        return false;
    }

    true
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

async fn validate_channel_scope_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id must not be a nil UUID".to_string(),
        ));
    }
    if let Some(channel_id) = channel_id
        && channel_id.is_nil()
    {
        return Err(CommerceError::Validation(
            "channel_id must not be a nil UUID".to_string(),
        ));
    }
    let normalized_slug = normalize_channel_slug(channel_slug);
    match channel_id {
        Some(channel_id) => {
            let model = channel::Entity::find_by_id(channel_id)
                .filter(channel::Column::TenantId.eq(tenant_id))
                .one(txn)
                .await?
                .ok_or_else(|| {
                    CommerceError::Validation(
                        "channel_id must reference a channel owned by the price-list tenant"
                            .to_string(),
                    )
                })?;
            if let Some(slug) = normalized_slug.as_deref()
                && slug
                    != normalize_channel_slug(Some(model.slug.as_str()))
                        .as_deref()
                        .unwrap_or_default()
            {
                return Err(CommerceError::Validation(
                    "channel_id and channel_slug must reference the same tenant channel"
                        .to_string(),
                ));
            }
        }
        None => {
            if let Some(slug) = normalized_slug {
                let exists = channel::Entity::find()
                    .filter(channel::Column::TenantId.eq(tenant_id))
                    .filter(channel::Column::Slug.eq(slug.clone()))
                    .one(txn)
                    .await?
                    .is_some();
                if !exists {
                    return Err(CommerceError::Validation(
                        "channel_slug must reference a channel owned by the price-list tenant"
                            .to_string(),
                    ));
                }
            }
        }
    }
    Ok(())
}

async fn load_variant_for_update_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> CommerceResult<entities::product_variant::Model> {
    if tenant_id.is_nil() || variant_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id and variant_id must not be nil UUIDs".to_string(),
        ));
    }
    let query = entities::product_variant::Entity::find_by_id(variant_id)
        .filter(entities::product_variant::Column::TenantId.eq(tenant_id));
    let variant = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        DatabaseBackend::Sqlite => {
            let statement = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE product_variants SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                [tenant_id.into(), variant_id.into()],
            );
            txn.execute_raw(statement).await?;
            query.one(txn).await?
        }
        _ => query.one(txn).await?,
    };
    variant.ok_or(CommerceError::VariantNotFound(variant_id))
}

async fn resolve_active_price_list(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> CommerceResult<entities::price_list::Model> {
    if tenant_id.is_nil() || price_list_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id and price_list_id must not be nil UUIDs".to_string(),
        ));
    }
    let price_list = entities::price_list::Entity::find_by_id(price_list_id)
        .filter(entities::price_list::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
        .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))?;

    validate_active_price_list(&price_list)?;
    Ok(price_list)
}

async fn resolve_active_price_list_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> CommerceResult<entities::price_list::Model> {
    if tenant_id.is_nil() || price_list_id.is_nil() {
        return Err(CommerceError::Validation(
            "tenant_id and price_list_id must not be nil UUIDs".to_string(),
        ));
    }
    let query = entities::price_list::Entity::find_by_id(price_list_id)
        .filter(entities::price_list::Column::TenantId.eq(tenant_id));
    let price_list = match txn.get_database_backend() {
        DatabaseBackend::Postgres | DatabaseBackend::MySql => {
            query.lock_exclusive().one(txn).await?
        }
        DatabaseBackend::Sqlite => {
            let statement = Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                "UPDATE price_lists SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                [tenant_id.into(), price_list_id.into()],
            );
            txn.execute_raw(statement).await?;
            query.one(txn).await?
        }
        _ => query.one(txn).await?,
    }
    .ok_or_else(|| CommerceError::Validation("price_list_id was not found".to_string()))?;

    validate_active_price_list(&price_list)?;
    Ok(price_list)
}

#[allow(clippy::result_large_err)]
fn validate_active_price_list(price_list: &entities::price_list::Model) -> CommerceResult<()> {
    validate_active_price_list_at(price_list, chrono::Utc::now())
        .map_err(ResolutionError::into_commerce)
}

#[allow(clippy::result_large_err)]
fn validate_active_price_list_at(
    price_list: &entities::price_list::Model,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(), ResolutionError> {
    if price_list.channel_id.is_some_and(|id| id.is_nil())
        || price_list
            .channel_slug
            .as_deref()
            .is_some_and(|slug| slug.trim().is_empty())
        || !matches!(price_list.r#type.as_str(), "sale" | "override")
        || price_list
            .starts_at
            .zip(price_list.ends_at)
            .is_some_and(|(start, end)| start >= end)
    {
        return Err(resolution::invalid_data(
            "Stored price-list scope, type or window is invalid",
        ));
    }
    if !price_list.status.eq_ignore_ascii_case("active") {
        return Err(CommerceError::Validation(
            "price_list_id must reference an active price list".to_string(),
        )
        .into());
    }

    if price_list
        .starts_at
        .as_ref()
        .is_some_and(|starts_at| *starts_at > now)
    {
        return Err(
            CommerceError::Validation("price_list_id is not active yet".to_string()).into(),
        );
    }
    if price_list
        .ends_at
        .as_ref()
        .is_some_and(|ends_at| *ends_at <= now)
    {
        return Err(
            CommerceError::Validation("price_list_id is already expired".to_string()).into(),
        );
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

fn map_product_detail(product: ProductResponse) -> StorefrontPricingProductDetail {
    StorefrontPricingProductDetail {
        id: product.id,
        status: map_product_status(product.status),
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
    product: ProductResponse,
    locale: &str,
    fallback_locale: &str,
) -> AdminPricingProductListItem {
    let translation =
        pick_product_translation(product.translations.as_slice(), locale, fallback_locale);

    AdminPricingProductListItem {
        id: product.id,
        status: map_product_status(product.status),
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
    product: ProductResponse,
    mut prices_by_variant: HashMap<Uuid, Vec<entities::price::Model>>,
) -> AdminPricingProductDetail {
    AdminPricingProductDetail {
        id: product.id,
        status: map_product_status(product.status),
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
                combination_identity: variant.combination_identity,
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

fn admin_pricing_price_from_model(price: entities::price::Model) -> AdminPricingPrice {
    AdminPricingPrice {
        currency_code: price.currency_code,
        amount: price.amount,
        compare_at_amount: price.compare_at_amount,
        discount_percent: calculate_discount_percent(price.amount, price.compare_at_amount),
        on_sale: is_sale_price(price.amount, price.compare_at_amount),
        price_list_id: price.price_list_id,
        channel_id: price.channel_id,
        channel_slug: price.channel_slug,
        min_quantity: price.min_quantity,
        max_quantity: price.max_quantity,
    }
}

/// Stored prices must be exactly representable in the currency's minor units. Otherwise the
/// catalog accepts a price (e.g. 9.995 USD) that events round and payment providers reject only
/// after an order and inventory reservation already exist.
#[allow(clippy::result_large_err)]
fn validate_price_precision(
    amount: Decimal,
    compare_at_amount: Option<Decimal>,
    currency_code: &str,
) -> CommerceResult<()> {
    for (label, value) in [
        ("Amount", Some(amount)),
        ("Compare at price", compare_at_amount),
    ] {
        if let Some(value) = value {
            money::to_minor_units_exact(value, currency_code).map_err(|error| {
                CommerceError::InvalidPrice(format!(
                    "{label} {value} is not representable in {currency_code} minor units: {error}"
                ))
            })?;
        }
    }
    Ok(())
}

#[allow(clippy::result_large_err)]
fn validate_price_tier_quantities(
    min_quantity: Option<i32>,
    max_quantity: Option<i32>,
) -> CommerceResult<()> {
    if let Some(min_quantity) = min_quantity
        && min_quantity <= 0
    {
        return Err(CommerceError::InvalidPrice(
            "Minimum quantity must be positive".into(),
        ));
    }

    if let Some(max_quantity) = max_quantity
        && max_quantity <= 0
    {
        return Err(CommerceError::InvalidPrice(
            "Maximum quantity must be positive".into(),
        ));
    }

    if let (Some(min_quantity), Some(max_quantity)) = (min_quantity, max_quantity)
        && max_quantity < min_quantity
    {
        return Err(CommerceError::InvalidPrice(
            "Maximum quantity must be greater than or equal to minimum quantity".into(),
        ));
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
    translations: &'a [ProductTranslationResponse],
    locale: &str,
    fallback_locale: &str,
) -> Option<&'a ProductTranslationResponse> {
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

fn map_product_status(status: rustok_product::entities::product::ProductStatus) -> ProductStatus {
    match status {
        rustok_product::entities::product::ProductStatus::Draft => ProductStatus::Draft,
        rustok_product::entities::product::ProductStatus::Active => ProductStatus::Active,
        rustok_product::entities::product::ProductStatus::Archived => ProductStatus::Archived,
    }
}

fn map_foundation_product_status(
    status: ProductStatus,
) -> rustok_product::entities::product::ProductStatus {
    match status {
        ProductStatus::Draft => rustok_product::entities::product::ProductStatus::Draft,
        ProductStatus::Active => rustok_product::entities::product::ProductStatus::Active,
        ProductStatus::Archived => rustok_product::entities::product::ProductStatus::Archived,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_discount_percent_bounds() {
        assert!(validate_discount_percent(Decimal::new(10, 0)).is_ok());
        assert!(validate_discount_percent(Decimal::new(100, 0)).is_ok());
        assert!(validate_discount_percent(Decimal::ZERO).is_err());
        assert!(validate_discount_percent(Decimal::new(-5, 0)).is_err());
        assert!(validate_discount_percent(Decimal::new(101, 0)).is_err());
    }

    #[test]
    fn validate_price_tier_quantities_bounds() {
        assert!(validate_price_tier_quantities(Some(1), Some(10)).is_ok());
        assert!(validate_price_tier_quantities(Some(5), Some(5)).is_ok());
        assert!(validate_price_tier_quantities(None, None).is_ok());
        assert!(validate_price_tier_quantities(Some(0), Some(10)).is_err());
        assert!(validate_price_tier_quantities(Some(10), Some(5)).is_err());
        assert!(validate_price_tier_quantities(Some(1), Some(-1)).is_err());
    }

    #[test]
    fn calculate_discount_percent_precision() {
        let amount = Decimal::new(80, 0);
        let compare_at = Some(Decimal::new(100, 0));
        assert_eq!(
            calculate_discount_percent(amount, compare_at),
            Some(Decimal::new(2000, 2))
        );

        let amount_equal = Decimal::new(100, 0);
        assert_eq!(calculate_discount_percent(amount_equal, compare_at), None);

        let amount_greater = Decimal::new(120, 0);
        assert_eq!(calculate_discount_percent(amount_greater, compare_at), None);
    }

    #[test]
    fn normalize_resolution_currency_accepts_valid_and_rejects_invalid() {
        assert_eq!(normalize_resolution_currency("usd").unwrap(), "USD");
        assert_eq!(normalize_resolution_currency("EUR").unwrap(), "EUR");
        assert!(normalize_resolution_currency("US").is_err());
        assert!(normalize_resolution_currency("USDT").is_err());
        assert!(normalize_resolution_currency("123").is_err());
    }

    #[test]
    fn normalize_resolution_quantity_bounds() {
        assert_eq!(normalize_resolution_quantity(Some(5)).unwrap(), 5);
        assert_eq!(normalize_resolution_quantity(None).unwrap(), 1);
        assert!(normalize_resolution_quantity(Some(0)).is_err());
        assert!(normalize_resolution_quantity(Some(-1)).is_err());
    }

    #[test]
    fn is_sale_price_behavior() {
        assert!(is_sale_price(
            Decimal::new(80, 0),
            Some(Decimal::new(100, 0))
        ));
        assert!(!is_sale_price(
            Decimal::new(100, 0),
            Some(Decimal::new(100, 0))
        ));
        assert!(!is_sale_price(
            Decimal::new(120, 0),
            Some(Decimal::new(100, 0))
        ));
        assert!(!is_sale_price(Decimal::new(80, 0), None));
    }
}
