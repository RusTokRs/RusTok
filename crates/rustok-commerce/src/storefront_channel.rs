use std::collections::{BTreeSet, HashMap, HashSet};

use rustok_api::RequestContext;
use rustok_channel::{error::ChannelError, ChannelService};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    dto::ProductResponse,
    entities::{inventory_item, inventory_level, stock_location},
};

pub(crate) async fn is_module_enabled_for_request_channel(
    db: &DatabaseConnection,
    request_context: &RequestContext,
    module_slug: &str,
) -> Result<bool, ChannelError> {
    let Some(channel_id) = request_context.channel_id else {
        return Ok(true);
    };

    ChannelService::new(db.clone())
        .is_module_enabled(channel_id, module_slug)
        .await
}

pub(crate) fn public_channel_slug_from_request(request_context: &RequestContext) -> Option<String> {
    normalize_public_channel_slug(request_context.channel_slug.as_deref())
}

pub(crate) fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

pub(crate) fn extract_allowed_channel_slugs(metadata: &Value) -> Vec<String> {
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

pub(crate) fn is_allowlist_visible_for_public_channel(
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

pub(crate) fn is_metadata_visible_for_public_channel(
    metadata: &Value,
    public_channel_slug: Option<&str>,
) -> bool {
    let allowed_channel_slugs = extract_allowed_channel_slugs(metadata);
    is_allowlist_visible_for_public_channel(&allowed_channel_slugs, public_channel_slug)
}

pub(crate) async fn load_available_inventory_by_variant_for_public_channel(
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

pub(crate) async fn load_available_inventory_for_variant_in_public_channel(
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

pub(crate) async fn apply_public_channel_inventory_to_product(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product: &mut ProductResponse,
    public_channel_slug: Option<&str>,
) -> Result<(), sea_orm::DbErr> {
    let variant_ids = product
        .variants
        .iter()
        .map(|variant| variant.id)
        .collect::<Vec<_>>();
    let available_by_variant = load_available_inventory_by_variant_for_public_channel(
        db,
        tenant_id,
        &variant_ids,
        public_channel_slug,
    )
    .await?;

    for variant in &mut product.variants {
        let available_inventory = available_by_variant.get(&variant.id).copied().unwrap_or(0);
        variant.inventory_quantity = available_inventory;
        variant.in_stock = available_inventory > 0 || variant.inventory_policy == "continue";
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        extract_allowed_channel_slugs, is_allowlist_visible_for_public_channel,
        is_metadata_visible_for_public_channel, normalize_public_channel_slug,
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
    fn metadata_visibility_defaults_to_public_when_no_allowlist_exists() {
        let unrestricted = serde_json::json!({});
        let restricted = serde_json::json!({
            "channel_visibility": {
                "allowed_channel_slugs": ["web"]
            }
        });

        assert!(is_metadata_visible_for_public_channel(&unrestricted, None));
        assert!(is_metadata_visible_for_public_channel(
            &restricted,
            Some("web")
        ));
        assert!(!is_metadata_visible_for_public_channel(
            &restricted,
            Some("mobile")
        ));
    }
}
