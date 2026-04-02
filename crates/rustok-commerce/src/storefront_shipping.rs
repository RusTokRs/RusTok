use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use uuid::Uuid;

use crate::{
    dto::CartResponse,
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
    metadata: &Value,
    required_profiles: &BTreeSet<String>,
) -> bool {
    if required_profiles.is_empty() {
        return true;
    }

    let Some(allowed_profiles) = extract_allowed_shipping_profile_slugs(metadata) else {
        return true;
    };

    required_profiles
        .iter()
        .all(|profile_slug| allowed_profiles.contains(profile_slug))
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

fn extract_allowed_shipping_profile_slugs(metadata: &Value) -> Option<BTreeSet<String>> {
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
