use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::Value;
use std::collections::BTreeSet;
use uuid::Uuid;

use crate::{
    dto::{CartResponse, CartShippingOptionSummary, ShippingOptionResponse},
    entities::{product, product_variant},
    CommerceResult, FulfillmentService,
};

const DEFAULT_SHIPPING_PROFILE_SLUG: &str = "default";

pub fn normalize_shipping_profile_slug(value: &str) -> Option<String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub fn shipping_profile_slug_from_product_metadata(metadata: &Value) -> String {
    metadata
        .get("shipping_profile")
        .and_then(|profile| profile.get("slug"))
        .and_then(Value::as_str)
        .and_then(normalize_shipping_profile_slug)
        .or_else(|| {
            metadata
                .get("shipping_profile_slug")
                .and_then(Value::as_str)
                .and_then(normalize_shipping_profile_slug)
        })
        .unwrap_or_else(|| DEFAULT_SHIPPING_PROFILE_SLUG.to_string())
}

pub fn product_shipping_profile_slug(
    product_shipping_profile_slug: Option<&str>,
    product_metadata: &Value,
) -> String {
    product_shipping_profile_slug
        .and_then(normalize_shipping_profile_slug)
        .unwrap_or_else(|| shipping_profile_slug_from_product_metadata(product_metadata))
}

pub fn effective_shipping_profile_slug(
    product_default_shipping_profile_slug: Option<&str>,
    product_metadata: &Value,
    variant_shipping_profile_slug: Option<&str>,
) -> String {
    variant_shipping_profile_slug
        .and_then(normalize_shipping_profile_slug)
        .unwrap_or_else(|| {
            product_shipping_profile_slug(product_default_shipping_profile_slug, product_metadata)
        })
}

pub fn is_shipping_option_compatible_with_profiles(
    option: &ShippingOptionResponse,
    required_profiles: &BTreeSet<String>,
) -> bool {
    if required_profiles.is_empty() {
        return true;
    }

    let Some(allowed_profiles) = allowed_shipping_profile_slugs_from_option(option) else {
        return true;
    };

    required_profiles
        .iter()
        .all(|profile_slug| allowed_profiles.contains(profile_slug))
}

fn allowed_shipping_profile_slugs_from_option(
    option: &ShippingOptionResponse,
) -> Option<BTreeSet<String>> {
    option
        .allowed_shipping_profile_slugs
        .as_ref()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| normalize_shipping_profile_slug(value))
                .collect()
        })
        .or_else(|| extract_allowed_shipping_profile_slugs_from_metadata(&option.metadata))
}

pub async fn load_cart_shipping_profile_slugs(
    _db: &DatabaseConnection,
    _tenant_id: Uuid,
    cart: &CartResponse,
) -> CommerceResult<BTreeSet<String>> {
    Ok(cart
        .line_items
        .iter()
        .filter_map(|item| normalize_shipping_profile_slug(item.shipping_profile_slug.as_str()))
        .collect())
}

pub async fn load_current_shipping_profile_slug_for_line_item(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product_id: Option<Uuid>,
    variant_id: Option<Uuid>,
) -> CommerceResult<String> {
    let Some(variant_id) = variant_id else {
        if let Some(product_id) = product_id {
            let product = product::Entity::find_by_id(product_id)
                .filter(product::Column::TenantId.eq(tenant_id))
                .one(db)
                .await?;
            return Ok(product
                .map(|product| {
                    product_shipping_profile_slug(
                        product.shipping_profile_slug.as_deref(),
                        &product.metadata,
                    )
                })
                .unwrap_or_else(|| DEFAULT_SHIPPING_PROFILE_SLUG.to_string()));
        }
        return Ok(DEFAULT_SHIPPING_PROFILE_SLUG.to_string());
    };

    let Some(variant) = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?
    else {
        return Ok(DEFAULT_SHIPPING_PROFILE_SLUG.to_string());
    };
    let product_id = product_id.unwrap_or(variant.product_id);
    let product = product::Entity::find_by_id(product_id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;

    Ok(product
        .map(|product| {
            effective_shipping_profile_slug(
                product.shipping_profile_slug.as_deref(),
                &product.metadata,
                variant.shipping_profile_slug.as_deref(),
            )
        })
        .unwrap_or_else(|| DEFAULT_SHIPPING_PROFILE_SLUG.to_string()))
}

pub fn map_shipping_option_summary(option: &ShippingOptionResponse) -> CartShippingOptionSummary {
    CartShippingOptionSummary {
        id: option.id,
        name: option.name.clone(),
        currency_code: option.currency_code.clone(),
        amount: option.amount,
        provider_id: option.provider_id.clone(),
        active: option.active,
        metadata: option.metadata.clone(),
    }
}

pub async fn enrich_cart_delivery_groups(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    mut cart: CartResponse,
    public_channel_slug: Option<&str>,
) -> CommerceResult<CartResponse> {
    let mut options = FulfillmentService::new(db.clone())
        .list_shipping_options(tenant_id)
        .await
        .map_err(|err| crate::CommerceError::Validation(err.to_string()))?;
    options.retain(|option| {
        option
            .currency_code
            .eq_ignore_ascii_case(&cart.currency_code)
    });
    options.retain(|option| {
        crate::storefront_channel::is_metadata_visible_for_public_channel(
            &option.metadata,
            public_channel_slug,
        )
    });

    for delivery_group in &mut cart.delivery_groups {
        let required_profiles = BTreeSet::from([delivery_group.shipping_profile_slug.clone()]);
        delivery_group.available_shipping_options = options
            .iter()
            .filter(|option| {
                is_shipping_option_compatible_with_profiles(option, &required_profiles)
            })
            .map(map_shipping_option_summary)
            .collect();
    }
    cart.selected_shipping_option_id = if cart.delivery_groups.len() == 1 {
        cart.delivery_groups[0].selected_shipping_option_id
    } else {
        None
    };

    Ok(cart)
}

fn extract_allowed_shipping_profile_slugs_from_metadata(
    metadata: &Value,
) -> Option<BTreeSet<String>> {
    metadata
        .get("shipping_profiles")
        .and_then(|profiles| profiles.get("allowed_slugs"))
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .filter_map(normalize_shipping_profile_slug)
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use super::{effective_shipping_profile_slug, is_shipping_option_compatible_with_profiles};
    use crate::dto::ShippingOptionResponse;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::collections::BTreeSet;
    use uuid::Uuid;

    #[test]
    fn shipping_option_compatibility_uses_typed_allowed_profiles() {
        let option = ShippingOptionResponse {
            id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            name: "Bulky Freight".to_string(),
            currency_code: "EUR".to_string(),
            amount: Decimal::new(2999, 2),
            provider_id: "manual".to_string(),
            active: true,
            allowed_shipping_profile_slugs: Some(vec![" bulky ".to_string()]),
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let required_profiles = BTreeSet::from([String::from("bulky")]);

        assert!(is_shipping_option_compatible_with_profiles(
            &option,
            &required_profiles,
        ));
    }

    #[test]
    fn effective_shipping_profile_prefers_variant_then_product_then_default() {
        let product_metadata = serde_json::json!({
            "shipping_profile": { "slug": "bulky" }
        });

        assert_eq!(
            effective_shipping_profile_slug(Some("cold-chain"), &product_metadata, Some("frozen")),
            "frozen"
        );
        assert_eq!(
            effective_shipping_profile_slug(Some("cold-chain"), &product_metadata, None),
            "cold-chain"
        );
        assert_eq!(
            effective_shipping_profile_slug(None, &product_metadata, None),
            "bulky"
        );
        assert_eq!(
            effective_shipping_profile_slug(None, &serde_json::json!({}), None),
            "default"
        );
    }
}
