use rustok_api::RequestContext;
use rustok_channel::{error::ChannelError, ChannelService};
use rustok_inventory::{
    is_metadata_visible_for_public_channel as inventory_metadata_visible_for_public_channel,
    load_inventory_projection_by_variant_for_public_channel,
    normalize_public_channel_slug as inventory_normalize_public_channel_slug,
    PublicChannelInventoryVariantProjectionInput,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::dto::ProductResponse;

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

pub(crate) fn normalize_public_channel_slug(value: Option<&str>) -> Option<String> {
    inventory_normalize_public_channel_slug(value)
}

pub(crate) fn is_metadata_visible_for_public_channel(
    metadata: &serde_json::Value,
    public_channel_slug: Option<&str>,
) -> bool {
    inventory_metadata_visible_for_public_channel(metadata, public_channel_slug)
}

pub(crate) fn public_channel_slug_from_request(request_context: &RequestContext) -> Option<String> {
    normalize_public_channel_slug(request_context.channel_slug.as_deref())
}

pub(crate) async fn apply_public_channel_inventory_to_product(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    product: &mut ProductResponse,
    public_channel_slug: Option<&str>,
) -> Result<(), sea_orm::DbErr> {
    let variants = product
        .variants
        .iter()
        .map(|variant| PublicChannelInventoryVariantProjectionInput {
            variant_id: variant.id,
            inventory_policy: variant.inventory_policy.as_str(),
        })
        .collect::<Vec<_>>();
    let projections = load_inventory_projection_by_variant_for_public_channel(
        db,
        tenant_id,
        &variants,
        public_channel_slug,
    )
    .await?;

    for variant in &mut product.variants {
        let Some(projection) = projections.get(&variant.id) else {
            continue;
        };
        variant.inventory_quantity = projection.available_quantity;
        variant.in_stock = projection.in_stock;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rustok_inventory::{
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
