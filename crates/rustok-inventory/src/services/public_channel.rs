use std::collections::{BTreeSet, HashMap, HashSet};

use rustok_commerce_foundation::{
    entities::{inventory_item, inventory_level, product_variant, stock_location},
    error::{CommerceError, CommerceResult},
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::Value;
use uuid::Uuid;

pub fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

pub fn extract_allowed_channel_slugs(metadata: &Value) -> Vec<String> {
    let Some(values) = metadata
        .as_object()
        .and_then(|object| object.get("channel_visibility"))
        .and_then(|value| value.as_object())
        .and_then(|object| object.get("allowed_channel_slugs"))
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    let mut normalized = BTreeSet::new();
    for value in values {
        if let Some(slug) = value
            .as_str()
            .and_then(|value| normalize_public_channel_slug(Some(value)))
        {
            normalized.insert(slug);
        }
    }

    normalized.into_iter().collect()
}

pub fn is_allowlist_visible_for_public_channel(
    allowed_channel_slugs: &[String],
    public_channel_slug: Option<&str>,
) -> bool {
    if allowed_channel_slugs.is_empty() {
        return true;
    }

    let Some(public_channel_slug) = normalize_public_channel_slug(public_channel_slug) else {
        return false;
    };

    allowed_channel_slugs
        .iter()
        .any(|slug| slug == &public_channel_slug)
}

pub fn is_metadata_visible_for_public_channel(
    metadata: &Value,
    public_channel_slug: Option<&str>,
) -> bool {
    let allowed_channel_slugs = extract_allowed_channel_slugs(metadata);
    is_allowlist_visible_for_public_channel(&allowed_channel_slugs, public_channel_slug)
}

/// Validates request-level storefront inventory semantics before any DB lookup.
///
/// Returns `Ok(true)` when the variant policy allows the caller to skip an
/// availability lookup (for example, backorder/`continue` policy).
pub fn check_public_channel_inventory_request(
    inventory_policy: &str,
    requested_quantity: i32,
) -> CommerceResult<bool> {
    if requested_quantity < 0 {
        return Err(CommerceError::Validation(
            "requested inventory quantity must be non-negative".to_string(),
        ));
    }

    Ok(super::inventory_policy_allows_backorder(inventory_policy))
}

pub async fn check_variant_availability_for_public_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant: &product_variant::Model,
    requested_quantity: i32,
    public_channel_slug: Option<&str>,
) -> CommerceResult<bool> {
    if check_public_channel_inventory_request(&variant.inventory_policy, requested_quantity)? {
        return Ok(true);
    }

    let available_inventory = load_available_inventory_for_variant_in_public_channel(
        db,
        tenant_id,
        variant.id,
        public_channel_slug,
    )
    .await?;

    Ok(available_inventory >= requested_quantity)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicChannelInventoryProjection {
    pub available_quantity: i32,
    pub in_stock: bool,
}

pub fn public_channel_inventory_projection(
    available_quantity: i32,
    inventory_policy: &str,
) -> PublicChannelInventoryProjection {
    PublicChannelInventoryProjection {
        available_quantity,
        in_stock: available_quantity > 0
            || super::inventory_policy_allows_backorder(inventory_policy),
    }
}

pub async fn load_inventory_projection_by_variant_for_public_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant_policies: &[(Uuid, String)],
    public_channel_slug: Option<&str>,
) -> Result<HashMap<Uuid, PublicChannelInventoryProjection>, sea_orm::DbErr> {
    let variant_ids = variant_policies
        .iter()
        .map(|(variant_id, _)| *variant_id)
        .collect::<Vec<_>>();
    let available_by_variant = load_available_inventory_by_variant_for_public_channel(
        db,
        tenant_id,
        &variant_ids,
        public_channel_slug,
    )
    .await?;

    Ok(variant_policies
        .iter()
        .map(|(variant_id, inventory_policy)| {
            let available_quantity = available_by_variant.get(variant_id).copied().unwrap_or(0);
            (
                *variant_id,
                public_channel_inventory_projection(available_quantity, inventory_policy),
            )
        })
        .collect())
}

pub async fn load_available_inventory_by_variant_for_public_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant_ids: &[Uuid],
    public_channel_slug: Option<&str>,
) -> Result<HashMap<Uuid, i32>, sea_orm::DbErr> {
    if variant_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let inventory_items = inventory_item::Entity::find()
        .filter(inventory_item::Column::VariantId.is_in(variant_ids.iter().copied()))
        .all(db)
        .await?;
    if inventory_items.is_empty() {
        return Ok(HashMap::new());
    }

    let item_to_variant: HashMap<Uuid, Uuid> = inventory_items
        .iter()
        .map(|item| (item.id, item.variant_id))
        .collect();

    let levels = inventory_level::Entity::find()
        .filter(inventory_level::Column::InventoryItemId.is_in(item_to_variant.keys().copied()))
        .all(db)
        .await?;
    if levels.is_empty() {
        return Ok(HashMap::new());
    }

    let locations = stock_location::Entity::find()
        .filter(stock_location::Column::TenantId.eq(tenant_id))
        .filter(stock_location::Column::DeletedAt.is_null())
        .filter(stock_location::Column::Id.is_in(levels.iter().map(|level| level.location_id)))
        .all(db)
        .await?;

    let visible_location_ids: HashSet<Uuid> = locations
        .into_iter()
        .filter(|location| {
            is_metadata_visible_for_public_channel(&location.metadata, public_channel_slug)
        })
        .map(|location| location.id)
        .collect();

    let mut available_by_variant = HashMap::new();
    for level in levels {
        if !visible_location_ids.contains(&level.location_id) {
            continue;
        }

        if let Some(variant_id) = item_to_variant.get(&level.inventory_item_id) {
            *available_by_variant.entry(*variant_id).or_insert(0) +=
                level.stocked_quantity - level.reserved_quantity;
        }
    }

    Ok(available_by_variant)
}

pub async fn load_available_inventory_for_variant_in_public_channel(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    variant_id: Uuid,
    public_channel_slug: Option<&str>,
) -> Result<i32, sea_orm::DbErr> {
    Ok(load_available_inventory_by_variant_for_public_channel(
        db,
        tenant_id,
        &[variant_id],
        public_channel_slug,
    )
    .await?
    .get(&variant_id)
    .copied()
    .unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::{
        check_public_channel_inventory_request, extract_allowed_channel_slugs,
        is_allowlist_visible_for_public_channel, is_metadata_visible_for_public_channel,
        normalize_public_channel_slug, public_channel_inventory_projection,
    };

    #[test]
    fn normalize_public_channel_slug_trims_and_lowercases() {
        assert_eq!(
            normalize_public_channel_slug(Some(" Web-Store ")).as_deref(),
            Some("web-store")
        );
        assert_eq!(normalize_public_channel_slug(Some("   ")), None);
        assert_eq!(normalize_public_channel_slug(None), None);
    }

    #[test]
    fn extract_allowed_channel_slugs_normalizes_and_deduplicates() {
        let metadata = serde_json::json!({
            "channel_visibility": {
                "allowed_channel_slugs": [" Web ", "mobile", "web", "", null]
            }
        });

        assert_eq!(
            extract_allowed_channel_slugs(&metadata),
            vec!["mobile".to_string(), "web".to_string()]
        );
    }

    #[test]
    fn allowlist_visibility_requires_matching_public_channel() {
        let allowlist = vec!["web".to_string()];

        assert!(is_allowlist_visible_for_public_channel(
            &allowlist,
            Some("web")
        ));
        assert!(!is_allowlist_visible_for_public_channel(
            &allowlist,
            Some("mobile")
        ));
        assert!(!is_allowlist_visible_for_public_channel(&allowlist, None));
    }

    #[test]
    fn empty_allowlist_is_visible_for_every_public_channel() {
        assert!(is_allowlist_visible_for_public_channel(&[], Some("web")));
        assert!(is_allowlist_visible_for_public_channel(&[], None));
    }

    #[test]
    fn metadata_visibility_uses_normalized_allowlist() {
        let metadata = serde_json::json!({
            "channel_visibility": {
                "allowed_channel_slugs": ["Web"]
            }
        });

        assert!(is_metadata_visible_for_public_channel(
            &metadata,
            Some(" web ")
        ));
        assert!(!is_metadata_visible_for_public_channel(
            &metadata,
            Some("mobile")
        ));
        assert!(!is_metadata_visible_for_public_channel(&metadata, None));
    }

    #[test]
    fn public_channel_inventory_request_rejects_negative_quantity() {
        let error = check_public_channel_inventory_request("continue", -1)
            .expect_err("negative quantities must fail before policy checks");

        assert!(error
            .to_string()
            .contains("requested inventory quantity must be non-negative"));
    }

    #[test]
    fn public_channel_inventory_request_skips_lookup_for_backorder_policy() {
        assert!(check_public_channel_inventory_request(" CONTINUE ", 0)
            .expect("backorderable request should be valid"));
        assert!(!check_public_channel_inventory_request("deny", 0)
            .expect("deny-policy zero request should still be valid"));
    }
}
