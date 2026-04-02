use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

use crate::{
    dto::{CartResponse, ShippingOptionResponse},
    entities::{product, product_variant},
    CommerceResult,
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
    db: &DatabaseConnection,
    tenant_id: Uuid,
    cart: &CartResponse,
) -> CommerceResult<BTreeSet<String>> {
    let variant_ids: Vec<Uuid> = cart
        .line_items
        .iter()
        .filter_map(|item| item.variant_id)
        .collect();
    let variant_to_product = if variant_ids.is_empty() {
        HashMap::new()
    } else {
        product_variant::Entity::find()
            .filter(product_variant::Column::TenantId.eq(tenant_id))
            .filter(product_variant::Column::Id.is_in(variant_ids))
            .all(db)
            .await?
            .into_iter()
            .map(|variant| (variant.id, variant.product_id))
            .collect::<HashMap<_, _>>()
    };

    let product_ids = cart
        .line_items
        .iter()
        .filter_map(|item| {
            item.variant_id
                .and_then(|variant_id| variant_to_product.get(&variant_id).copied())
                .or(item.product_id)
        })
        .collect::<BTreeSet<_>>();
    if product_ids.is_empty() {
        return Ok(BTreeSet::new());
    }

    let products = product::Entity::find()
        .filter(product::Column::TenantId.eq(tenant_id))
        .filter(product::Column::Id.is_in(product_ids))
        .all(db)
        .await?;

    Ok(products
        .into_iter()
        .map(|product| shipping_profile_slug_from_product_metadata(&product.metadata))
        .collect())
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
    use super::is_shipping_option_compatible_with_profiles;
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
}
