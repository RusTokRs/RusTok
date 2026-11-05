use rust_decimal::Decimal;
use sea_orm::sea_query::Expr;
use sea_orm::{
    AccessMode, ColumnTrait, Condition, DatabaseBackend, DatabaseConnection, EntityTrait,
    IsolationLevel, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{PortContext, PortError, normalize_locale_tag};
use rustok_commerce_foundation::error::{CommerceError, CommerceResult};
use rustok_core::money;

use super::resolution::{self, ResolutionError};
use super::types::*;
use super::PricingService;
use crate::entities::{price, price_list, price_list_translation};

impl PricingService {
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
    pub(crate) async fn preview_scoped_percentage_discount(
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
    pub async fn get_price(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
        currency_code: &str,
    ) -> CommerceResult<Option<Decimal>> {
        self.ensure_variant_tenant(tenant_id, variant_id).await?;
        let price = price::Entity::find()
            .filter(price::Column::VariantId.eq(variant_id))
            .filter(price::Column::CurrencyCode.eq(currency_code))
            .filter(price::Column::RegionId.is_null())
            .filter(price::Column::PriceListId.is_null())
            .filter(price::Column::MinQuantity.is_null())
            .filter(price::Column::MaxQuantity.is_null())
            .one(&self.db)
            .await?;

        Ok(price.map(|price| price.amount))
    }

    #[instrument(skip(self))]
    pub async fn get_variant_prices(
        &self,
        tenant_id: Uuid,
        variant_id: Uuid,
    ) -> CommerceResult<Vec<price::Model>> {
        self.ensure_variant_tenant(tenant_id, variant_id).await?;
        let prices = price::Entity::find()
            .filter(price::Column::VariantId.eq(variant_id))
            .all(&self.db)
            .await?;

        Ok(prices)
    }

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
            let list = price_list::Entity::find_by_id(id)
                .filter(price_list::Column::TenantId.eq(tenant_id))
                .one(&txn)
                .await?
                .ok_or_else(|| CommerceError::Validation("price_list_id was not found".into()))?;
            super::commands::validate_active_price_list_at(&list, resolved_at)?;
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
        let mut query = price::Entity::find()
            .filter(price::Column::VariantId.is_in(variant_ids.clone()))
            .filter(price::Column::CurrencyCode.eq(&context.currency_code));
        query = match context.price_list_id {
            Some(id) => query.filter(
                Condition::any()
                    .add(price::Column::PriceListId.is_null())
                    .add(price::Column::PriceListId.eq(id)),
            ),
            None => query.filter(price::Column::PriceListId.is_null()),
        };
        query = match context.region_id {
            Some(id) => query.filter(
                Condition::any()
                    .add(price::Column::RegionId.is_null())
                    .add(price::Column::RegionId.eq(id)),
            ),
            None => query.filter(price::Column::RegionId.is_null()),
        };
        query = match context.channel_id {
            Some(id) => query.filter(
                Condition::any()
                    .add(price::Column::ChannelId.is_null())
                    .add(price::Column::ChannelId.eq(id)),
            ),
            None => query.filter(price::Column::ChannelId.is_null()),
        };
        let quantity = context.quantity.unwrap_or(1);
        let active_window = Condition::all()
            .add(
                Condition::any()
                    .add(price::Column::MinQuantity.is_null())
                    .add(price::Column::MinQuantity.lte(quantity)),
            )
            .add(
                Condition::any()
                    .add(price::Column::MaxQuantity.is_null())
                    .add(price::Column::MaxQuantity.gte(quantity)),
            );
        let invalid_window = Condition::any()
            .add(price::Column::MinQuantity.lte(0))
            .add(price::Column::MaxQuantity.lte(0))
            .add(
                Expr::col(price::Column::MinQuantity)
                    .gt(Expr::col(price::Column::MaxQuantity)),
            );
        query = query.filter(Condition::any().add(active_window).add(invalid_window));
        let rows = query
            .limit((MAX_PRICE_RESOLUTION_ROWS + 1) as u64)
            .all(&txn)
            .await?;
        if rows.len() > MAX_PRICE_RESOLUTION_ROWS {
            return Err(ResolutionError::RowBudget);
        }
        let mut by_variant = HashMap::<Uuid, Vec<price::Model>>::new();
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
        let price_lists = price_list::Entity::find()
            .filter(price_list::Column::TenantId.eq(tenant_id))
            .order_by_desc(price_list::Column::UpdatedAt)
            .all(&self.db)
            .await?;
        let translations = if price_lists.is_empty() {
            Vec::new()
        } else {
            let ids = price_lists
                .iter()
                .map(|price_list| price_list.id)
                .collect::<Vec<_>>();
            price_list_translation::Entity::find()
                .filter(price_list_translation::Column::PriceListId.is_in(ids))
                .all(&self.db)
                .await?
        };
        let mut translations_by_list: HashMap<Uuid, Vec<price_list_translation::Model>> =
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

    pub(crate) fn resolve_price_list_name(
        translations: &[price_list_translation::Model],
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

    pub(crate) async fn find_canonical_price_row(
        &self,
        variant_id: Uuid,
        currency_code: &str,
        price_list_id: Option<Uuid>,
        channel_id: Option<Uuid>,
        channel_slug: Option<&str>,
    ) -> CommerceResult<price::Model> {
        let currency_code = normalize_resolution_currency(currency_code)?;
        price::Entity::find()
            .filter(price::Column::VariantId.eq(variant_id))
            .filter(price::Column::CurrencyCode.eq(&currency_code))
            .filter(price::Column::RegionId.is_null())
            .filter(optional_uuid_filter(
                price::Column::PriceListId,
                price_list_id,
            ))
            .filter(optional_uuid_filter(
                price::Column::ChannelId,
                channel_id,
            ))
            .filter(optional_string_filter(
                price::Column::ChannelSlug,
                normalize_channel_slug(channel_slug),
            ))
            .filter(price::Column::MinQuantity.is_null())
            .filter(price::Column::MaxQuantity.is_null())
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
}

pub(crate) async fn resolve_requested_price_list_id(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Option<Uuid>,
    channel_id: Option<Uuid>,
    channel_slug: Option<&str>,
) -> CommerceResult<Option<Uuid>> {
    let Some(price_list_id) = price_list_id else {
        return Ok(None);
    };

    let price_list = super::commands::resolve_active_price_list(db, tenant_id, price_list_id).await?;

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

pub(crate) fn channel_scope_matches(
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

pub(crate) fn optional_int_filter(column: price::Column, value: Option<i32>) -> sea_orm::Condition {
    match value {
        Some(value) => sea_orm::Condition::all().add(column.eq(value)),
        None => sea_orm::Condition::all().add(column.is_null()),
    }
}

pub(crate) fn optional_uuid_filter(
    column: price::Column,
    value: Option<Uuid>,
) -> sea_orm::Condition {
    match value {
        Some(value) => sea_orm::Condition::all().add(column.eq(value)),
        None => sea_orm::Condition::all().add(column.is_null()),
    }
}

pub(crate) fn optional_string_filter(
    column: price::Column,
    value: Option<String>,
) -> sea_orm::Condition {
    match value {
        Some(value) => sea_orm::Condition::all().add(column.eq(value)),
        None => sea_orm::Condition::all().add(column.is_null()),
    }
}

pub(crate) fn price_specificity_cmp(
    left: &price::Model,
    right: &price::Model,
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
