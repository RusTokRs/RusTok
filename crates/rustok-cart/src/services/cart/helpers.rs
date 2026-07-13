use chrono::Utc;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set,
    Statement,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    time::Duration,
};
use uuid::Uuid;

use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, normalize_locale_tag,
};
use rustok_commerce_foundation::entities::{region, region_country_tax_policy};
use rustok_core::generate_id;
use rustok_fulfillment::entities::shipping_option;
use rustok_tax::{
    TaxCalculationInput, TaxCalculationPort, TaxPolicyCountryRule, TaxPolicySnapshot, TaxableAmount,
};

use crate::dto::{
    CartAdjustmentResponse, CartDeliveryGroupResponse, CartLineItemResponse, CartResponse,
    CartTaxLineResponse, UpdateCartContextInput,
};
use crate::entities;
use crate::error::{CartError, CartResult};

use super::types::*;

pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_CHECKING_OUT: &str = "checking_out";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_ABANDONED: &str = "abandoned";

pub const DEFAULT_SHIPPING_PROFILE_SLUG: &str = "default";
pub const PRICING_ADJUSTMENT_SOURCE_TYPE: &str = "pricing";
pub const PROMOTION_ADJUSTMENT_SOURCE_TYPE: &str = "promotion";
pub const CART_PROMOTION_SCOPE: &str = "cart";
pub const LINE_ITEM_PROMOTION_SCOPE: &str = "line_item";
pub const SHIPPING_PROMOTION_SCOPE: &str = "shipping";

fn cart_tax_port_context(cart: &entities::cart::Model) -> PortContext {
    let context = PortContext::new(
        cart.tenant_id.to_string(),
        PortActor::service("rustok-cart.tax"),
        cart.locale_code
            .as_deref()
            .unwrap_or(PLATFORM_FALLBACK_LOCALE),
        format!("cart-tax:{}", cart.id),
    )
    .with_deadline(Duration::from_secs(2));

    match cart.channel_id {
        Some(channel_id) => context.with_channel(channel_id.to_string()),
        None => context,
    }
}

fn cart_tax_port_error(error: PortError) -> CartError {
    CartError::TaxBoundary {
        kind: error.kind,
        code: error.code,
        message: error.message,
        retryable: error.retryable,
    }
}

pub fn ensure_active(status: &str, action: &str) -> CartResult<()> {
    if status == STATUS_ACTIVE {
        Ok(())
    } else {
        Err(CartError::InvalidTransition {
            from: status.to_string(),
            to: action.to_string(),
        })
    }
}

pub fn normalize_country_code(value: &str) -> CartResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 2 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(normalized)
    } else {
        Err(CartError::Validation(format!(
            "country_code `{value}` is invalid"
        )))
    }
}

pub fn normalize_locale_code(value: &str) -> CartResult<String> {
    let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
    if (2..=10).contains(&normalized.len()) {
        Ok(normalized)
    } else {
        Err(CartError::Validation(format!(
            "locale_code `{value}` is invalid"
        )))
    }
}

pub fn normalize_shipping_profile_slug(value: Option<&str>) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| DEFAULT_SHIPPING_PROFILE_SLUG.to_string())
}

pub fn normalize_seller_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

pub fn normalize_adjustment_source_type(value: &str) -> CartResult<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized.len() > 64 {
        return Err(CartError::Validation(
            "adjustment source_type must be 1-64 characters".to_string(),
        ));
    }
    Ok(normalized)
}

pub fn normalize_adjustment_source_id(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_owned())
}

pub fn subtotal_amount(line_items: &[entities::cart_line_item::Model]) -> Decimal {
    line_items
        .iter()
        .fold(Decimal::ZERO, |acc, item| acc + item.total_price)
}

pub fn adjustment_total(adjustments: &[entities::cart_adjustment::Model]) -> Decimal {
    adjustments
        .iter()
        .fold(Decimal::ZERO, |acc, adjustment| acc + adjustment.amount)
}

pub fn net_total(subtotal_amount: Decimal, adjustment_total: Decimal) -> Decimal {
    if adjustment_total > subtotal_amount {
        Decimal::ZERO
    } else {
        subtotal_amount - adjustment_total
    }
}

pub fn channel_tax_provider_id(metadata: &Value, channel_id: Option<Uuid>) -> Option<String> {
    let channel_id = channel_id?;
    let channel_key = channel_id.to_string();
    let value = metadata
        .get("channel_tax_provider_ids")
        .and_then(Value::as_object)
        .and_then(|mapping| mapping.get(&channel_key))?;

    match value {
        Value::String(value) => Some(value.as_str()),
        Value::Object(value) => value
            .get("provider_id")
            .and_then(Value::as_str)
            .or_else(|| value.get("provider").and_then(Value::as_str)),
        _ => None,
    }
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(|value| value.to_string())
}

pub fn seller_id_from_metadata(metadata: &Value) -> Option<String> {
    metadata
        .get("seller")
        .and_then(|seller| seller.get("id"))
        .and_then(Value::as_str)
        .and_then(|value| normalize_seller_id(Some(value)))
        .or_else(|| {
            metadata
                .get("seller_id")
                .and_then(Value::as_str)
                .and_then(|value| normalize_seller_id(Some(value)))
        })
}

pub fn delivery_group_snapshot_for_line_item(
    item: &entities::cart_line_item::Model,
) -> DeliveryGroupSnapshot {
    let seller_id = seller_id_from_metadata(&item.metadata);
    DeliveryGroupSnapshot {
        key: DeliveryGroupKey {
            shipping_profile_slug: normalize_shipping_profile_slug(Some(
                item.shipping_profile_slug.as_str(),
            )),
            seller_id,
            seller_scope: None,
        },
    }
}

pub fn collect_delivery_group_snapshots(
    line_items: &[entities::cart_line_item::Model],
) -> BTreeSet<DeliveryGroupSnapshot> {
    line_items
        .iter()
        .map(delivery_group_snapshot_for_line_item)
        .collect()
}

impl PartialEq for DeliveryGroupSnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl Eq for DeliveryGroupSnapshot {}

impl PartialOrd for DeliveryGroupSnapshot {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DeliveryGroupSnapshot {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

pub fn matching_delivery_group_keys(
    available_groups: &BTreeSet<DeliveryGroupSnapshot>,
    shipping_profile_slug: &str,
    seller_id: Option<&str>,
    _seller_scope: Option<&str>,
) -> Vec<DeliveryGroupKey> {
    available_groups
        .iter()
        .filter(|group| {
            if group.key.shipping_profile_slug != shipping_profile_slug {
                return false;
            }

            if let Some(seller_id) = seller_id {
                return group.key.seller_id.as_deref() == Some(seller_id);
            }

            group.key.seller_id.is_none()
        })
        .map(|group| group.key.clone())
        .collect()
}

pub fn selection_map_from_records<I>(
    available_groups: &BTreeSet<DeliveryGroupSnapshot>,
    records: I,
) -> BTreeMap<DeliveryGroupKey, Option<Uuid>>
where
    I: IntoIterator<Item = entities::cart_shipping_selection::Model>,
{
    let mut desired = BTreeMap::new();

    for record in records {
        let seller_id = normalize_seller_id(record.seller_id.as_deref());
        for key in matching_delivery_group_keys(
            available_groups,
            record.shipping_profile_slug.as_str(),
            seller_id.as_deref(),
            None,
        ) {
            desired.insert(key, record.selected_shipping_option_id);
        }
    }

    desired
}

pub fn build_delivery_groups(
    line_items: &[entities::cart_line_item::Model],
    selection_map: &BTreeMap<DeliveryGroupKey, Option<Uuid>>,
) -> Vec<CartDeliveryGroupResponse> {
    let mut groups = BTreeMap::<DeliveryGroupKey, Vec<Uuid>>::new();
    for item in line_items {
        let snapshot = delivery_group_snapshot_for_line_item(item);
        groups
            .entry(snapshot.key)
            .and_modify(|line_item_ids| line_item_ids.push(item.id))
            .or_insert_with(|| vec![item.id]);
    }

    groups
        .into_iter()
        .map(|(group_key, line_item_ids)| CartDeliveryGroupResponse {
            selected_shipping_option_id: selection_map.get(&group_key).copied().flatten(),
            shipping_profile_slug: group_key.shipping_profile_slug,
            seller_id: group_key.seller_id,
            seller_scope: group_key.seller_scope,
            line_item_ids,
            available_shipping_options: Vec::new(),
        })
        .collect()
}

pub fn sanitize_line_item_metadata(metadata: Value) -> Value {
    let mut metadata = match metadata {
        Value::Object(object) => object,
        value => return value,
    };

    metadata.remove("seller_label");

    if let Some(Value::Object(mut seller)) = metadata.remove("seller") {
        seller.remove("label");
        metadata.insert("seller".to_string(), Value::Object(seller));
    }

    Value::Object(metadata)
}

pub fn sanitize_adjustment_metadata(metadata: Value) -> Value {
    let mut metadata = match metadata {
        Value::Object(object) => object,
        value => return value,
    };

    metadata.remove("label");
    metadata.remove("display_label");
    metadata.remove("localized_label");

    Value::Object(metadata)
}

pub fn validate_promotion_percent(discount_percent: Decimal) -> CartResult<()> {
    if discount_percent <= Decimal::ZERO || discount_percent > Decimal::from(100) {
        return Err(CartError::Validation(
            "discount_percent must be greater than 0 and less than or equal to 100".to_string(),
        ));
    }
    Ok(())
}

pub fn validate_fixed_promotion_amount(amount: Decimal) -> CartResult<()> {
    if amount <= Decimal::ZERO {
        return Err(CartError::Validation(
            "promotion amount must be greater than zero".to_string(),
        ));
    }
    Ok(())
}

pub fn normalize_required_adjustment_source_id(value: &str) -> CartResult<String> {
    normalize_adjustment_source_id(Some(value)).ok_or_else(|| {
        CartError::Validation("promotion source_id must be 1-191 characters".to_string())
    })
}

pub fn matches_promotion_adjustment(
    adjustment: &entities::cart_adjustment::Model,
    line_item_id: Option<Uuid>,
    source_id: &str,
) -> bool {
    adjustment.source_type == PROMOTION_ADJUSTMENT_SOURCE_TYPE
        && adjustment.cart_line_item_id == line_item_id
        && normalize_adjustment_source_id(adjustment.source_id.as_deref()).as_deref()
            == Some(source_id)
}

pub fn resolve_promotion_base_amount(
    line_items: &[entities::cart_line_item::Model],
    adjustments: &[entities::cart_adjustment::Model],
    line_item_id: Option<Uuid>,
    source_id: &str,
) -> CartResult<Decimal> {
    match line_item_id {
        Some(line_item_id) => {
            let line_item = line_items
                .iter()
                .find(|item| item.id == line_item_id)
                .ok_or(CartError::CartLineItemNotFound(line_item_id))?;
            let existing_adjustments = adjustments
                .iter()
                .filter(|adjustment| adjustment.cart_line_item_id == Some(line_item_id))
                .filter(|adjustment| {
                    !matches_promotion_adjustment(adjustment, Some(line_item_id), source_id)
                })
                .fold(Decimal::ZERO, |acc, adjustment| acc + adjustment.amount);
            Ok((line_item.total_price - existing_adjustments).max(Decimal::ZERO))
        }
        None => {
            let subtotal_amount = subtotal_amount(line_items);
            let existing_adjustments = adjustments
                .iter()
                .filter(|adjustment| !matches_promotion_adjustment(adjustment, None, source_id))
                .fold(Decimal::ZERO, |acc, adjustment| acc + adjustment.amount);
            Ok((subtotal_amount - existing_adjustments).max(Decimal::ZERO))
        }
    }
}

pub fn resolve_shipping_promotion_base_amount(
    shipping_total: Decimal,
    adjustments: &[entities::cart_adjustment::Model],
    source_id: &str,
) -> Decimal {
    let existing_adjustments = adjustments
        .iter()
        .filter(|adjustment| adjustment.cart_line_item_id.is_none())
        .filter(|adjustment| adjustment_scope(adjustment) == Some(SHIPPING_PROMOTION_SCOPE))
        .filter(|adjustment| !matches_promotion_adjustment(adjustment, None, source_id))
        .fold(Decimal::ZERO, |acc, adjustment| acc + adjustment.amount);
    (shipping_total - existing_adjustments).max(Decimal::ZERO)
}

pub fn promotion_metadata(
    metadata: Value,
    kind: CartPromotionKind,
    scope: &str,
    discount_percent: Option<Decimal>,
    fixed_amount: Option<Decimal>,
) -> Value {
    let mut metadata = match metadata {
        Value::Object(object) => object,
        _ => serde_json::Map::new(),
    };

    metadata.insert(
        "kind".to_string(),
        Value::from(match kind {
            CartPromotionKind::PercentageDiscount => "percentage_discount",
            CartPromotionKind::FixedDiscount => "fixed_discount",
        }),
    );
    metadata.insert("scope".to_string(), Value::from(scope));
    if let Some(discount_percent) = discount_percent {
        metadata.insert(
            "discount_percent".to_string(),
            Value::from(discount_percent.normalize().to_string()),
        );
    }
    if let Some(fixed_amount) = fixed_amount {
        metadata.insert(
            "fixed_amount".to_string(),
            Value::from(fixed_amount.normalize().to_string()),
        );
    }

    Value::Object(metadata)
}

pub fn adjustment_scope(adjustment: &entities::cart_adjustment::Model) -> Option<&str> {
    adjustment.metadata.get("scope").and_then(Value::as_str)
}

pub fn line_item_tax_class(metadata: &Value) -> Option<String> {
    metadata
        .get("tax_class")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn shipping_tax_class(metadata: &Value) -> Option<String> {
    metadata
        .get("tax_class")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn customer_tax_exempt(metadata: &Value) -> bool {
    metadata
        .get("customer_tax_exempt")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub async fn load_line_item_titles<C>(
    conn: &C,
    line_items: &[entities::cart_line_item::Model],
    preferred_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> CartResult<HashMap<Uuid, String>>
where
    C: sea_orm::ConnectionTrait,
{
    let mut titles = HashMap::new();
    if line_items.is_empty() {
        return Ok(titles);
    }

    let preferred_locale = preferred_locale.and_then(normalize_locale_tag);
    let fallback_locale = tenant_default_locale.and_then(normalize_locale_tag);
    let line_item_ids = line_items.iter().map(|item| item.id).collect::<Vec<_>>();
    let rows = entities::cart_line_item_translation::Entity::find()
        .filter(
            entities::cart_line_item_translation::Column::CartLineItemId
                .is_in(line_item_ids.clone()),
        )
        .all(conn)
        .await?;

    let mut rows_by_item = HashMap::<Uuid, Vec<entities::cart_line_item_translation::Model>>::new();
    for row in rows {
        rows_by_item
            .entry(row.cart_line_item_id)
            .or_default()
            .push(row);
    }

    for line_item in line_items {
        if let Some(title) = rows_by_item.remove(&line_item.id).and_then(|rows| {
            select_cart_line_item_title(
                &rows,
                preferred_locale.as_deref(),
                fallback_locale.as_deref(),
            )
        }) {
            titles.insert(line_item.id, title);
        }
    }

    Ok(titles)
}

pub fn select_cart_line_item_title(
    rows: &[entities::cart_line_item_translation::Model],
    preferred_locale: Option<&str>,
    fallback_locale: Option<&str>,
) -> Option<String> {
    let preferred_locale = preferred_locale.and_then(normalize_locale_tag);
    let fallback_locale = fallback_locale.and_then(normalize_locale_tag);

    preferred_locale
        .as_deref()
        .and_then(|preferred_locale| {
            rows.iter()
                .find(|row| normalize_locale_tag(&row.locale).as_deref() == Some(preferred_locale))
        })
        .or_else(|| {
            fallback_locale.as_deref().and_then(|fallback_locale| {
                rows.iter().find(|row| {
                    normalize_locale_tag(&row.locale).as_deref() == Some(fallback_locale)
                })
            })
        })
        .or_else(|| rows.first())
        .map(|row| row.title.clone())
}

pub async fn load_tenant_default_locale<C>(conn: &C, tenant_id: Uuid) -> CartResult<String>
where
    C: ConnectionTrait,
{
    let row = conn
        .query_one(Statement::from_sql_and_values(
            conn.get_database_backend(),
            "SELECT default_locale FROM tenants WHERE id = ?",
            vec![tenant_id.into()],
        ))
        .await?;

    let default_locale = row
        .and_then(|row| row.try_get::<String>("", "default_locale").ok())
        .as_deref()
        .map(normalize_locale_code)
        .transpose()?
        .unwrap_or_else(|| PLATFORM_FALLBACK_LOCALE.to_string());

    Ok(default_locale)
}

// Database helper functions relocated from cart.rs for modular service sizing

pub async fn load_cart<C>(
    conn: &C,
    tenant_id: Uuid,
    cart_id: Uuid,
) -> CartResult<entities::cart::Model>
where
    C: ConnectionTrait,
{
    load_cart_in_tx(conn, tenant_id, cart_id).await
}

pub async fn load_cart_in_tx<C>(
    conn: &C,
    tenant_id: Uuid,
    cart_id: Uuid,
) -> CartResult<entities::cart::Model>
where
    C: ConnectionTrait,
{
    entities::cart::Entity::find_by_id(cart_id)
        .filter(entities::cart::Column::TenantId.eq(tenant_id))
        .one(conn)
        .await?
        .ok_or(CartError::CartNotFound(cart_id))
}

pub async fn build_response<C>(conn: &C, cart: entities::cart::Model) -> CartResult<CartResponse>
where
    C: ConnectionTrait,
{
    let line_items = entities::cart_line_item::Entity::find()
        .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
        .order_by_asc(entities::cart_line_item::Column::CreatedAt)
        .all(conn)
        .await?;
    let tenant_default_locale = load_tenant_default_locale(conn, cart.tenant_id).await?;
    let title_map = load_line_item_titles(
        conn,
        &line_items,
        cart.locale_code.as_deref(),
        Some(tenant_default_locale.as_str()),
    )
    .await?;
    let adjustments = entities::cart_adjustment::Entity::find()
        .filter(entities::cart_adjustment::Column::CartId.eq(cart.id))
        .order_by_asc(entities::cart_adjustment::Column::CreatedAt)
        .all(conn)
        .await?;
    let tax_lines = entities::cart_tax_line::Entity::find()
        .filter(entities::cart_tax_line::Column::CartId.eq(cart.id))
        .order_by_asc(entities::cart_tax_line::Column::CreatedAt)
        .all(conn)
        .await?;
    let shipping_selections = entities::cart_shipping_selection::Entity::find()
        .filter(entities::cart_shipping_selection::Column::CartId.eq(cart.id))
        .all(conn)
        .await?;
    let subtotal_amount = subtotal_amount(&line_items);
    let adjustment_total = adjustment_total(&adjustments);
    let shipping_total = cart.shipping_total;
    let total_amount = cart.total_amount;
    let delivery_group_snapshots = collect_delivery_group_snapshots(&line_items);
    let selection_map = selection_map_from_records(&delivery_group_snapshots, shipping_selections);
    let delivery_groups = build_delivery_groups(&line_items, &selection_map);
    let selected_shipping_option_id = match delivery_groups.len() {
        0 => cart.selected_shipping_option_id,
        1 => delivery_groups[0].selected_shipping_option_id,
        _ => None,
    };

    Ok(CartResponse {
        id: cart.id,
        tenant_id: cart.tenant_id,
        channel_id: cart.channel_id,
        channel_slug: cart.channel_slug,
        customer_id: cart.customer_id,
        email: cart.email,
        region_id: cart.region_id,
        country_code: cart.country_code,
        locale_code: cart.locale_code,
        selected_shipping_option_id,
        status: cart.status,
        currency_code: cart.currency_code,
        subtotal_amount,
        adjustment_total,
        shipping_total,
        total_amount,
        tax_total: cart.tax_total,
        metadata: cart.metadata,
        created_at: cart.created_at.with_timezone(&Utc),
        updated_at: cart.updated_at.with_timezone(&Utc),
        completed_at: cart.completed_at.map(|value| value.with_timezone(&Utc)),
        line_items: line_items
            .into_iter()
            .map(|item| {
                let seller_id = seller_id_from_metadata(&item.metadata);
                CartLineItemResponse {
                    id: item.id,
                    cart_id: item.cart_id,
                    product_id: item.product_id,
                    variant_id: item.variant_id,
                    shipping_profile_slug: item.shipping_profile_slug,
                    seller_id,
                    seller_scope: None,
                    sku: item.sku,
                    title: title_map.get(&item.id).cloned().unwrap_or_default(),
                    quantity: item.quantity,
                    unit_price: item.unit_price,
                    total_price: item.total_price,
                    currency_code: item.currency_code,
                    metadata: item.metadata,
                    created_at: item.created_at.with_timezone(&Utc),
                    updated_at: item.updated_at.with_timezone(&Utc),
                }
            })
            .collect(),
        adjustments: adjustments
            .into_iter()
            .map(|adjustment| CartAdjustmentResponse {
                id: adjustment.id,
                cart_id: adjustment.cart_id,
                line_item_id: adjustment.cart_line_item_id,
                source_type: adjustment.source_type,
                source_id: adjustment.source_id,
                amount: adjustment.amount,
                currency_code: adjustment.currency_code,
                metadata: adjustment.metadata,
                created_at: adjustment.created_at.with_timezone(&Utc),
                updated_at: adjustment.updated_at.with_timezone(&Utc),
            })
            .collect(),
        tax_lines: tax_lines
            .into_iter()
            .map(|line| CartTaxLineResponse {
                id: line.id,
                cart_id: line.cart_id,
                line_item_id: line.cart_line_item_id,
                shipping_option_id: line.shipping_option_id,
                description: line.description,
                provider_id: line.provider_id,
                rate: line.rate,
                amount: line.amount,
                currency_code: line.currency_code,
                metadata: line.metadata,
                created_at: line.created_at.with_timezone(&Utc),
                updated_at: line.updated_at.with_timezone(&Utc),
            })
            .collect(),
        delivery_groups,
    })
}

pub async fn load_shipping_total<C>(
    conn: &C,
    cart: &entities::cart::Model,
    shipping_selections: &[entities::cart_shipping_selection::Model],
) -> CartResult<Decimal>
where
    C: ConnectionTrait,
{
    let shipping_option_ids = if shipping_selections.is_empty() {
        cart.selected_shipping_option_id
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        shipping_selections
            .iter()
            .filter_map(|selection| selection.selected_shipping_option_id)
            .collect::<Vec<_>>()
    };

    if shipping_option_ids.is_empty() {
        return Ok(Decimal::ZERO);
    }

    let options = shipping_option::Entity::find()
        .filter(shipping_option::Column::Id.is_in(shipping_option_ids))
        .all(conn)
        .await?;

    Ok(options
        .into_iter()
        .fold(Decimal::ZERO, |acc, option| acc + option.amount))
}

pub async fn recalculate_tax_lines<C>(
    conn: &C,
    tax_calculation_port: &dyn TaxCalculationPort,
    cart: &entities::cart::Model,
    line_items: &[entities::cart_line_item::Model],
    shipping_selections: &[entities::cart_shipping_selection::Model],
) -> CartResult<(Decimal, bool)>
where
    C: ConnectionTrait,
{
    entities::cart_tax_line::Entity::delete_many()
        .filter(entities::cart_tax_line::Column::CartId.eq(cart.id))
        .exec(conn)
        .await?;

    let Some(region_id) = cart.region_id else {
        return Ok((Decimal::ZERO, false));
    };

    let region = region::Entity::find_by_id(region_id)
        .filter(region::Column::TenantId.eq(cart.tenant_id))
        .one(conn)
        .await?
        .ok_or(CartError::Validation(
            "Region not found for cart".to_string(),
        ))?;
    let country_tax_policies = region_country_tax_policy::Entity::find()
        .filter(region_country_tax_policy::Column::RegionId.eq(region_id))
        .all(conn)
        .await?;
    let tax_rate = region.tax_rate;
    let now = Utc::now();
    let mut taxable_amounts = Vec::new();
    for item in line_items {
        if item.total_price <= Decimal::ZERO {
            continue;
        }
        taxable_amounts.push(TaxableAmount {
            line_item_id: Some(item.id),
            shipping_option_id: None,
            item_tax_class: line_item_tax_class(&item.metadata),
            shipping_tax_class: None,
            description: Some("line_item".to_string()),
            amount: item.total_price,
        });
    }

    for selection in shipping_selections {
        let Some(shipping_option_id) = selection.selected_shipping_option_id else {
            continue;
        };
        let option = shipping_option::Entity::find_by_id(shipping_option_id)
            .filter(shipping_option::Column::TenantId.eq(cart.tenant_id))
            .one(conn)
            .await?;
        let Some(option) = option else {
            continue;
        };
        if option.currency_code != cart.currency_code {
            continue;
        }
        if option.amount <= Decimal::ZERO {
            continue;
        }
        taxable_amounts.push(TaxableAmount {
            line_item_id: None,
            shipping_option_id: Some(option.id),
            item_tax_class: None,
            shipping_tax_class: shipping_tax_class(&option.metadata),
            description: Some("shipping".to_string()),
            amount: option.amount,
        });
    }

    let result = tax_calculation_port
        .calculate_tax(
            cart_tax_port_context(cart),
            TaxCalculationInput {
                currency_code: cart.currency_code.clone(),
                channel_id: cart.channel_id,
                customer_tax_exempt: customer_tax_exempt(&cart.metadata),
                policy: TaxPolicySnapshot {
                    provider_id: region.tax_provider_id.clone(),
                    channel_provider_id: channel_tax_provider_id(&region.metadata, cart.channel_id),
                    country_code: cart.country_code.clone(),
                    tax_rate,
                    tax_included: region.tax_included,
                    country_rules: country_tax_policies
                        .into_iter()
                        .map(|policy| TaxPolicyCountryRule {
                            country_code: policy.country_code,
                            tax_rate: policy.tax_rate,
                            tax_included: policy.tax_included,
                        })
                        .collect(),
                },
                taxable_amounts,
            },
        )
        .await
        .map_err(cart_tax_port_error)?;

    let tax_lines = result
        .lines
        .into_iter()
        .map(|line| entities::cart_tax_line::ActiveModel {
            id: Set(generate_id()),
            cart_id: Set(cart.id),
            cart_line_item_id: Set(line.line_item_id),
            shipping_option_id: Set(line.shipping_option_id),
            description: Set(line.description),
            provider_id: Set(line.provider_id),
            rate: Set(line.rate),
            amount: Set(line.amount),
            currency_code: Set(line.currency_code),
            metadata: Set(line.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        })
        .collect::<Vec<_>>();

    if !tax_lines.is_empty() {
        entities::cart_tax_line::Entity::insert_many(tax_lines)
            .exec(conn)
            .await?;
    }

    Ok((result.tax_total, result.tax_included))
}

pub async fn recalculate_totals<C>(
    conn: &C,
    tax_calculation_port: &dyn TaxCalculationPort,
    cart: entities::cart::Model,
) -> CartResult<()>
where
    C: ConnectionTrait,
{
    let line_items = entities::cart_line_item::Entity::find()
        .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
        .all(conn)
        .await?;
    let adjustments = entities::cart_adjustment::Entity::find()
        .filter(entities::cart_adjustment::Column::CartId.eq(cart.id))
        .all(conn)
        .await?;
    let shipping_selections = entities::cart_shipping_selection::Entity::find()
        .filter(entities::cart_shipping_selection::Column::CartId.eq(cart.id))
        .all(conn)
        .await?;
    let shipping_total = load_shipping_total(conn, &cart, &shipping_selections).await?;
    let (tax_total, tax_included) = recalculate_tax_lines(
        conn,
        tax_calculation_port,
        &cart,
        &line_items,
        &shipping_selections,
    )
    .await?;
    let subtotal = subtotal_amount(&line_items);
    let adjusted_total = net_total(subtotal, adjustment_total(&adjustments));
    let total_amount = if tax_included {
        adjusted_total + shipping_total
    } else {
        adjusted_total + shipping_total + tax_total
    };

    let mut active: entities::cart::ActiveModel = cart.into();
    active.shipping_total = Set(shipping_total);
    active.total_amount = Set(total_amount);
    active.tax_total = Set(tax_total);
    active.updated_at = Set(Utc::now().into());
    active.update(conn).await?;
    Ok(())
}

pub async fn apply_shipping_selection_patch<C>(
    conn: &C,
    cart: &entities::cart::Model,
    input: &UpdateCartContextInput,
) -> CartResult<()>
where
    C: ConnectionTrait,
{
    let line_items = entities::cart_line_item::Entity::find()
        .filter(entities::cart_line_item::Column::CartId.eq(cart.id))
        .all(conn)
        .await?;
    let available_group_snapshots = collect_delivery_group_snapshots(&line_items);
    let existing = entities::cart_shipping_selection::Entity::find()
        .filter(entities::cart_shipping_selection::Column::CartId.eq(cart.id))
        .all(conn)
        .await?;
    let mut desired = selection_map_from_records(&available_group_snapshots, existing);

    if let Some(shipping_selections) = &input.shipping_selections {
        desired.clear();
        for selection in shipping_selections {
            let normalized =
                normalize_shipping_profile_slug(Some(selection.shipping_profile_slug.as_str()));
            let normalized_seller_id = normalize_seller_id(selection.seller_id.as_deref());
            let matching_keys = matching_delivery_group_keys(
                &available_group_snapshots,
                normalized.as_str(),
                normalized_seller_id.as_deref(),
                None,
            );
            for key in matching_keys {
                desired.insert(key, selection.selected_shipping_option_id);
            }
        }
    } else if available_group_snapshots.len() <= 1 {
        if let Some(group) = available_group_snapshots.iter().next() {
            desired.insert(group.key.clone(), input.selected_shipping_option_id);
        } else {
            desired.clear();
        }
    } else if input.selected_shipping_option_id != cart.selected_shipping_option_id
        && input.selected_shipping_option_id.is_some()
    {
        return Err(CartError::Validation(
            "selected_shipping_option_id can only be used for carts with a single delivery group"
                .to_string(),
        ));
    }

    store_shipping_selections(conn, cart.id, desired).await?;
    reconcile_cart_shipping_state(conn, cart.id).await
}

pub async fn store_shipping_selections<C>(
    conn: &C,
    cart_id: Uuid,
    desired: BTreeMap<DeliveryGroupKey, Option<Uuid>>,
) -> CartResult<()>
where
    C: ConnectionTrait,
{
    let existing = entities::cart_shipping_selection::Entity::find()
        .filter(entities::cart_shipping_selection::Column::CartId.eq(cart_id))
        .all(conn)
        .await?;
    let existing_map = existing
        .into_iter()
        .map(|selection| {
            (
                DeliveryGroupKey {
                    shipping_profile_slug: selection.shipping_profile_slug.clone(),
                    seller_id: normalize_seller_id(selection.seller_id.as_deref()),
                    seller_scope: None,
                },
                selection,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let now = Utc::now();

    for (group_key, selected_shipping_option_id) in &desired {
        if let Some(current) = existing_map.get(group_key) {
            let mut active: entities::cart_shipping_selection::ActiveModel = current.clone().into();
            active.selected_shipping_option_id = Set(*selected_shipping_option_id);
            active.updated_at = Set(now.into());
            active.update(conn).await?;
        } else {
            entities::cart_shipping_selection::ActiveModel {
                id: Set(generate_id()),
                cart_id: Set(cart_id),
                shipping_profile_slug: Set(group_key.shipping_profile_slug.clone()),
                seller_id: Set(group_key.seller_id.clone()),
                seller_scope: Set(group_key.seller_scope.clone()),
                selected_shipping_option_id: Set(*selected_shipping_option_id),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            }
            .insert(conn)
            .await?;
        }
    }

    for (group_key, current) in existing_map {
        if !desired.contains_key(&group_key) {
            let active: entities::cart_shipping_selection::ActiveModel = current.into();
            active.delete(conn).await?;
        }
    }

    Ok(())
}

pub async fn reconcile_cart_shipping_state<C>(conn: &C, cart_id: Uuid) -> CartResult<()>
where
    C: ConnectionTrait,
{
    let cart = entities::cart::Entity::find_by_id(cart_id)
        .one(conn)
        .await?
        .ok_or(CartError::CartNotFound(cart_id))?;
    let line_items = entities::cart_line_item::Entity::find()
        .filter(entities::cart_line_item::Column::CartId.eq(cart_id))
        .order_by_asc(entities::cart_line_item::Column::CreatedAt)
        .all(conn)
        .await?;
    let delivery_group_snapshots = collect_delivery_group_snapshots(&line_items);
    let mut desired = entities::cart_shipping_selection::Entity::find()
        .filter(entities::cart_shipping_selection::Column::CartId.eq(cart_id))
        .all(conn)
        .await
        .map(|records| selection_map_from_records(&delivery_group_snapshots, records))?;

    if delivery_group_snapshots.len() == 1
        && desired.is_empty()
        && cart.selected_shipping_option_id.is_some()
        && !line_items.is_empty()
    {
        if let Some(group) = delivery_group_snapshots.iter().next() {
            desired.insert(group.key.clone(), cart.selected_shipping_option_id);
        }
    }

    store_shipping_selections(conn, cart_id, desired.clone()).await?;

    let legacy_selected_shipping_option_id = match delivery_group_snapshots.len() {
        0 => cart.selected_shipping_option_id,
        1 => delivery_group_snapshots
            .iter()
            .next()
            .and_then(|group| desired.get(&group.key).copied().flatten()),
        _ => None,
    };
    let mut active: entities::cart::ActiveModel = cart.into();
    active.selected_shipping_option_id = Set(legacy_selected_shipping_option_id);
    active.updated_at = Set(Utc::now().into());
    active.update(conn).await?;
    Ok(())
}

pub async fn replace_pricing_adjustments<C>(
    conn: &C,
    cart_id: Uuid,
    currency_code: &str,
    updates: Vec<(Uuid, Option<CartPricingAdjustmentUpdate>)>,
) -> CartResult<()>
where
    C: ConnectionTrait,
{
    if updates.is_empty() {
        return Ok(());
    }

    let line_item_ids = updates
        .iter()
        .map(|(line_item_id, _)| *line_item_id)
        .collect::<Vec<_>>();
    entities::cart_adjustment::Entity::delete_many()
        .filter(entities::cart_adjustment::Column::CartId.eq(cart_id))
        .filter(entities::cart_adjustment::Column::SourceType.eq(PRICING_ADJUSTMENT_SOURCE_TYPE))
        .filter(entities::cart_adjustment::Column::CartLineItemId.is_in(line_item_ids))
        .exec(conn)
        .await?;

    let now = Utc::now();
    for (line_item_id, adjustment) in updates {
        let Some(adjustment) = adjustment else {
            continue;
        };
        if adjustment.amount <= Decimal::ZERO {
            continue;
        }

        entities::cart_adjustment::ActiveModel {
            id: Set(generate_id()),
            cart_id: Set(cart_id),
            cart_line_item_id: Set(Some(line_item_id)),
            source_type: Set(PRICING_ADJUSTMENT_SOURCE_TYPE.to_string()),
            source_id: Set(normalize_adjustment_source_id(
                adjustment.source_id.as_deref(),
            )),
            amount: Set(adjustment.amount),
            currency_code: Set(currency_code.to_ascii_uppercase()),
            metadata: Set(sanitize_adjustment_metadata(adjustment.metadata)),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(conn)
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn channel_tax_provider_id_reads_string_mapping_for_channel() {
        let channel_id = Uuid::new_v4();
        let metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "provider_alpha"
            }
        });

        let resolved = channel_tax_provider_id(&metadata, Some(channel_id));
        assert_eq!(resolved.as_deref(), Some("provider_alpha"));
    }

    #[test]
    fn channel_tax_provider_id_ignores_blank_or_malformed_values() {
        let channel_id = Uuid::new_v4();

        let blank_metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): "   "
            }
        });
        assert_eq!(
            channel_tax_provider_id(&blank_metadata, Some(channel_id)),
            None
        );

        let typed_legacy_metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider": "external_tax"}
            }
        });
        assert_eq!(
            channel_tax_provider_id(&typed_legacy_metadata, Some(channel_id)).as_deref(),
            Some("external_tax")
        );

        let malformed_metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"unknown_key": "external_tax"}
            }
        });
        assert_eq!(
            channel_tax_provider_id(&malformed_metadata, Some(channel_id)),
            None
        );

        let typed_object_metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {"provider_id": "external_tax"}
            }
        });
        assert_eq!(
            channel_tax_provider_id(&typed_object_metadata, Some(channel_id)).as_deref(),
            Some("external_tax")
        );
    }

    #[test]
    fn channel_tax_provider_id_prefers_provider_id_over_provider_alias() {
        let channel_id = Uuid::new_v4();
        let metadata = json!({
            "channel_tax_provider_ids": {
                channel_id.to_string(): {
                    "provider_id": "region_default",
                    "provider": "external_tax"
                }
            }
        });

        assert_eq!(
            channel_tax_provider_id(&metadata, Some(channel_id)).as_deref(),
            Some("region_default")
        );
    }

    #[test]
    fn channel_tax_provider_id_returns_none_without_channel_context() {
        let metadata = json!({
            "channel_tax_provider_ids": {
                Uuid::new_v4().to_string(): "provider_alpha"
            }
        });

        assert_eq!(channel_tax_provider_id(&metadata, None), None);
    }

    #[test]
    fn tax_class_helpers_read_trimmed_tax_classes() {
        let metadata = json!({"tax_class": " standard "});
        assert_eq!(line_item_tax_class(&metadata).as_deref(), Some("standard"));
        assert_eq!(shipping_tax_class(&metadata).as_deref(), Some("standard"));
    }

    #[test]
    fn customer_tax_exempt_helper_defaults_to_false() {
        assert!(!customer_tax_exempt(&json!({})));
        assert!(!customer_tax_exempt(&json!({"customer_tax_exempt": "yes"})));
        assert!(customer_tax_exempt(&json!({"customer_tax_exempt": true})));
    }
}
