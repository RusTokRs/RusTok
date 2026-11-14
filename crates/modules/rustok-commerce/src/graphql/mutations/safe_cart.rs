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

    struct StoreContextDiagnosticError;

    impl std::fmt::Debug for StoreContextDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
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
            StoreContextError::TenantBoundary { .. } => (
                "Store context could not be resolved safely",
                "STORE_CONTEXT_RESOLUTION_FAILED",
                false,
                "tenant_boundary",
            ),
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
            let error = StoreContextDiagnosticError;
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
        CartContextUpdateRequest, CartStorefrontAddLineItemRequest, CartStorefrontCreateRequest,
        CartStorefrontLineItemPricingRequest, CartStorefrontPort, CartStorefrontReadRequest,
        CartStorefrontRemoveLineItemRequest, CartStorefrontRepriceRequest,
    };
    use rustok_api::{PortActorKind, PortContext, PortError};

    const CART_GRAPHQL_OWNER_BOUNDARY: &str = "commerce_graphql_cart";

    #[derive(Clone, Copy)]
    struct CartOwnerDiagnosticContext {
        tenant_id_shape: &'static str,
        actor_kind: &'static str,
        actor_id_shape: &'static str,
        claim_count: usize,
        role_count: usize,
        channel_shape: &'static str,
        locale_shape: &'static str,
        correlation_id_shape: &'static str,
        causation_id_shape: &'static str,
        traceparent_shape: &'static str,
        idempotency_key_shape: &'static str,
        deadline_ms: Option<u64>,
    }

    impl From<&PortContext> for CartOwnerDiagnosticContext {
        fn from(context: &PortContext) -> Self {
            Self {
                tenant_id_shape: identity_text_shape(context.tenant_id.as_str()),
                actor_kind: actor_kind_name(&context.actor.kind),
                actor_id_shape: identity_text_shape(context.actor.id.as_str()),
                claim_count: context.claims.len(),
                role_count: context.roles.len(),
                channel_shape: optional_text_shape(context.channel.as_deref()),
                locale_shape: text_shape(context.locale.as_str()),
                correlation_id_shape: text_shape(context.correlation_id.as_str()),
                causation_id_shape: optional_text_shape(context.causation_id.as_deref()),
                traceparent_shape: optional_text_shape(context.traceparent.as_deref()),
                idempotency_key_shape: optional_text_shape(context.idempotency_key.as_deref()),
                deadline_ms: context.deadline_ms,
            }
        }
    }

    struct CartOwnerDiagnosticError;

    impl std::fmt::Debug for CartOwnerDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    fn actor_kind_name(kind: &PortActorKind) -> &'static str {
        match kind {
            PortActorKind::User => "user",
            PortActorKind::Service => "service",
            PortActorKind::System => "system",
        }
    }

    fn identity_text_shape(value: &str) -> &'static str {
        if value.is_empty() {
            return "empty";
        }
        match uuid::Uuid::parse_str(value) {
            Ok(value) if value.is_nil() => "uuid_nil",
            Ok(_) => "uuid_non_nil",
            Err(_) => "opaque",
        }
    }

    fn text_shape(value: &str) -> &'static str {
        if value.is_empty() { "empty" } else { "present" }
    }

    fn optional_text_shape(value: Option<&str>) -> &'static str {
        match value {
            None => "absent",
            Some("") => "empty",
            Some(_) => "present",
        }
    }

    fn retain_cart_owner_context<T>(
        context: &PortContext,
        operation: &'static str,
        result: Result<T, PortError>,
    ) -> Result<T, PortError> {
        result.inspect_err(|error| {
            let diagnostic_context = CartOwnerDiagnosticContext::from(context);
            let owner_code = error.code.clone();
            let owner_kind = error.kind.clone();
            let owner_retryable = error.retryable;
            let owner_message_shape = text_shape(error.message.as_str());
            let owner_message_len = error.message.len();
            let diagnostic_error = CartOwnerDiagnosticError;

            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_cart",
                tenant_id_shape = diagnostic_context.tenant_id_shape,
                actor_kind = diagnostic_context.actor_kind,
                actor_id_shape = diagnostic_context.actor_id_shape,
                claim_count = diagnostic_context.claim_count,
                role_count = diagnostic_context.role_count,
                channel_shape = diagnostic_context.channel_shape,
                locale_shape = diagnostic_context.locale_shape,
                correlation_id_shape = diagnostic_context.correlation_id_shape,
                causation_id_shape = diagnostic_context.causation_id_shape,
                traceparent_shape = diagnostic_context.traceparent_shape,
                idempotency_key_shape = diagnostic_context.idempotency_key_shape,
                deadline_ms = ?diagnostic_context.deadline_ms,
                operation,
                owner_code = %owner_code,
                owner_message_shape,
                owner_message_len,
                owner_kind = ?owner_kind,
                owner_retryable,
                boundary = CART_GRAPHQL_OWNER_BOUNDARY,
                "commerce GraphQL storefront cart owner call failed"
            );
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
            request: CartContextUpdateRequest,
        ) -> Result<::rustok_cart::CartResponse, PortError> {
            let error_context = context.clone();
            let result = self.inner.update_storefront_context(context, request).await;
            retain_cart_owner_context(&error_context, "update_storefront_context", result)
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
    use rustok_api::{PortActorKind, PortContext, PortError};

    const PRICING_GRAPHQL_OWNER_BOUNDARY: &str = "commerce_graphql_cart";

    #[derive(Clone, Copy)]
    struct PricingOwnerDiagnosticContext {
        tenant_id_shape: &'static str,
        actor_kind: &'static str,
        actor_id_shape: &'static str,
        claim_count: usize,
        role_count: usize,
        channel_shape: &'static str,
        locale_shape: &'static str,
        correlation_id_shape: &'static str,
        causation_id_shape: &'static str,
        traceparent_shape: &'static str,
        idempotency_key_shape: &'static str,
        deadline_ms: Option<u64>,
    }

    impl From<&PortContext> for PricingOwnerDiagnosticContext {
        fn from(context: &PortContext) -> Self {
            Self {
                tenant_id_shape: identity_text_shape(context.tenant_id.as_str()),
                actor_kind: actor_kind_name(&context.actor.kind),
                actor_id_shape: identity_text_shape(context.actor.id.as_str()),
                claim_count: context.claims.len(),
                role_count: context.roles.len(),
                channel_shape: optional_text_shape(context.channel.as_deref()),
                locale_shape: text_shape(context.locale.as_str()),
                correlation_id_shape: text_shape(context.correlation_id.as_str()),
                causation_id_shape: optional_text_shape(context.causation_id.as_deref()),
                traceparent_shape: optional_text_shape(context.traceparent.as_deref()),
                idempotency_key_shape: optional_text_shape(context.idempotency_key.as_deref()),
                deadline_ms: context.deadline_ms,
            }
        }
    }

    struct PricingOwnerDiagnosticError;

    impl std::fmt::Debug for PricingOwnerDiagnosticError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("redacted")
        }
    }

    fn actor_kind_name(kind: &PortActorKind) -> &'static str {
        match kind {
            PortActorKind::User => "user",
            PortActorKind::Service => "service",
            PortActorKind::System => "system",
        }
    }

    fn identity_text_shape(value: &str) -> &'static str {
        if value.is_empty() {
            return "empty";
        }
        match uuid::Uuid::parse_str(value) {
            Ok(value) if value.is_nil() => "uuid_nil",
            Ok(_) => "uuid_non_nil",
            Err(_) => "opaque",
        }
    }

    fn text_shape(value: &str) -> &'static str {
        if value.is_empty() { "empty" } else { "present" }
    }

    fn optional_text_shape(value: Option<&str>) -> &'static str {
        match value {
            None => "absent",
            Some("") => "empty",
            Some(_) => "present",
        }
    }

    fn retain_pricing_owner_context<T>(
        context: &PortContext,
        operation: &'static str,
        result: Result<T, PortError>,
    ) -> Result<T, PortError> {
        result.inspect_err(|error| {
            let diagnostic_context = PricingOwnerDiagnosticContext::from(context);
            let owner_code = error.code.clone();
            let owner_kind = error.kind.clone();
            let owner_retryable = error.retryable;
            let owner_message_shape = text_shape(error.message.as_str());
            let owner_message_len = error.message.len();
            let diagnostic_error = PricingOwnerDiagnosticError;

            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_pricing",
                tenant_id_shape = diagnostic_context.tenant_id_shape,
                actor_kind = diagnostic_context.actor_kind,
                actor_id_shape = diagnostic_context.actor_id_shape,
                claim_count = diagnostic_context.claim_count,
                role_count = diagnostic_context.role_count,
                channel_shape = diagnostic_context.channel_shape,
                locale_shape = diagnostic_context.locale_shape,
                correlation_id_shape = diagnostic_context.correlation_id_shape,
                causation_id_shape = diagnostic_context.causation_id_shape,
                traceparent_shape = diagnostic_context.traceparent_shape,
                idempotency_key_shape = diagnostic_context.idempotency_key_shape,
                deadline_ms = ?diagnostic_context.deadline_ms,
                operation,
                owner_code = %owner_code,
                owner_message_shape,
                owner_message_len,
                owner_kind = ?owner_kind,
                owner_retryable,
                boundary = PRICING_GRAPHQL_OWNER_BOUNDARY,
                "commerce GraphQL storefront cart pricing owner call failed"
            );
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

        async fn resolve_product_prices(
            &self,
            context: PortContext,
            request: ::rustok_pricing::ResolveProductPricesRequest,
        ) -> Result<::rustok_pricing::ResolvedProductPricesSnapshot, PortError> {
            let error_context = context.clone();
            let result = self.inner.resolve_product_prices(context, request).await;
            retain_pricing_owner_context(&error_context, "resolve_product_prices", result)
        }

        async fn list_storefront_product_pricing(
            &self,
            context: PortContext,
            request: ::rustok_pricing::StorefrontProductPricingListRequest,
        ) -> Result<::rustok_pricing::StorefrontPricingProductList, PortError> {
            let error_context = context.clone();
            let result = self
                .inner
                .list_storefront_product_pricing(context, request)
                .await;
            retain_pricing_owner_context(&error_context, "list_storefront_product_pricing", result)
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
        product_runtime: ::rustok_product::ProductCatalogReadRuntime,
    ) -> Arc<dyn PricingReadPort> {
        Arc::new(ContextualPricingReadPort {
            inner: ::rustok_pricing::in_process_pricing_read_port(db, event_bus, product_runtime),
        })
    }
}

pub(crate) use pricing_read_owner_boundary::in_process_pricing_read_port as contextual_pricing_read_port;

mod rustok_cart_shim {
    pub(crate) use super::cart_storefront_owner_boundary::in_process_cart_storefront_port;
    pub use ::rustok_cart::{
        CartPricingContextGuard, CartStorefrontCreateRequest, CartStorefrontReadRequest,
        CartStorefrontRemoveLineItemRequest,
    };
}

mod rustok_pricing_shim {
    pub(crate) use super::pricing_read_owner_boundary::in_process_pricing_read_port;
}

mod async_graphql_shim {
    pub use ::async_graphql::{Context, Error, MaybeUndefined, Object};

    pub type Result<T> = std::result::Result<T, super::cart_context_boundary::BoundaryError>;
}

use self::async_graphql_shim as async_graphql;
use self::rustok_cart_shim as rustok_cart;
use self::rustok_pricing_shim as rustok_pricing;

use async_graphql::{Context, Object, Result};
use rustok_api::{AuthContext, RequestContext, TenantContext};
use uuid::Uuid;

use crate::StoreContextService;
use rustok_cart::{
    CartStorefrontCreateRequest, CartStorefrontReadRequest, CartStorefrontRemoveLineItemRequest,
    in_process_cart_storefront_port,
};
use rustok_pricing::in_process_pricing_read_port;

use super::super::{current_tenant_scope, types::*};
use super::helpers::*;
use super::typed_shipping_option_helper::validate_selected_shipping_option;

#[derive(Default)]
pub struct CommerceCartMutation;

#[Object]
impl CommerceCartMutation {
    async fn create_storefront_cart(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        input: CreateStorefrontCartInput,
    ) -> Result<GqlStoreCart> {
        super::super::require_storefront_cart_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Storefront cart creation")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let context = StoreContextService::new(
            db.clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
        )
        .resolve_context(
            tenant_id,
            crate::dto::ResolveStoreContextInput {
                region_id: input.region_id,
                country_code: input.country_code.clone(),
                locale: input
                    .locale
                    .clone()
                    .or_else(|| Some(request_context.locale.clone())),
                currency_code: input.currency_code.clone(),
            },
        )
        .await?;
        let currency_code = context
            .currency_code
            .clone()
            .or(input.currency_code.clone())
            .ok_or_else(|| {
                async_graphql::Error::new(
                    "currency_code is required unless it can be resolved from region/country",
                )
            })?;

        let cart = in_process_cart_storefront_port(db.clone())
            .create_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    tenant_id,
                    "create",
                    true,
                ),
                CartStorefrontCreateRequest {
                    input: crate::dto::CreateCartInput {
                        customer_id,
                        email: input.email,
                        region_id: context.region.as_ref().map(|region| region.id),
                        country_code: input.country_code,
                        locale_code: Some(context.locale.clone()),
                        selected_shipping_option_id: None,
                        currency_code,
                        metadata: parse_optional_metadata(input.metadata.as_deref())?,
                    },
                    channel_id: request_context.channel_id,
                    channel_slug: request_context.channel_slug.clone(),
                },
            )
            .await
            .map_err(cart_port_error)?;
        let cart = enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            cart,
        )
        .await?;

        Ok(GqlStoreCart {
            cart: cart.into(),
            context: context.into(),
        })
    }

    async fn add_storefront_cart_line_item(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        input: AddStorefrontCartLineItemInput,
    ) -> Result<GqlCart> {
        super::super::require_storefront_cart_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Add storefront cart line item")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let product_runtime =
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            );
        let runtime = crate::storefront_cart_runtime::StorefrontCartRuntime::new(
            product_runtime.read_port(),
            in_process_pricing_read_port(
                db.clone(),
                event_bus.clone(),
                crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                    db.clone(),
                    event_bus.clone(),
                ),
            ),
            in_process_cart_storefront_port(db.clone()),
            rustok_inventory::in_process_inventory_reservation_port(db.clone(), event_bus.clone()),
        );
        let updated = runtime
            .add_line_item(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "add-line-item",
                    true,
                ),
                customer_id,
                tenant.default_locale.as_str(),
                crate::storefront_cart_runtime::StorefrontCartAddCommand {
                    cart_id,
                    variant_id: input.variant_id,
                    quantity: input.quantity,
                    expected_context: input.expected_context.map(Into::into),
                    expected_locale: input.expected_locale,
                    metadata: parse_optional_metadata(input.metadata.as_deref())?,
                },
            )
            .await
            .map_err(cart_port_error)?;
        Ok(enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            updated,
        )
        .await?
        .into())
    }

    async fn update_storefront_cart_context(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        input: UpdateStorefrontCartContextInput,
    ) -> Result<GqlStoreCart> {
        super::super::require_storefront_cart_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Update storefront cart context")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest { cart_id },
            )
            .await
            .map_err(cart_port_error)?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;

        let region_was_explicit = !input.region_id.is_undefined();
        let email = maybe_undefined_or_existing(input.email, cart.email.clone());
        let requested_region_id = maybe_undefined_or_existing(input.region_id, cart.region_id);
        let requested_country_code = match input.country_code {
            async_graphql::MaybeUndefined::Value(value) => Some(value),
            async_graphql::MaybeUndefined::Null => None,
            async_graphql::MaybeUndefined::Undefined if region_was_explicit => None,
            async_graphql::MaybeUndefined::Undefined => cart.country_code.clone(),
        };
        let requested_locale = maybe_undefined_or_existing(input.locale, cart.locale_code.clone())
            .or_else(|| Some(request_context.locale.clone()));
        let requested_shipping_option_id = maybe_undefined_or_existing(
            input.selected_shipping_option_id,
            cart.selected_shipping_option_id,
        );
        let requested_shipping_selections = match input.shipping_selections {
            async_graphql::MaybeUndefined::Value(items) => Some(
                items
                    .into_iter()
                    .map(|item| crate::dto::CartShippingSelectionInput {
                        shipping_profile_slug: item.shipping_profile_slug,
                        seller_id: item.seller_id,
                        seller_scope: None,
                        selected_shipping_option_id: item.selected_shipping_option_id,
                    })
                    .collect::<Vec<_>>(),
            ),
            async_graphql::MaybeUndefined::Null => Some(Vec::new()),
            async_graphql::MaybeUndefined::Undefined => None,
        };

        let context = StoreContextService::new(
            db.clone(),
            std::sync::Arc::new(rustok_region::RegionService::new(db.clone())),
        )
        .resolve_context(
            tenant_id,
            crate::dto::ResolveStoreContextInput {
                region_id: requested_region_id,
                country_code: requested_country_code.clone(),
                locale: requested_locale,
                currency_code: Some(cart.currency_code.clone()),
            },
        )
        .await?;
        let context_input = crate::dto::UpdateCartContextInput {
            email,
            region_id: context.region.as_ref().map(|region| region.id),
            country_code: requested_country_code,
            locale_code: Some(context.locale.clone()),
            selected_shipping_option_id: requested_shipping_option_id,
            shipping_selections: requested_shipping_selections,
        };
        let target = cart.project_context(&context_input).map_err(|_| {
            cart_port_error(rustok_api::PortError::validation(
                "commerce.cart_context_invalid",
                "Cart context is invalid",
            ))
        })?;
        let write_context = storefront_cart_port_context(
            tenant_id,
            request_context,
            ctx.data_opt::<AuthContext>(),
            cart_id,
            "update-context",
            true,
        );
        let public_channel_slug =
            crate::storefront_channel::resolve_cart_channel(db, &write_context, &target)
                .await
                .map_err(cart_port_error)?;
        validate_selected_shipping_option(
            db,
            tenant_id,
            &target,
            context_input.selected_shipping_option_id,
            context_input.shipping_selections.as_deref(),
            &target.currency_code,
            public_channel_slug.as_deref(),
            target.locale_code.as_deref(),
            Some(tenant.default_locale.as_str()),
        )
        .await?;
        let pricing = in_process_pricing_read_port(
            db.clone(),
            event_bus.clone(),
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            ),
        );
        let command = crate::prepare_storefront_cart_context(
            pricing.as_ref(),
            write_context.clone(),
            &cart,
            context_input,
        )
        .await
        .map_err(super::typed_reprice_helper::cart_context_preparation_error)?;
        let updated = cart_storefront_port
            .update_storefront_context(write_context, command)
            .await
            .map_err(cart_port_error)?;
        let updated = enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            updated,
        )
        .await?;

        Ok(GqlStoreCart {
            cart: updated.into(),
            context: context.into(),
        })
    }

    async fn update_storefront_cart_line_item(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        line_id: Uuid,
        input: UpdateStorefrontCartLineItemInput,
    ) -> Result<GqlCart> {
        super::super::require_storefront_cart_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let request_context = ctx.data::<RequestContext>()?;
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Update storefront cart line item")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest { cart_id },
            )
            .await
            .map_err(cart_port_error)?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let event_bus = ctx.data::<rustok_outbox::TransactionalEventBus>()?;
        let product_runtime =
            crate::graphql_runtime::product_catalog_read_runtime_for_current_graphql_scope(
                db.clone(),
                event_bus.clone(),
            );
        let workflow = crate::storefront_cart_runtime::StorefrontCartRuntime::new(
            product_runtime.read_port(),
            in_process_pricing_read_port(db.clone(), event_bus.clone(), product_runtime),
            cart_storefront_port,
            rustok_inventory::in_process_inventory_reservation_port(db.clone(), event_bus.clone()),
        );
        let updated = workflow
            .update_line_item_quantity(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "update-line-item",
                    true,
                ),
                customer_id,
                tenant.default_locale.as_str(),
                cart,
                line_id,
                input.quantity,
            )
            .await
            .map_err(cart_port_error)?;
        Ok(enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            updated,
        )
        .await?
        .into())
    }

    async fn remove_storefront_cart_line_item(
        &self,
        ctx: &Context<'_>,
        tenant_id: Option<Uuid>,
        cart_id: Uuid,
        line_id: Uuid,
    ) -> Result<GqlCart> {
        super::super::require_storefront_cart_enabled(ctx).await?;

        let db = ctx.data::<sea_orm::DatabaseConnection>()?;
        let tenant = ctx.data::<TenantContext>()?;
        let tenant_id = current_tenant_scope(ctx, tenant_id, "Remove storefront cart line item")?;
        let customer_id =
            resolve_optional_storefront_customer_id(db, tenant_id, ctx.data_opt::<AuthContext>())
                .await?;
        let request_context = ctx.data::<RequestContext>()?;
        let cart_storefront_port = in_process_cart_storefront_port(db.clone());
        let cart = cart_storefront_port
            .read_storefront_cart(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "read",
                    false,
                ),
                CartStorefrontReadRequest { cart_id },
            )
            .await
            .map_err(cart_port_error)?;
        ensure_storefront_cart_access(&cart, customer_id)?;
        let updated = cart_storefront_port
            .remove_storefront_line_item(
                storefront_cart_port_context(
                    tenant_id,
                    request_context,
                    ctx.data_opt::<AuthContext>(),
                    cart_id,
                    "remove-line-item",
                    true,
                ),
                CartStorefrontRemoveLineItemRequest {
                    cart_id,
                    line_item_id: line_id,
                    pricing_context: rustok_cart::CartPricingContextGuard::capture_removal(
                        &cart, line_id,
                    )
                    .map_err(cart_port_error)?,
                },
            )
            .await
            .map_err(cart_port_error)?;
        Ok(enrich_storefront_cart(
            db,
            tenant_id,
            request_context,
            tenant.default_locale.as_str(),
            updated,
        )
        .await?
        .into())
    }
}
