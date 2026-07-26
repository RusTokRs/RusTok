mod rustok_fulfillment_shim {
    use ::rustok_fulfillment::{FulfillmentError, FulfillmentResult, ShippingOptionResponse};
    use sea_orm::DatabaseConnection;
    use uuid::Uuid;

    const STOREFRONT_CART_LEGACY_HELPER_BOUNDARY: &str =
        "commerce_graphql_storefront_cart_legacy_helper";

    pub struct FulfillmentService {
        inner: ::rustok_fulfillment::FulfillmentService,
    }

    impl FulfillmentService {
        pub fn new(db: DatabaseConnection) -> Self {
            Self {
                inner: ::rustok_fulfillment::FulfillmentService::new(db),
            }
        }

        pub async fn get_shipping_option(
            &self,
            tenant_id: Uuid,
            shipping_option_id: Uuid,
            requested_locale: Option<&str>,
            tenant_default_locale: Option<&str>,
        ) -> FulfillmentResult<ShippingOptionResponse> {
            self.inner
                .get_shipping_option(
                    tenant_id,
                    shipping_option_id,
                    requested_locale,
                    tenant_default_locale,
                )
                .await
                .map_err(|error| {
                    log_shipping_option_error(
                        &error,
                        tenant_id,
                        shipping_option_id,
                        requested_locale,
                        tenant_default_locale,
                    );
                    error
                })
        }
    }

    fn log_shipping_option_error(
        error: &FulfillmentError,
        tenant_id: Uuid,
        shipping_option_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) {
        let (owner_code, owner_kind, owner_retryable) = match error {
            FulfillmentError::Validation(_) => ("fulfillment.validation", "validation", false),
            FulfillmentError::ShippingOptionNotFound(_) => (
                "fulfillment.shipping_option_not_found",
                "not_found",
                false,
            ),
            FulfillmentError::FulfillmentNotFound(_) => {
                ("fulfillment.fulfillment_not_found", "not_found", false)
            }
            FulfillmentError::InvalidTransition { .. } => {
                ("fulfillment.invalid_transition", "conflict", false)
            }
            FulfillmentError::Database(_) => (
                "fulfillment.database_unavailable",
                "unavailable",
                true,
            ),
        };

        match error {
            FulfillmentError::Database(_) => tracing::error!(
                error = ?error,
                owner = "rustok_fulfillment",
                tenant_id = %tenant_id,
                shipping_option_id = %shipping_option_id,
                requested_locale = ?requested_locale,
                tenant_default_locale = ?tenant_default_locale,
                operation = "get_shipping_option",
                owner_code,
                owner_kind,
                owner_retryable,
                boundary = STOREFRONT_CART_LEGACY_HELPER_BOUNDARY,
                "fulfillment owner shipping option lookup failed"
            ),
            _ => tracing::warn!(
                error = ?error,
                owner = "rustok_fulfillment",
                tenant_id = %tenant_id,
                shipping_option_id = %shipping_option_id,
                requested_locale = ?requested_locale,
                tenant_default_locale = ?tenant_default_locale,
                operation = "get_shipping_option",
                owner_code,
                owner_kind,
                owner_retryable,
                boundary = STOREFRONT_CART_LEGACY_HELPER_BOUNDARY,
                "fulfillment owner shipping option lookup was rejected"
            ),
        }
    }
}

mod rustok_pricing_shim {
    pub use ::rustok_pricing::{
        PriceResolutionContext, PricingReadPort, ResolveProductPriceRequest, ResolvedPrice,
    };
    pub(crate) use super::super::cart::contextual_pricing_read_port as in_process_pricing_read_port;
}

use self::rustok_fulfillment_shim as rustok_fulfillment;
use self::rustok_pricing_shim as rustok_pricing;

include!("helpers.rs");
