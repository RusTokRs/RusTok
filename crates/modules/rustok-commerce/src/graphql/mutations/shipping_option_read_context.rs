use std::sync::Arc;

use rustok_api::{PortActor, PortContext};
use rustok_fulfillment::ShippingOptionReadPort;
use uuid::Uuid;

pub(crate) fn storefront_shipping_option_read_port(
    db: sea_orm::DatabaseConnection,
) -> Arc<dyn ShippingOptionReadPort> {
    rustok_fulfillment::in_process_shipping_option_read_port(db)
}

pub(crate) fn storefront_shipping_option_read_context(
    tenant_id: Uuid,
    cart_id: Uuid,
    locale: &str,
    public_channel_slug: Option<&str>,
    operation: &str,
) -> PortContext {
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("rustok-commerce.storefront-shipping"),
        locale,
        format!("storefront-shipping:{operation}:{cart_id}"),
    )
    .with_deadline(std::time::Duration::from_secs(2));

    public_channel_slug
        .map(|channel| context.clone().with_channel(channel))
        .unwrap_or(context)
}
