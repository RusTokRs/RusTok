//! Buyer command construction. A displayed amount is deliberately absent from these DTOs.

use rustok_pricing_storefront::{
    PricingResolutionContext, normalize_pricing_context, parse_optional_currency_code,
    parse_optional_uuid_string,
};
use serde::{Deserialize, Serialize};

use super::CartCoreError;
use crate::model::StorefrontCart;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CartCreateRequest {
    pub currency_code: String,
    pub region_id: Option<String>,
    pub locale: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CartAddVariantRequest {
    pub cart_id: String,
    pub variant_id: String,
    pub quantity: i32,
    pub expected_context: PricingResolutionContext,
    pub locale: Option<String>,
}

fn invalid() -> CartCoreError {
    CartCoreError::Validation("A valid cart, SKU and ordinary price context are required".into())
}

pub fn validate_cart_create(request: &CartCreateRequest) -> Result<(), CartCoreError> {
    parse_optional_currency_code(Some(request.currency_code.clone())).map_err(|_| invalid())?;
    parse_optional_uuid_string(request.region_id.clone(), "region_id").map_err(|_| invalid())?;
    if request.locale.as_deref().is_some_and(|locale| {
        rustok_api::normalize_locale_tag(locale).is_none() || locale.len() > 64
    }) {
        return Err(invalid());
    }
    Ok(())
}

pub fn validate_cart_add(request: &CartAddVariantRequest) -> Result<(), CartCoreError> {
    parse_optional_uuid_string(Some(request.cart_id.clone()), "cart_id").map_err(|_| invalid())?;
    validate_buyer_selection(
        &request.variant_id,
        request.quantity,
        &request.expected_context,
        request.locale.as_deref(),
    )
}

pub(super) fn validate_buyer_selection(
    variant_id: &str,
    quantity: i32,
    expected: &PricingResolutionContext,
    locale: Option<&str>,
) -> Result<(), CartCoreError> {
    parse_optional_uuid_string(Some(variant_id.to_string()), "variant_id")
        .map_err(|_| invalid())?;
    normalize_pricing_context(
        Some(expected.currency_code.clone()),
        expected.region_id.clone(),
        expected.price_list_id.clone(),
        expected.channel_id.clone(),
        expected.channel_slug.clone(),
        Some(expected.quantity),
    )
    .map_err(|_| invalid())?;
    if locale.is_some_and(|locale| {
        rustok_api::normalize_locale_tag(locale).is_none() || locale.len() > 64
    }) || quantity < 1
        || expected.quantity != quantity
        || expected.price_list_id.is_some()
    {
        return Err(invalid());
    }
    Ok(())
}

pub fn create_cart_request(
    context: &PricingResolutionContext,
    locale: Option<String>,
) -> Result<CartCreateRequest, CartCoreError> {
    if context.price_list_id.is_some() {
        return Err(invalid());
    }
    let request = CartCreateRequest {
        currency_code: context.currency_code.clone(),
        region_id: context.region_id.clone(),
        locale,
    };
    validate_cart_create(&request)?;
    Ok(request)
}

pub fn add_variant_request(
    cart_id: String,
    variant_id: String,
    context: PricingResolutionContext,
    locale: Option<String>,
) -> Result<CartAddVariantRequest, CartCoreError> {
    let request = CartAddVariantRequest {
        cart_id,
        variant_id,
        quantity: context.quantity,
        expected_context: context,
        locale,
    };
    validate_cart_add(&request)?;
    Ok(request)
}

/// Null region/channel selectors are real context, not wildcard discount/eligibility requests.
pub fn buyer_cart_context_matches(
    cart: &StorefrontCart,
    context: &PricingResolutionContext,
    locale: Option<&str>,
) -> bool {
    cart.currency_code
        .eq_ignore_ascii_case(&context.currency_code)
        && cart.region_id == context.region_id
        && cart
            .channel_id
            .as_ref()
            .is_none_or(|id| context.channel_id.as_ref() == Some(id))
        && cart.channel_slug.as_ref().is_none_or(|slug| {
            context
                .channel_slug
                .as_ref()
                .is_some_and(|actual| actual.eq_ignore_ascii_case(slug))
        })
        && (cart.channel_id.is_some()
            || cart.channel_slug.is_some()
            || (context.channel_id.is_none() && context.channel_slug.is_none()))
        && context.price_list_id.is_none()
        && match (cart.locale_code.as_deref(), locale) {
            (Some(cart), Some(requested)) => rustok_api::locale_tags_match(cart, requested),
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(_), None) => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> PricingResolutionContext {
        PricingResolutionContext {
            currency_code: "EUR".into(),
            region_id: None,
            price_list_id: None,
            channel_id: None,
            channel_slug: None,
            quantity: 2,
        }
    }

    #[test]
    fn buyer_commands_have_no_displayed_money_or_list_eligibility() {
        let cart = uuid::Uuid::new_v4().to_string();
        let sku = uuid::Uuid::new_v4().to_string();
        assert!(
            add_variant_request(cart.clone(), sku.clone(), context(), Some("en".into())).is_ok()
        );
        let mut preview = context();
        preview.price_list_id = Some(uuid::Uuid::new_v4().to_string());
        assert!(
            add_variant_request(
                cart.clone(),
                sku.clone(),
                preview.clone(),
                Some("en".into())
            )
            .is_err()
        );
        assert!(create_cart_request(&preview, Some("en".into())).is_err());
        let mut invalid = context();
        invalid.quantity = 0;
        assert!(
            add_variant_request(cart.clone(), sku.clone(), invalid, Some("en".into())).is_err()
        );
        assert!(
            add_variant_request(
                "00000000-0000-0000-0000-000000000000".into(),
                sku,
                context(),
                Some("en".into())
            )
            .is_err()
        );
    }
}
