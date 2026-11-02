use rustok_api::RequestContext;
use rustok_channel::{ChannelService, error::ChannelError};
use rustok_inventory::{
    is_metadata_visible_for_public_channel as inventory_metadata_visible_for_public_channel,
    normalize_public_channel_slug as inventory_normalize_public_channel_slug,
};
use sea_orm::DatabaseConnection;

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

#[cfg(test)]
mod tests {
    use rustok_inventory::{
        extract_allowed_channel_slugs, is_allowlist_visible_for_public_channel,
        is_metadata_visible_for_public_channel, normalize_public_channel_slug,
    };

    #[tokio::test]
    async fn foreign_saved_cart_refuses_before_channel_owner_io() {
        let (mut cart, _) = crate::services::checkout_catalog::tests::fixture();
        cart.channel_id = Some(uuid::Uuid::from_u128(100));
        let context = rustok_api::PortContext::new(
            uuid::Uuid::from_u128(200).to_string(),
            rustok_api::PortActor::service("checkout-scope-test"),
            "en-GB",
            "checkout-scope-test",
        )
        .with_deadline(std::time::Duration::from_secs(2));
        // No Channel schema: reaching the owner lookup would be a storage failure, not this
        // exact admission refusal. The test must not rely on a seeded default channel.
        let db = sea_orm::Database::connect("sqlite::memory:").await.unwrap();
        let error = super::resolve_cart_channel(&db, &context, &cart)
            .await
            .unwrap_err();
        assert_eq!(error.code, "commerce.checkout_cart_scope_invalid");
    }

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

/// Native admission uses the same tenant module state and Channel owner as public Store APIs.
/// Context extraction is the adapter's responsibility; rejection never becomes an anonymous scope.
pub async fn admit_storefront_buyer_request(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    request: &rustok_api::RequestContext,
) -> Result<(), PortError> {
    if tenant_id.is_nil() || request.tenant_id != tenant_id {
        return Err(PortError::forbidden(
            "commerce.cart_tenant_mismatch",
            "Storefront request scope was refused",
        ));
    }
    let enabled = rustok_api::is_tenant_module_enabled(db, tenant_id, "commerce")
        .await
        .map_err(|_| {
            PortError::unavailable(
                "commerce.cart_module_state_unavailable",
                "Storefront availability could not be established",
            )
        })?;
    let channel_enabled = is_module_enabled_for_request_channel(db, request, "commerce")
        .await
        .map_err(|_| {
            PortError::unavailable(
                "commerce.cart_channel_state_unavailable",
                "Storefront channel availability could not be established",
            )
        })?;
    if !enabled || !channel_enabled {
        return Err(PortError::forbidden(
            "commerce.cart_module_disabled",
            "Commerce is not available in this storefront scope",
        ));
    }
    Ok(())
}

/// Resolve saved Cart selectors, never the current page or a default channel. The pair is
/// read-only: immutable Cart/checkpoint payloads keep their original representation.
/// A timeout bounds waiting; it does not certify cancellation of the underlying DB query.
pub(crate) async fn resolve_cart_channel(
    db: &DatabaseConnection,
    context: &PortContext,
    cart: &rustok_cart::CartResponse,
) -> Result<Option<String>, PortError> {
    context.require_policy(rustok_api::PortCallPolicy::read())?;
    if cart.id.is_nil()
        || cart.tenant_id.is_nil()
        || Uuid::parse_str(context.tenant_id.trim()).ok() != Some(cart.tenant_id)
    {
        return Err(PortError::validation(
            "commerce.checkout_cart_scope_invalid",
            "Cart identity does not match the checkout scope; inspect the cart before retrying",
        ));
    }
    let mut read_context = context.clone();
    read_context.channel = cart.channel_slug.clone();
    let channel_port = rustok_channel::in_process_channel_read_port(db.clone());
    let deadline = std::time::Duration::from_millis(context.deadline_ms.unwrap_or(2000).min(2000));
    let projection = tokio::time::timeout(
        deadline,
        channel_port.read_selected_channel(
            read_context,
            cart.channel_id,
            cart.channel_slug.clone(),
        ),
    )
    .await
    .map_err(|_| {
        PortError::timeout(
            "commerce.checkout_channel_timeout",
            "Cart channel verification exceeded its deadline",
        )
    })??;
    Ok(projection.map(|projection| projection.detail.channel.slug))
}
