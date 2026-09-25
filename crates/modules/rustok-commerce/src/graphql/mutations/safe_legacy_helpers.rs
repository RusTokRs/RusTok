mod rustok_fulfillment_shim {
    use std::sync::Arc;

    use ::rustok_fulfillment::{
        ReadShippingOptionProjectionRequest, ShippingOptionReadPort, ShippingOptionResponse,
    };
    use ::rustok_api::PortErrorKind;
    use sea_orm::DatabaseConnection;
    use uuid::Uuid;

    pub struct FulfillmentService {
        shipping_option_reads: Arc<dyn ShippingOptionReadPort>,
    }

    impl FulfillmentService {
        pub fn new(db: DatabaseConnection) -> Self {
            Self {
                shipping_option_reads:
                    crate::graphql_runtime::shipping_option_read_runtime_for_current_graphql_scope(
                        db,
                    )
                    .shipping_option_read_port(),
            }
        }

        pub async fn get_shipping_option(
            &self,
            tenant_id: Uuid,
            shipping_option_id: Uuid,
            requested_locale: Option<&str>,
            tenant_default_locale: Option<&str>,
        ) -> async_graphql::Result<ShippingOptionResponse> {
            let locale = requested_locale
                .or(tenant_default_locale)
                .unwrap_or_default();
            let context = rustok_api::PortContext::new(
                tenant_id.to_string(),
                rustok_api::PortActor::service("rustok-commerce.graphql-cart-shipping-option"),
                locale,
                format!("commerce-graphql-cart-shipping-option:{shipping_option_id}"),
            )
            .with_deadline(std::time::Duration::from_secs(2));
            let call_context =
                crate::graphql_runtime::fulfillment_read_call_context_for_current_graphql_scope();
            let context = call_context
                .channel()
                .map(|channel| context.clone().with_channel(channel))
                .unwrap_or(context);

            self.shipping_option_reads
                .read_shipping_option_projection(
                    context,
                    ReadShippingOptionProjectionRequest {
                        shipping_option_id,
                        requested_locale: requested_locale.map(str::to_owned),
                        tenant_default_locale: tenant_default_locale.map(str::to_owned),
                    },
                )
                .await
                .map_err(|error| {
                    let (message, error_kind) = match error.kind {
                        PortErrorKind::Validation => (
                            "Shipping option request is invalid",
                            "validation",
                        ),
                        PortErrorKind::NotFound => ("Shipping option was not found", "not_found"),
                        PortErrorKind::Conflict => (
                            "Shipping option state conflicts with this query",
                            "conflict",
                        ),
                        PortErrorKind::Forbidden => (
                            "Shipping option query is not permitted",
                            "forbidden",
                        ),
                        PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                            "Shipping option data is temporarily unavailable",
                            "temporarily_unavailable",
                        ),
                        PortErrorKind::InvariantViolation => (
                            "Shipping option query could not be completed safely",
                            "invariant",
                        ),
                    };
                    tracing::error!(
                        owner = "rustok_fulfillment",
                        owner_operation = "read_shipping_option_projection",
                        correlation_id = %context.correlation_id,
                        error_kind,
                        owner_code = %error.code,
                        owner_message_length = error.message.chars().count(),
                        owner_retryable = error.retryable,
                        boundary = "commerce_graphql_cart_shipping_option",
                        "commerce GraphQL legacy shipping-option owner error was sanitized"
                    );
                    async_graphql::Error::new(message)
                })
        }
    }
}

mod rustok_pricing_shim {
    pub(crate) use super::super::cart::contextual_pricing_read_port as in_process_pricing_read_port;
    pub use ::rustok_pricing::{
        PriceResolutionContext, PricingReadPort, ResolveProductPriceRequest, ResolvedPrice,
    };
}

use self::rustok_fulfillment_shim as rustok_fulfillment;
use self::rustok_pricing_shim as rustok_pricing;

include!("helpers.rs");
