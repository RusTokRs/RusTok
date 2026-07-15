use uuid::Uuid;

use crate::core::routing::normalize_channel_value;
use crate::model::{
    PricingDiscountDraft, PricingPrice, PricingPriceDraft, PricingPriceListRuleDraft,
    PricingPriceListScopeDraft, PricingResolutionContext,
};
use rustok_ui_core::normalize_ui_text;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PricingAdminRequestError {
    message: String,
}

impl PricingAdminRequestError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PricingAdminRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PricingAdminRequestError {}

pub(crate) fn text_or_none(value: String) -> Option<String> {
    normalize_ui_text(value.as_str())
}

pub(crate) fn normalized_currency_code(value: String) -> Option<String> {
    let value = text_or_none(value)?;
    if value.len() == 3 {
        Some(value.to_ascii_uppercase())
    } else {
        None
    }
}

pub(crate) fn normalized_region_id(value: String) -> Option<String> {
    let value = text_or_none(value)?;
    Uuid::parse_str(value.as_str()).ok().map(|_| value)
}

pub(crate) fn normalized_price_list_id(value: String) -> Option<String> {
    let value = text_or_none(value)?;
    Uuid::parse_str(value.as_str()).ok().map(|_| value)
}

pub(crate) fn normalized_quantity(value: String) -> Option<i32> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|quantity| *quantity > 0)
}

pub(crate) fn parse_optional_currency_code(
    currency_code: Option<String>,
) -> Result<Option<String>, PricingAdminRequestError> {
    let Some(currency_code) = currency_code.and_then(text_or_none) else {
        return Ok(None);
    };
    let normalized = currency_code.to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(PricingAdminRequestError::new(
            "currency_code must be a 3-letter code",
        ));
    }

    Ok(Some(normalized))
}

pub(crate) fn parse_optional_uuid_string(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<String>, PricingAdminRequestError> {
    let Some(value) = value.and_then(text_or_none) else {
        return Ok(None);
    };

    Uuid::parse_str(value.as_str())
        .map(|_| Some(value))
        .map_err(|_| PricingAdminRequestError::new(format!("Invalid {field_name}")))
}

pub(crate) fn parse_resolution_quantity(
    quantity: Option<i32>,
) -> Result<i32, PricingAdminRequestError> {
    match quantity {
        Some(value) if value < 1 => {
            Err(PricingAdminRequestError::new("quantity must be at least 1"))
        }
        Some(value) => Ok(value),
        None => Ok(1),
    }
}

pub(crate) fn sanitize_channel_slug(channel_slug: Option<String>) -> Option<String> {
    channel_slug
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

pub(crate) fn sanitize_resolution_context(
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingResolutionContext>, PricingAdminRequestError> {
    let requires_currency = region_id
        .as_ref()
        .and_then(|value| text_or_none(value.clone()))
        .is_some()
        || price_list_id
            .as_ref()
            .and_then(|value| text_or_none(value.clone()))
            .is_some()
        || quantity.is_some();
    let Some(currency_code) = parse_optional_currency_code(currency_code)? else {
        if requires_currency {
            return Err(PricingAdminRequestError::new(
                "currency_code is required for pricing resolution context",
            ));
        }
        return Ok(None);
    };

    Ok(Some(PricingResolutionContext {
        currency_code,
        region_id: parse_optional_uuid_string(region_id, "region_id")?,
        price_list_id: parse_optional_uuid_string(price_list_id, "price_list_id")?,
        channel_id: None,
        channel_slug: None,
        quantity: parse_resolution_quantity(quantity)?,
    }))
}

pub(crate) fn build_product_admin_href(module_route_base: &str, product_id: &str) -> String {
    format!("{module_route_base}?id={product_id}")
}

pub(crate) fn build_resolution_context(
    currency_value: String,
    region_value: String,
    price_list_value: String,
    channel_id_value: String,
    channel_slug_value: String,
    quantity_value: String,
) -> Option<PricingResolutionContext> {
    let currency_code = normalized_currency_code(currency_value)?;
    Some(PricingResolutionContext {
        currency_code,
        region_id: normalized_region_id(region_value),
        price_list_id: normalized_price_list_id(price_list_value),
        channel_id: normalize_channel_value(&channel_id_value),
        channel_slug: normalize_channel_value(&channel_slug_value),
        quantity: normalized_quantity(quantity_value).unwrap_or(1),
    })
}

pub(crate) fn price_draft_from_price(price: PricingPrice) -> PricingPriceDraft {
    PricingPriceDraft {
        currency_code: price.currency_code,
        amount: price.amount,
        compare_at_amount: price.compare_at_amount.unwrap_or_default(),
        price_list_id: price.price_list_id.unwrap_or_default(),
        channel_id: price.channel_id.unwrap_or_default(),
        channel_slug: price.channel_slug.unwrap_or_default(),
        min_quantity: price
            .min_quantity
            .map(|value| value.to_string())
            .unwrap_or_default(),
        max_quantity: price
            .max_quantity
            .map(|value| value.to_string())
            .unwrap_or_default(),
    }
}

pub(crate) fn empty_price_draft(
    currency_code: String,
    selected_price_list_id: Option<String>,
) -> PricingPriceDraft {
    PricingPriceDraft {
        currency_code,
        amount: String::new(),
        compare_at_amount: String::new(),
        price_list_id: selected_price_list_id.unwrap_or_default(),
        channel_id: String::new(),
        channel_slug: String::new(),
        min_quantity: String::new(),
        max_quantity: String::new(),
    }
}

pub(crate) struct PriceDraftForm {
    pub currency_code: String,
    pub amount: String,
    pub compare_at_amount: String,
    pub price_list_id: String,
    pub channel_id: String,
    pub channel_slug: String,
    pub min_quantity: String,
    pub max_quantity: String,
}

pub(crate) fn build_price_draft(form: PriceDraftForm) -> PricingPriceDraft {
    PricingPriceDraft {
        currency_code: form.currency_code,
        amount: form.amount,
        compare_at_amount: form.compare_at_amount,
        price_list_id: form.price_list_id,
        channel_id: form.channel_id,
        channel_slug: form.channel_slug,
        min_quantity: form.min_quantity,
        max_quantity: form.max_quantity,
    }
}

pub(crate) fn build_discount_draft(
    currency_code: String,
    discount_percent: String,
    selected_price_list_id: Option<String>,
    channel_id: String,
    channel_slug: String,
) -> PricingDiscountDraft {
    PricingDiscountDraft {
        currency_code,
        discount_percent,
        price_list_id: selected_price_list_id.unwrap_or_default(),
        channel_id,
        channel_slug,
    }
}

pub(crate) fn build_price_list_rule_draft(adjustment_percent: String) -> PricingPriceListRuleDraft {
    PricingPriceListRuleDraft { adjustment_percent }
}

pub(crate) fn clear_price_list_rule_draft() -> PricingPriceListRuleDraft {
    build_price_list_rule_draft(String::new())
}

pub(crate) fn build_price_list_scope_draft(
    channel_id: String,
    channel_slug: String,
) -> PricingPriceListScopeDraft {
    PricingPriceListScopeDraft {
        channel_id,
        channel_slug,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_resolution_context_normalizes_admin_inputs() {
        let context = build_resolution_context(
            " usd ".to_string(),
            "not-a-uuid".to_string(),
            "550e8400-e29b-41d4-a716-446655440000".to_string(),
            " channel-id ".to_string(),
            " channel-slug ".to_string(),
            "0".to_string(),
        )
        .expect("valid currency should build a context");

        assert_eq!(context.currency_code, "USD");
        assert_eq!(context.region_id, None);
        assert_eq!(
            context.price_list_id.as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        assert_eq!(context.channel_id.as_deref(), Some("channel-id"));
        assert_eq!(context.channel_slug.as_deref(), Some("channel-slug"));
        assert_eq!(context.quantity, 1);
    }

    #[test]
    fn sanitize_resolution_context_requires_currency_for_context_fields() {
        let error = sanitize_resolution_context(
            None,
            Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
            None,
            None,
        )
        .expect_err("region-scoped context without currency should fail");

        assert_eq!(
            error.to_string(),
            "currency_code is required for pricing resolution context"
        );

        let context = sanitize_resolution_context(Some(" usd ".to_string()), None, None, Some(2))
            .expect("valid context should parse")
            .expect("quantity requires an explicit context");

        assert_eq!(context.currency_code, "USD");
        assert_eq!(context.quantity, 2);
    }

    #[test]
    fn parse_optional_uuid_string_trims_and_validates_values() {
        assert_eq!(
            parse_optional_uuid_string(
                Some(" 550e8400-e29b-41d4-a716-446655440000 ".to_string()),
                "price_list_id",
            )
            .expect("valid UUID should parse")
            .as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );

        let error = parse_optional_uuid_string(Some("not-a-uuid".to_string()), "price_list_id")
            .expect_err("invalid UUID should fail");
        assert_eq!(error.to_string(), "Invalid price_list_id");
    }

    #[test]
    fn draft_builders_keep_request_policy_out_of_render_adapter() {
        let price_draft = price_draft_from_price(PricingPrice {
            currency_code: "usd".to_string(),
            amount: "12.50".to_string(),
            compare_at_amount: Some("15.00".to_string()),
            discount_percent: None,
            on_sale: true,
            price_list_id: Some("price-list".to_string()),
            channel_id: Some("channel-id".to_string()),
            channel_slug: Some("web".to_string()),
            min_quantity: Some(2),
            max_quantity: Some(5),
        });

        assert_eq!(price_draft.currency_code, "usd");
        assert_eq!(price_draft.compare_at_amount, "15.00");
        assert_eq!(price_draft.price_list_id, "price-list");
        assert_eq!(price_draft.channel_id, "channel-id");
        assert_eq!(price_draft.channel_slug, "web");
        assert_eq!(price_draft.min_quantity, "2");
        assert_eq!(price_draft.max_quantity, "5");

        let discount_draft = build_discount_draft(
            "EUR".to_string(),
            "10".to_string(),
            Some("list-1".to_string()),
            "channel-id".to_string(),
            "web".to_string(),
        );
        assert_eq!(discount_draft.price_list_id, "list-1");
        assert_eq!(discount_draft.channel_slug, "web");

        let scope_draft = build_price_list_scope_draft("channel-id".to_string(), "web".to_string());
        assert_eq!(scope_draft.channel_id, "channel-id");
        assert_eq!(scope_draft.channel_slug, "web");

        assert_eq!(
            clear_price_list_rule_draft().adjustment_percent,
            String::new()
        );
    }
}
