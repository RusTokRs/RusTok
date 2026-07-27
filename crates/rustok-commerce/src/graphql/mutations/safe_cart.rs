mod cart_context_boundary {
    use ::async_graphql::{Error, ErrorExtensions};

    use crate::StoreContextError;

    #[derive(Clone)]
    pub(crate) enum BoundaryError {
        Graphql(Error),
        Public {
            message: &'static str,
            code: &'static str,
            retryable: bool,
        },
    }

    impl From<Error> for BoundaryError {
        fn from(error: Error) -> Self {
            Self::Graphql(error)
        }
    }

    fn public_graphql_error(message: &'static str, code: &'static str, retryable: bool) -> Error {
        Error::new(message).extend_with(|_, extensions| {
            extensions.set("code", code);
            extensions.set("retryable", retryable);
        })
    }

    fn store_context_error_envelope(
        error: &StoreContextError,
    ) -> (&'static str, &'static str, bool, &'static str) {
        match error {
            StoreContextError::TenantNotFound(_) => (
                "Store context was not found",
                "STORE_CONTEXT_NOT_FOUND",
                false,
                "tenant_not_found",
            ),
            StoreContextError::Validation(_) | StoreContextError::CurrencyRegionMismatch { .. } => {
                (
                    "Store context request is invalid",
                    "STORE_CONTEXT_REQUEST_INVALID",
                    false,
                    "validation",
                )
            }
            StoreContextError::RegionBoundary { .. } => (
                "Store context could not be resolved safely",
                "STORE_CONTEXT_RESOLUTION_FAILED",
                false,
                "region_boundary",
            ),
            StoreContextError::Database(_) => (
                "Store context is temporarily unavailable",
                "STORE_CONTEXT_TEMPORARILY_UNAVAILABLE",
                true,
                "database",
            ),
        }
    }

    impl From<StoreContextError> for BoundaryError {
        fn from(error: StoreContextError) -> Self {
            let (message, code, retryable, error_kind) = store_context_error_envelope(&error);
            tracing::error!(
                error = ?error,
                owner = "rustok_commerce.store_context",
                error_kind,
                public_code = code,
                retryable,
                operation = "resolve_store_context",
                boundary = "commerce_graphql_cart",
                "commerce GraphQL cart store context resolution failed"
            );
            Self::Public {
                message,
                code,
                retryable,
            }
        }
    }

    impl From<BoundaryError> for Error {
        fn from(error: BoundaryError) -> Self {
            match error {
                BoundaryError::Graphql(error) => error,
                BoundaryError::Public {
                    message,
                    code,
                    retryable,
                } => public_graphql_error(message, code, retryable),
            }
        }
    }
}

mod cart_storefront_owner_boundary {
    use std::sync::Arc;

    use ::rustok_cart::{
        CartStorefrontAddLineItemRequest, CartStorefrontContextUpdateRequest,
        CartStorefrontCreateRequest, CartStorefrontLineItemPricingRequest,
        CartStorefrontLineItemQuantityRequest, CartStorefrontPort, CartStorefrontReadRequest,
        CartStorefrontRemoveLineItemRequest, CartStorefrontRepriceRequest,
    };
    use rustok_api::{PortContext, PortError};

    const CART_GRAPHQL_OWNER_BOUNDARY: &str = "commerce_graphql_cart";

    fn retain_cart_owner_context<T>(
        context: &PortContext,
        operation: &'static str,
        result: Result<T, PortError>,
    ) -> Result<T, PortError> {
        result.map_err(|error| {
            tracing::error!(
                error = ?error,
                owner = "rustok_cart",
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                locale = %context.locale,
                actor_kind = ?context.actor.kind,
                actor_id = %context.actor.id,
                causation_id = ?context.causation_id,
                idempotency_key = ?context.idempotency_key,
                operation,
                owner_code = %error.code,
                owner_kind = ?error.kind,
                owner_retryable = error.retryable,
                boundary = CART_GRAPHQL_OWNER_BOUNDARY,
                "commerce GraphQL storefront cart owner call failed"
            );
            error
        })
    }

    struct ContextualCartStorefrontPort {
        inner: Arc<dyn CartStorefrontPort>,
    }

    #[async_trait::async_trait]
    impl CartStorefrontPort for ContextualCartStorefrontPort {
        async fn read_storefront_cart(
            &self,
            context: PortContext,
            request: CartStorefrontReadRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self.inner.read_storefront_cart(context, request).await;
            retain_cart_owner_context(&error_context, "read_storefront_cart", result)
        }

        async fn create_storefront_cart(
            &self,
            context: PortContext,
            request: CartStorefrontCreateRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self.inner.create_storefront_cart(context, request).await;
            retain_cart_owner_context(&error_context, "create_storefront_cart", result)
        }

        async fn add_storefront_line_item(
            &self,
            context: PortContext,
            request: CartStorefrontAddLineItemRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self.inner.add_storefront_line_item(context, request).await;
            retain_cart_owner_context(&error_context, "add_storefront_line_item", result)
        }

        async fn update_storefront_context(
            &self,
            context: PortContext,
            request: CartStorefrontContextUpdateRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self.inner.update_storefront_context(context, request).await;
            retain_cart_owner_context(&error_context, "update_storefront_context", result)
        }

        async fn update_storefront_line_item_quantity(
            &self,
            context: PortContext,
            request: CartStorefrontLineItemQuantityRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .update_storefront_line_item_quantity(context, request)
                .await;
            retain_cart_owner_context(
                &error_context,
                "update_storefront_line_item_quantity",
                result,
            )
        }

        async fn update_storefront_line_item_pricing(
            &self,
            context: PortContext,
            request: CartStorefrontLineItemPricingRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .update_storefront_line_item_pricing(context, request)
                .await;
            retain_cart_owner_context(
                &error_context,
                "update_storefront_line_item_pricing",
                result,
            )
        }

        async fn remove_storefront_line_item(
            &self,
            context: PortContext,
            request: CartStorefrontRemoveLineItemRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .remove_storefront_line_item(context, request)
                .await;
            retain_cart_owner_context(&error_context, "remove_storefront_line_item", result)
        }

        async fn reprice_storefront_line_items(
            &self,
            context: PortContext,
            request: CartStorefrontRepriceRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .reprice_storefront_line_items(context, request)
                .await;
            retain_cart_owner_context(&error_context, "reprice_storefront_line_items", result)
        }
    }

    pub(crate) fn in_process_cart_storefront_port(
        db: sea_orm::DatabaseConnection,
    ) -> Arc<dyn CartStorefrontPort> {
        Arc::new(ContextualCartStorefrontPort {
            inner: ::rustok_cart::in_process_cart_storefront_port(db),
        })
    }
}

mod pricing_read_owner_boundary {
    use std::sync::Arc;

    use ::rustok_pricing::{
        ActivePriceListProjectionRequest, ActivePriceListProjectionSnapshot,
        AdminProductPricingProjectionRequest, PreviewVariantDiscountRequest,
        PriceListProjectionRequest, PriceListProjectionSnapshot, PricingReadPort,
        ResolveProductPriceRequest, ResolvedProductPriceSnapshot,
        StorefrontProductPricingProjectionRequest,
    };
    use rustok_api::{PortContext, PortError};

    const PRICING_GRAPHQL_OWNER_BOUNDARY: &str = "commerce_graphql_cart";

    fn retain_pricing_owner_context<T>(
        context: &PortContext,
        operation: &'static str,
        result: Result<T, PortError>,
    ) -> Result<T, PortError> {
        result.map_err(|error| {
            tracing::error!(
                error = ?error,
                owner = "rustok_pricing",
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                channel = ?context.channel,
                locale = %context.locale,
                actor_kind = ?context.actor.kind,
                actor_id = %context.actor.id,
                causation_id = ?context.causation_id,
                operation,
                owner_code = %error.code,
                owner_kind = ?error.kind,
                owner_retryable = error.retryable,
                boundary = PRICING_GRAPHQL_OWNER_BOUNDARY,
                "commerce GraphQL storefront cart pricing owner call failed"
            );
            error
        })
    }

    struct ContextualPricingReadPort {
        inner: Arc<dyn PricingReadPort>,
    }

    #[async_trait::async_trait]
    impl PricingReadPort for ContextualPricingReadPort {
        async fn resolve_product_price(
            &self,
            context: PortContext,
            request: ResolveProductPriceRequest,
        ) -> Result<ResolvedProductPriceSnapshot, PortError> {
            let error_context = context.clone();
            let result = self.inner.resolve_product_price(context, request).await;
            retain_pricing_owner_context(&error_context, "resolve_product_price", result)
        }

        async fn read_price_list_projection(
            &self,
            context: PortContext,
            request: PriceListProjectionRequest,
        ) -> Result<PriceListProjectionSnapshot, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .read_price_list_projection(context, request)
                .await;
            retain_pricing_owner_context(&error_context, "read_price_list_projection", result)
        }

        async fn list_active_price_list_projections(
            &self,
            context: PortContext,
            request: ActivePriceListProjectionRequest,
        ) -> Result<Vec<ActivePriceListProjectionSnapshot>, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .list_active_price_list_projections(context, request)
                .await;
            retain_pricing_owner_context(
                &error_context,
                "list_active_price_list_projections",
                result,
            )
        }

        async fn read_admin_product_pricing_projection(
            &self,
            context: PortContext,
            request: AdminProductPricingProjectionRequest,
        ) -> Result<::rustok_pricing::AdminPricingProductDetail, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .read_admin_product_pricing_projection(context, request)
                .await;
            retain_pricing_owner_context(
                &error_context,
                "read_admin_product_pricing_projection",
                result,
            )
        }

        async fn read_storefront_product_pricing_projection(
            &self,
            context: PortContext,
            request: StorefrontProductPricingProjectionRequest,
        ) -> Result<Option<::rustok_pricing::StorefrontPricingProductDetail>, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .read_storefront_product_pricing_projection(context, request)
                .await;
            retain_pricing_owner_context(
                &error_context,
                "read_storefront_product_pricing_projection",
                result,
            )
        }

        async fn preview_variant_discount(
            &self,
            context: PortContext,
            request: PreviewVariantDiscountRequest,
        ) -> Result<::rustok_pricing::PriceAdjustmentPreview, PortError> {
            let error_context = context.clone();
            let result = self.inner.preview_variant_discount(context, request).await;
            retain_pricing_owner_context(&error_context, "preview_variant_discount", result)
        }
    }

    pub(crate) fn in_process_pricing_read_port(
        db: sea_orm::DatabaseConnection,
        event_bus: rustok_outbox::TransactionalEventBus,
    ) -> Arc<dyn PricingReadPort> {
        Arc::new(ContextualPricingReadPort {
            inner: ::rustok_pricing::in_process_pricing_read_port(db, event_bus),
        })
    }
}

pub(crate) use pricing_read_owner_boundary::in_process_pricing_read_port as contextual_pricing_read_port;

mod rustok_cart_shim {
    pub(crate) use super::cart_storefront_owner_boundary::in_process_cart_storefront_port;
    pub use ::rustok_cart::{
        CartStorefrontAddLineItemRequest, CartStorefrontContextUpdateRequest,
        CartStorefrontCreateRequest, CartStorefrontLineItemPricingRequest,
        CartStorefrontLineItemQuantityRequest, CartStorefrontReadRequest,
        CartStorefrontRemoveLineItemRequest,
    };
}

mod rustok_pricing_shim {
    pub(crate) use super::pricing_read_owner_boundary::in_process_pricing_read_port;
    pub use ::rustok_pricing::{ResolveProductPriceRequest, ResolvedPrice};
}

mod async_graphql_shim {
    pub use ::async_graphql::{Context, Error, MaybeUndefined, Object};

    pub type Result<T> = std::result::Result<T, super::cart_context_boundary::BoundaryError>;
}

use self::async_graphql_shim as async_graphql;
use self::rustok_cart_shim as rustok_cart;
use self::rustok_pricing_shim as rustok_pricing;

include!("cart.rs");
