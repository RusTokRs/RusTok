//! Product buyer state is a projection of persisted sparse tuples and Pricing observations.
//! No Cartesian SKUs, monetary resolver, reservation, or customer/list eligibility lives here.

use std::collections::{BTreeMap, BTreeSet};

use rustok_pricing_storefront::{normalize_pricing_context, parse_pricing_route_quantity};
use rustok_ui_core::{encode_ui_query_pairs, UiRouteContext};
use serde::{Deserialize, Serialize};

use super::{build_fetch_request, pricing_quote_matches_request, FetchRequest, RouteInput};
use crate::i18n::t;
use crate::model::{PricingResolutionContext, ProductDetail, ProductPricingDetail, ProductVariant};

const MAX_VARIANTS: usize = 256;
const MAX_AXES: usize = 32;
const MAX_OPTIONS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuyerError {
    Projection,
    Selection,
    Request,
    Quote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuyerInput {
    pub values: BTreeMap<String, String>,
    pub quantity_text: String,
    pub request: FetchRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyerQuoteRequest {
    pub product_id: String,
    pub variant_id: String,
    pub variant_ids: Vec<String>,
    pub handle: String,
    pub locale: Option<String>,
    pub pricing: PricingResolutionContext,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuyerQuoteCycle {
    pub input: u64,
    pub scope: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProductBuyerSnapshot {
    pub tenant_slug: String,
    pub product: ProductDetail,
    pub route: UiRouteContext,
    pub input: RouteInput,
    pub handle: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuyerQuoteObservation {
    pub request: BuyerQuoteRequest,
    pub cycle: BuyerQuoteCycle,
    pub quote: Option<ProductPricingDetail>,
    pub failed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuyerAxisOptionView {
    pub value: String,
    pub label: String,
    pub disabled: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuyerAxisView {
    pub attribute_id: String,
    pub label: String,
    pub value: String,
    pub options: Vec<BuyerAxisOptionView>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuyerView {
    pub axes: Vec<BuyerAxisView>,
    pub currencies: Vec<String>,
    pub selected_sku: Option<String>,
    pub price: Option<String>,
    pub observed_at: Option<String>,
    pub status: String,
    pub can_purchase: bool,
    pub quantity_valid: bool,
    pub preview: bool,
    pub unavailable: bool,
    pub request: Option<BuyerQuoteRequest>,
}

fn uuid(value: &str) -> Result<String, BuyerError> {
    let id = uuid::Uuid::parse_str(value.trim()).map_err(|_| BuyerError::Selection)?;
    if id.is_nil() {
        return Err(BuyerError::Selection);
    }
    Ok(id.to_string())
}

fn tuple(variant: &ProductVariant) -> BTreeMap<String, String> {
    variant
        .axis_values
        .iter()
        .map(|value| (value.attribute_id.clone(), value.option_id.clone()))
        .collect()
}

/// Refuse incomplete/oversized projections, never truncate them into a different catalog.
pub fn validate_product(product: &ProductDetail) -> Result<(), BuyerError> {
    let invalid = || BuyerError::Projection;
    if product.variant_axes.len() > MAX_AXES
        || product.variants.is_empty()
        || product.variants.len() > MAX_VARIANTS
    {
        return Err(invalid());
    }
    if uuid(&product.id).map_err(|_| invalid())? != product.id {
        return Err(invalid());
    }
    let mut axes = BTreeMap::new();
    let mut config_ids = BTreeSet::new();
    for axis in &product.variant_axes {
        let id = uuid(&axis.attribute_id).map_err(|_| invalid())?;
        if id != axis.attribute_id
            || uuid(&axis.id).map_err(|_| invalid())? != axis.id
            || !config_ids.insert(uuid(&axis.id).map_err(|_| invalid())?)
            || axis.allowed_values.is_empty()
            || axis.allowed_values.len() > MAX_OPTIONS
        {
            return Err(invalid());
        }
        let options = axis
            .allowed_values
            .iter()
            .map(|option| uuid(&option.option_id))
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(|_| invalid())?;
        if options.len() != axis.allowed_values.len()
            || axis
                .allowed_values
                .iter()
                .any(|option| !options.contains(&option.option_id))
            || axes.insert(id, options).is_some()
        {
            return Err(invalid());
        }
    }
    let mut variant_ids = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for variant in &product.variants {
        let id = uuid(&variant.id).map_err(|_| invalid())?;
        if id != variant.id
            || !variant_ids.insert(id)
            || variant.axis_values.len() != axes.len()
            || !matches!(variant.inventory_policy.as_str(), "deny" | "continue")
        {
            return Err(invalid());
        }
        let values = tuple(variant);
        if values.len() != axes.len()
            || values.iter().any(|(axis, option)| {
                !axes
                    .get(axis)
                    .is_some_and(|allowed| allowed.contains(option))
            })
        {
            return Err(invalid());
        }
        let identity = values
            .iter()
            .map(|(axis, option)| format!("{axis}:{option}"))
            .collect::<Vec<_>>()
            .join(";");
        let stored_identity = (!identity.is_empty()).then_some(identity.clone());
        if variant.combination_identity != stored_identity || !identities.insert(identity) {
            return Err(invalid());
        }
    }
    if axes.is_empty() && product.variants.len() != 1 {
        return Err(invalid());
    }
    Ok(())
}

pub fn buyer_currencies(product: &ProductDetail) -> Vec<String> {
    product
        .variants
        .iter()
        .flat_map(|variant| &variant.prices)
        .filter_map(|price| {
            rustok_pricing_storefront::parse_optional_currency_code(Some(
                price.currency_code.clone(),
            ))
            .ok()
            .flatten()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn selected_variant<'a>(
    product: &'a ProductDetail,
    values: &BTreeMap<String, String>,
) -> Option<&'a ProductVariant> {
    if values.len() != product.variant_axes.len() {
        return None;
    }
    product
        .variants
        .iter()
        .find(|variant| tuple(variant) == *values)
}

pub fn build_input(
    product: &ProductDetail,
    route: &UiRouteContext,
    input: &RouteInput,
    handle: &str,
) -> Result<BuyerInput, BuyerError> {
    validate_product(product)?;
    let mut values = BTreeMap::new();
    for (key, value) in &route.query {
        if let Some(axis) = key.strip_prefix("axis.") {
            let axis = uuid(axis)?;
            let value = uuid(value)?;
            if !product.variant_axes.iter().any(|candidate| {
                candidate.attribute_id == axis
                    && candidate
                        .allowed_values
                        .iter()
                        .any(|option| option.option_id == value)
            }) || values.insert(axis, value).is_some()
            {
                return Err(BuyerError::Selection);
            }
        }
    }
    if let Some(id) = route.query.get("variant_id") {
        let id = uuid(id)?;
        let variant = product
            .variants
            .iter()
            .find(|variant| variant.id == id)
            .ok_or(BuyerError::Selection)?;
        let requested_tuple = tuple(variant);
        if values
            .iter()
            .any(|(axis, value)| requested_tuple.get(axis) != Some(value))
        {
            return Err(BuyerError::Selection);
        }
        values = requested_tuple;
    }
    let mut request = build_fetch_request(input);
    request.selected_handle = Some(handle.to_string());
    let currencies = buyer_currencies(product);
    if request.currency_code.is_none() && currencies.len() == 1 {
        request.currency_code = Some(currencies[0].clone());
    }
    Ok(BuyerInput {
        values,
        request,
        quantity_text: input.quantity.unwrap_or(1).to_string(),
    })
}

pub fn pick_axis(
    product: &ProductDetail,
    input: &BuyerInput,
    axis: &str,
    option: &str,
) -> Result<BuyerInput, BuyerError> {
    let config = product
        .variant_axes
        .iter()
        .find(|candidate| candidate.attribute_id == axis)
        .ok_or(BuyerError::Selection)?;
    if !option.is_empty()
        && !config
            .allowed_values
            .iter()
            .any(|value| value.option_id == option)
    {
        return Err(BuyerError::Selection);
    }
    let mut next = input.clone();
    if option.is_empty() {
        next.values.remove(axis);
    } else {
        next.values.insert(axis.to_string(), option.to_string());
    }
    Ok(next)
}

pub fn change_currency(input: &BuyerInput, currency: String) -> BuyerInput {
    let mut next = input.clone();
    next.request.currency_code = (!currency.is_empty()).then_some(currency);
    next.request.region_id = None;
    next.request.price_list_id = None;
    next
}

pub fn change_quantity(input: &BuyerInput, quantity_text: String) -> BuyerInput {
    let mut next = input.clone();
    next.quantity_text = quantity_text;
    next
}

pub fn ordinary_context(input: &BuyerInput) -> BuyerInput {
    let mut next = input.clone();
    next.request.price_list_id = None;
    next
}

pub fn use_cart_context(
    input: &BuyerInput,
    context: &rustok_cart_storefront::model::StorefrontCart,
) -> BuyerInput {
    let mut next = input.clone();
    next.request.currency_code = Some(context.currency_code.clone());
    next.request.region_id = context.region_id.clone();
    next.request.price_list_id = None;
    next.request.channel_id = context.channel_id.clone();
    next.request.channel_slug = context.channel_slug.clone();
    if let Some(locale) = context.locale_code.as_ref() {
        next.request.locale = Some(locale.clone());
    }
    next
}

pub fn quote_request(
    product: &ProductDetail,
    input: &BuyerInput,
) -> Result<Option<BuyerQuoteRequest>, BuyerError> {
    let quantity = parse_pricing_route_quantity(Some(&input.quantity_text))
        .map_err(|_| BuyerError::Request)?;
    let Some(variant) = selected_variant(product, &input.values) else {
        return Ok(None);
    };
    let Some(currency) = input.request.currency_code.clone() else {
        return Ok(None);
    };
    let pricing = normalize_pricing_context(
        Some(currency),
        input.request.region_id.clone(),
        input.request.price_list_id.clone(),
        input.request.channel_id.clone(),
        input.request.channel_slug.clone(),
        quantity,
    )
    .map_err(|_| BuyerError::Request)?
    .ok_or(BuyerError::Request)?;
    Ok(Some(BuyerQuoteRequest {
        product_id: product.id.clone(),
        variant_id: variant.id.clone(),
        variant_ids: product
            .variants
            .iter()
            .map(|variant| variant.id.clone())
            .collect(),
        handle: input
            .request
            .selected_handle
            .clone()
            .ok_or(BuyerError::Request)?,
        locale: input.request.locale.clone(),
        pricing,
    }))
}

pub fn validate_quote_request(request: &BuyerQuoteRequest) -> Result<(), BuyerError> {
    if request.handle.trim().is_empty()
        || request.handle.len() > 255
        || request.variant_ids.is_empty()
        || request.variant_ids.len() > MAX_VARIANTS
        || uuid(&request.product_id)? != request.product_id
        || uuid(&request.variant_id)? != request.variant_id
        || !request.variant_ids.contains(&request.variant_id)
        || request.locale.as_deref().is_some_and(|locale| {
            rustok_api::normalize_locale_tag(locale).is_none() || locale.len() > 64
        })
    {
        return Err(BuyerError::Request);
    }
    let ids = request
        .variant_ids
        .iter()
        .map(|id| uuid(id))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if ids.len() != request.variant_ids.len()
        || request.variant_ids.iter().any(|id| !ids.contains(id))
    {
        return Err(BuyerError::Request);
    }
    let pricing = &request.pricing;
    let canonical = normalize_pricing_context(
        Some(pricing.currency_code.clone()),
        pricing.region_id.clone(),
        pricing.price_list_id.clone(),
        pricing.channel_id.clone(),
        pricing.channel_slug.clone(),
        Some(pricing.quantity),
    )
    .map_err(|_| BuyerError::Request)?
    .ok_or(BuyerError::Request)?;
    if canonical != *pricing {
        return Err(BuyerError::Request);
    }
    Ok(())
}

pub fn assert_quote(
    product: &ProductDetail,
    request: &BuyerQuoteRequest,
    quote: &ProductPricingDetail,
) -> Result<(), BuyerError> {
    let ids = quote
        .variants
        .iter()
        .map(|variant| variant.id.as_str())
        .collect::<BTreeSet<_>>();
    if quote.id != product.id
        || request.product_id != product.id
        || ids.len() != quote.variants.len()
        || ids.len() != product.variants.len()
        || product
            .variants
            .iter()
            .any(|variant| !ids.contains(variant.id.as_str()))
        || !pricing_quote_matches_request(quote, Some(&request.pricing))
    {
        return Err(BuyerError::Quote);
    }
    Ok(())
}

pub fn observe_quote(
    product: &ProductDetail,
    request: BuyerQuoteRequest,
    cycle: BuyerQuoteCycle,
    result: Result<ProductPricingDetail, String>,
) -> BuyerQuoteObservation {
    match result.and_then(|quote| {
        assert_quote(product, &request, &quote)
            .map(|()| quote)
            .map_err(|_| "Quote identity mismatch".to_string())
    }) {
        Ok(quote) => BuyerQuoteObservation {
            request,
            cycle,
            quote: Some(quote),
            failed: false,
        },
        Err(_) => BuyerQuoteObservation {
            request,
            cycle,
            quote: None,
            failed: true,
        },
    }
}

pub fn build_view(
    product: &ProductDetail,
    input: &BuyerInput,
    cycle: BuyerQuoteCycle,
    observation: Option<&BuyerQuoteObservation>,
    locale: Option<&str>,
) -> BuyerView {
    let quantity = parse_pricing_route_quantity(Some(&input.quantity_text))
        .ok()
        .flatten();
    let variant = selected_variant(product, &input.values);
    let request_result = quote_request(product, input);
    let request_invalid = request_result.is_err();
    let request = request_result.unwrap_or(None);
    let observation = observation.filter(|observation| {
        observation.cycle == cycle && Some(&observation.request) == request.as_ref()
    });
    let quote = observation
        .and_then(|observation| observation.quote.as_ref())
        .filter(|quote| {
            request
                .as_ref()
                .is_some_and(|request| assert_quote(product, request, quote).is_ok())
        });
    let price = quote
        .and_then(|quote| {
            variant.and_then(|variant| quote.variants.iter().find(|price| price.id == variant.id))
        })
        .and_then(|variant| variant.effective_price.as_ref());
    let preview = quote
        .and_then(|quote| quote.resolution_context.as_ref())
        .is_some_and(|context| context.price_list_id.is_some());
    let has_stock = variant.zip(quantity).is_some_and(|(variant, quantity)| {
        variant.in_stock
            && (variant.inventory_policy == "continue" || variant.inventory_quantity >= quantity)
    });
    let status = if quantity.is_none() {
        "invalidQuantity"
    } else if input.values.len() != product.variant_axes.len() {
        "chooseAxes"
    } else if variant.is_none() {
        "missingTuple"
    } else if input.request.currency_code.is_none() {
        "chooseCurrency"
    } else if request_invalid {
        "requestError"
    } else if observation.is_none() {
        "loading"
    } else if quote.is_none() {
        "unavailable"
    } else if price.is_none() {
        "unpriced"
    } else if preview {
        "preview"
    } else if !has_stock {
        "stock"
    } else {
        "ready"
    };
    let mut currencies = buyer_currencies(product);
    if let Some(currency) = input.request.currency_code.as_ref() {
        if !currencies.contains(currency) {
            currencies.push(currency.clone());
            currencies.sort();
        }
    }
    BuyerView {
        axes: axis_views(product, &input.values),
        currencies,
        selected_sku: variant
            .map(|variant| variant.sku.clone().unwrap_or_else(|| variant.id.clone())),
        price: price.map(|price| {
            super::format_product_price(
                locale,
                &price.currency_code,
                &price.amount,
                price.compare_at_amount.as_deref(),
                price.discount_percent.as_deref(),
            )
        }),
        observed_at: quote.and_then(|quote| quote.resolved_at.clone()),
        status: t(
            locale,
            &format!("product.buyer.{status}"),
            "The selection or price is not available.",
        ),
        can_purchase: status == "ready",
        quantity_valid: quantity.is_some(),
        preview,
        unavailable: status == "unavailable",
        request,
    }
}

pub fn axis_views(
    product: &ProductDetail,
    values: &BTreeMap<String, String>,
) -> Vec<BuyerAxisView> {
    let mut axes = product.variant_axes.iter().collect::<Vec<_>>();
    axes.sort_by_key(|axis| (axis.position, &axis.attribute_id));
    axes.into_iter()
        .map(|axis| {
            let available = product
                .variants
                .iter()
                .filter(|variant| {
                    variant.axis_values.iter().all(|value| {
                        value.attribute_id == axis.attribute_id
                            || values
                                .get(&value.attribute_id)
                                .is_none_or(|picked| picked == &value.option_id)
                    })
                })
                .flat_map(|variant| {
                    variant
                        .axis_values
                        .iter()
                        .filter(|value| value.attribute_id == axis.attribute_id)
                        .map(|value| value.option_id.as_str())
                })
                .collect::<BTreeSet<_>>();
            let mut options = axis.allowed_values.iter().collect::<Vec<_>>();
            options.sort_by_key(|option| (option.position, &option.option_id));
            BuyerAxisView {
                attribute_id: axis.attribute_id.clone(),
                label: axis.name.clone(),
                value: values.get(&axis.attribute_id).cloned().unwrap_or_default(),
                options: options
                    .into_iter()
                    .map(|option| BuyerAxisOptionView {
                        value: option.option_id.clone(),
                        label: option.value.clone(),
                        disabled: !available.contains(option.option_id.as_str()),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// Preserve unrelated host query state. Only an existing complete tuple receives a SKU UUID.
pub fn selection_href(
    base: &str,
    route: &UiRouteContext,
    product: &ProductDetail,
    input: &BuyerInput,
) -> String {
    let mut query = route.query.clone();
    query.retain(|key, _| key != "variant_id" && !key.starts_with("axis."));
    if let Some(variant) = selected_variant(product, &input.values) {
        query.insert("variant_id".into(), variant.id.clone());
    } else {
        for (axis, option) in &input.values {
            query.insert(format!("axis.{axis}"), option.clone());
        }
    }
    for (key, value) in [
        ("currency", input.request.currency_code.clone()),
        ("region_id", input.request.region_id.clone()),
        ("price_list_id", input.request.price_list_id.clone()),
        ("channel_id", input.request.channel_id.clone()),
        ("channel_slug", input.request.channel_slug.clone()),
    ] {
        if let Some(value) = value {
            query.insert(key.into(), value);
        } else {
            query.remove(key);
        }
    }
    query.insert("quantity".into(), input.quantity_text.clone());
    if let Some(handle) = input.request.selected_handle.as_ref() {
        query.insert("handle".into(), handle.clone());
    }
    let pairs = query
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    format!("{base}?{}", encode_ui_query_pairs(&pairs))
}

pub fn error_message(error: BuyerError, locale: Option<&str>) -> String {
    let key = if error == BuyerError::Projection {
        "projectionError"
    } else {
        "requestError"
    };
    t(
        locale,
        &format!("product.buyer.{key}"),
        "The product selection was refused. Reset the selection before continuing.",
    )
}

pub fn snapshot(
    product: ProductDetail,
    route: UiRouteContext,
    handle: String,
    tenant_slug: String,
) -> Result<ProductBuyerSnapshot, BuyerError> {
    let input = route_input(&route).map_err(|_| BuyerError::Request)?;
    build_input(&product, &route, &input, &handle)?;
    if tenant_slug.trim().is_empty() || tenant_slug.len() > 255 {
        return Err(BuyerError::Projection);
    }
    Ok(ProductBuyerSnapshot {
        product,
        route,
        input,
        handle,
        tenant_slug,
    })
}

pub fn route_input(
    route: &UiRouteContext,
) -> Result<RouteInput, rustok_pricing_storefront::StorefrontPricingQueryError> {
    let get = |key| route.query.get(key).cloned();
    super::build_route_input(
        get("handle"),
        route.locale.clone(),
        get("currency"),
        get("region_id"),
        get("price_list_id"),
        get("channel_id"),
        get("channel_slug"),
        get("quantity"),
    )
}

/// A snapshot contains public catalog data and query state only, never credentials or capabilities.
pub fn encode_snapshot(snapshot: &ProductBuyerSnapshot) -> Result<String, BuyerError> {
    let json = serde_json::to_string(snapshot).map_err(|_| BuyerError::Projection)?;
    if json.len() > 2 * 1024 * 1024 {
        return Err(BuyerError::Projection);
    }
    Ok(json)
}
pub fn decode_snapshot(json: &str) -> Result<ProductBuyerSnapshot, BuyerError> {
    if json.len() > 2 * 1024 * 1024 {
        return Err(BuyerError::Projection);
    }
    let snapshot: ProductBuyerSnapshot =
        serde_json::from_str(json).map_err(|_| BuyerError::Projection)?;
    if snapshot.tenant_slug.trim().is_empty() || snapshot.tenant_slug.len() > 255 {
        return Err(BuyerError::Projection);
    }
    let expected = route_input(&snapshot.route).map_err(|_| BuyerError::Projection)?;
    if expected != snapshot.input {
        return Err(BuyerError::Projection);
    }
    build_input(
        &snapshot.product,
        &snapshot.route,
        &snapshot.input,
        &snapshot.handle,
    )?;
    Ok(snapshot)
}

pub fn selection_with_cart_href(
    base: &str,
    route: &UiRouteContext,
    product: &ProductDetail,
    input: &BuyerInput,
    cart_id: Option<String>,
) -> String {
    let mut route = route.clone();
    route.query.remove("cart_id");
    if let Some(id) = cart_id {
        route.query.insert("cart_id".into(), id);
    }
    selection_href(base, &route, product, input)
}

pub fn cart_context_href(
    route: &UiRouteContext,
    product: &ProductDetail,
    input: &BuyerInput,
    cart: &rustok_cart_storefront::model::StorefrontCart,
) -> Option<String> {
    rustok_api::normalize_locale_tag(cart.locale_code.as_deref()?)?;
    let next = use_cart_context(input, cart);
    let mut route = route.clone();
    route.locale = next.request.locale.clone();
    let base = route.module_route_base(&super::resolve_route_segment(
        route.route_segment.as_deref(),
    ));
    Some(selection_with_cart_href(
        &base,
        &route,
        product,
        &next,
        Some(cart.id.clone()),
    ))
}

pub fn cart_href(route: &UiRouteContext, cart_id: &str) -> String {
    let base = route.module_route_base("cart");
    format!("{base}?{}", encode_ui_query_pairs(&[("cart_id", cart_id)]))
}

pub fn reset_href(route: &UiRouteContext) -> String {
    let base = route.module_route_base(&super::resolve_route_segment(
        route.route_segment.as_deref(),
    ));
    let pairs = route
        .query
        .iter()
        .filter(|(key, _)| {
            !matches!(
                key.as_str(),
                "variant_id"
                    | "quantity"
                    | "currency"
                    | "region_id"
                    | "price_list_id"
                    | "channel_id"
                    | "channel_slug"
            ) && !key.starts_with("axis.")
        })
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    format!("{base}?{}", encode_ui_query_pairs(&pairs))
}

#[derive(Clone, Debug)]
pub struct BuyerLabels {
    pub quantity: String,
    pub currency: String,
    pub not_selected: String,
    pub sku: String,
    pub add: String,
    pub retry: String,
    pub reset: String,
    pub use_cart_context: String,
    pub ordinary: String,
    pub selection_link: String,
    pub inspect_cart: String,
    pub reset_cart: String,
    pub observation: String,
    pub javascript: String,
    pub link_error: String,
    pub cart_locale: String,
}
pub fn labels(locale: Option<&str>) -> BuyerLabels {
    let text = |key: &str, fallback: &str| t(locale, &format!("product.buyer.{key}"), fallback);
    BuyerLabels {
        quantity: text("quantity", "Quantity"),
        currency: text("currency", "Currency"),
        not_selected: text("notSelected", "Not selected"),
        sku: text("sku", "SKU"),
        add: text("add", "Add to cart"),
        retry: text("retry", "Request a new quote"),
        reset: text("reset", "Reset selection"),
        use_cart_context: text("useCartContext", "Use cart context and quote again"),
        ordinary: text("ordinary", "Request an ordinary quote"),
        selection_link: text("selectionLink", "Link to this selection"),
        inspect_cart: text("inspectCart", "Inspect cart"),
        reset_cart: text(
            "resetCart",
            "Reset cart reference (does not delete the cart)",
        ),
        observation: text("observation", "Price observed at"),
        javascript: text(
            "javascript",
            "Interactive SKU selection requires the buyer client. If it cannot start, purchases remain disabled.",
        ),
        cart_locale: text(
            "cartLocale",
            "This cart has no verifiable locale. Inspect it or explicitly reset its local reference before continuing.",
        ),
        link_error: text(
            "linkError",
            "The selection link could not be saved. Purchases are disabled; reload this page before continuing.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(value: u32) -> String {
        format!("00000000-0000-4000-8000-{value:012}")
    }
    fn product() -> ProductDetail {
        let axis = |attribute, a, b| {
            serde_json::json!({"id": id(attribute + 100), "attributeId": id(attribute), "name": "Axis", "position": 0,
            "allowedValues": [{"optionId": id(a), "value": "A", "position": 0}, {"optionId": id(b), "value": "B", "position": 1}] })
        };
        let variant = |sku, a, b| {
            serde_json::json!({"id": id(sku), "title": "SKU", "inventoryPolicy": "deny", "inventoryQuantity": 4, "inStock": true, "prices": [],
            "axisValues": [{"attributeId": id(10), "optionId": id(a)}, {"attributeId": id(20), "optionId": id(b)}],
            "combinationIdentity": format!("{}:{};{}:{}", id(10), id(a), id(20), id(b)) })
        };
        serde_json::from_value(serde_json::json!({"id": id(1), "status": "published", "tags": [], "translations": [],
            "variantAxes": [axis(10,11,12), axis(20,21,22)], "variants": [variant(3,11,21), variant(4,12,22)] })).unwrap()
    }
    #[test]
    fn sparse_tuples_and_snapshot_route_identity_are_not_inferred_or_repaired() {
        let product = product();
        validate_product(&product).unwrap();
        assert!(selected_variant(&product, &BTreeMap::new()).is_none());
        let missing = BTreeMap::from([(id(10), id(11)), (id(20), id(22))]);
        assert!(selected_variant(&product, &missing).is_none());
        let full = BTreeMap::from([(id(10), id(11)), (id(20), id(21))]);
        assert_eq!(
            selected_variant(&product, &full).map(|variant| &variant.id),
            Some(&id(3))
        );
        let route = UiRouteContext {
            locale: Some("en".into()),
            query: BTreeMap::from([
                ("variant_id".into(), id(3)),
                ("quantity".into(), "2".into()),
            ]),
            ..Default::default()
        };
        let mut snapshot = snapshot(product, route, "boots".into(), "shop".into()).unwrap();
        let encoded = encode_snapshot(&snapshot).unwrap();
        assert!(decode_snapshot(&encoded).is_ok());
        snapshot.input.quantity = Some(3);
        assert!(decode_snapshot(&encode_snapshot(&snapshot).unwrap()).is_err());
        snapshot.tenant_slug.clear();
        assert!(decode_snapshot(&encode_snapshot(&snapshot).unwrap()).is_err());
    }
}
