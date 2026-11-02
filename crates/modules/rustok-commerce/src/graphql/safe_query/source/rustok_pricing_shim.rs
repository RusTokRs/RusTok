use std::sync::Arc;

use ::rustok_api::{PortContext, PortError, PortErrorKind};
use ::rustok_outbox::TransactionalEventBus;
use ::rustok_pricing::PricingReadPort as OwnerPricingReadPort;
pub(crate) use ::rustok_pricing::{
    ActivePriceListProjectionRequest, AdminProductPricingProjectionRequest, PriceResolutionContext,
    ResolveProductPricesRequest, StorefrontProductPricingListRequest,
    StorefrontProductPricingProjectionRequest,
};
use ::sea_orm::DatabaseConnection;

use super::super::query_error_boundary::{BoundaryError, QueryGraphqlMessage};

const GRAPHQL_QUERY_PRICING_BOUNDARY: &str = "commerce_graphql_query_pricing";

struct PricingQueryDiagnosticError;

impl std::fmt::Debug for PricingQueryDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PricingQueryKind {
    Validation,
    NotFound,
    Conflict,
    Forbidden,
    Unavailable,
    Timeout,
    InvariantViolation,
}

impl From<&PortErrorKind> for PricingQueryKind {
    fn from(kind: &PortErrorKind) -> Self {
        match kind {
            PortErrorKind::Validation => Self::Validation,
            PortErrorKind::NotFound => Self::NotFound,
            PortErrorKind::Conflict => Self::Conflict,
            PortErrorKind::Forbidden => Self::Forbidden,
            PortErrorKind::Unavailable => Self::Unavailable,
            PortErrorKind::Timeout => Self::Timeout,
            PortErrorKind::InvariantViolation => Self::InvariantViolation,
        }
    }
}

impl PartialEq<PortErrorKind> for PricingQueryKind {
    fn eq(&self, other: &PortErrorKind) -> bool {
        matches!(
            (self, other),
            (Self::Validation, PortErrorKind::Validation)
                | (Self::NotFound, PortErrorKind::NotFound)
                | (Self::Conflict, PortErrorKind::Conflict)
                | (Self::Forbidden, PortErrorKind::Forbidden)
                | (Self::Unavailable, PortErrorKind::Unavailable)
                | (Self::Timeout, PortErrorKind::Timeout)
                | (Self::InvariantViolation, PortErrorKind::InvariantViolation)
        )
    }
}

pub(crate) struct PricingGraphqlMessage {
    error: PortError,
}

impl QueryGraphqlMessage for PricingGraphqlMessage {
    fn into_query_boundary(self) -> BoundaryError {
        let (message, code, error_kind, technical) = match &self.error.kind {
            PortErrorKind::Validation => (
                "Pricing query is invalid",
                "PRICING_REQUEST_INVALID",
                "validation",
                false,
            ),
            PortErrorKind::NotFound => (
                "Pricing data was not found",
                "PRICING_RESOURCE_NOT_FOUND",
                "not_found",
                false,
            ),
            PortErrorKind::Conflict => (
                "Pricing state conflicts with this query",
                "PRICING_STATE_CONFLICT",
                "conflict",
                false,
            ),
            PortErrorKind::Forbidden => (
                "Pricing query is not permitted",
                "PRICING_ACCESS_DENIED",
                "forbidden",
                false,
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Pricing data is temporarily unavailable",
                "PRICING_TEMPORARILY_UNAVAILABLE",
                "unavailable",
                true,
            ),
            PortErrorKind::InvariantViolation => (
                "Pricing query could not be completed safely",
                "PRICING_OPERATION_FAILED",
                "invariant",
                true,
            ),
        };
        // Dependency presentation must not turn an unsupported selected capability into a retry loop.
        let retryable = self.error.retryable;
        let (message, code) = if self.error.kind == PortErrorKind::Unavailable
            && matches!(
                self.error.code.as_str(),
                "pricing.product_identity_capability_unavailable"
                    | "pricing.product_write_capability_unavailable"
            ) {
            (
                "The selected Product owner does not support this pricing operation",
                "PRICING_CAPABILITY_UNAVAILABLE",
            )
        } else {
            (message, code)
        };
        let owner_message_present = !self.error.message.is_empty();
        let owner_message_length = self.error.message.chars().count();
        let diagnostic_error = PricingQueryDiagnosticError;
        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_pricing",
                error_kind,
                owner_code = %self.error.code,
                owner_message_present,
                owner_message_length,
                owner_retryable = self.error.retryable,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_PRICING_BOUNDARY,
                "commerce GraphQL pricing query failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = "rustok_pricing",
                error_kind,
                owner_code = %self.error.code,
                owner_message_present,
                owner_message_length,
                owner_retryable = self.error.retryable,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_PRICING_BOUNDARY,
                "commerce GraphQL pricing query was rejected"
            );
        }
        BoundaryError::Public {
            message,
            code,
            retryable,
        }
    }
}

pub(crate) struct PricingQueryPortError {
    pub(crate) kind: PricingQueryKind,
    pub(crate) message: PricingGraphqlMessage,
}

impl PricingQueryPortError {
    pub(crate) fn is_product_not_found(&self) -> bool {
        self.kind == PricingQueryKind::NotFound
            && self.message.error.code == "pricing.product_not_found"
    }
}

impl From<PortError> for PricingQueryPortError {
    fn from(error: PortError) -> Self {
        let kind = PricingQueryKind::from(&error.kind);
        Self {
            kind,
            message: PricingGraphqlMessage { error },
        }
    }
}

#[::async_trait::async_trait]
pub(crate) trait PricingReadPort: Send + Sync {
    async fn resolve_product_prices(
        &self,
        context: PortContext,
        request: ResolveProductPricesRequest,
    ) -> Result<::rustok_pricing::ResolvedProductPricesSnapshot, PricingQueryPortError>;

    async fn list_storefront_product_pricing(
        &self,
        context: PortContext,
        request: StorefrontProductPricingListRequest,
    ) -> Result<::rustok_pricing::StorefrontPricingProductList, PricingQueryPortError>;

    async fn list_active_price_list_projections(
        &self,
        context: PortContext,
        request: ActivePriceListProjectionRequest,
    ) -> Result<Vec<::rustok_pricing::ActivePriceListProjectionSnapshot>, PricingQueryPortError>;

    async fn read_admin_product_pricing_projection(
        &self,
        context: PortContext,
        request: AdminProductPricingProjectionRequest,
    ) -> Result<::rustok_pricing::AdminPricingProductDetail, PricingQueryPortError>;

    async fn read_storefront_product_pricing_projection(
        &self,
        context: PortContext,
        request: StorefrontProductPricingProjectionRequest,
    ) -> Result<Option<::rustok_pricing::StorefrontPricingProductDetail>, PricingQueryPortError>;
}

/// Typed error-presentation adapter for the Commerce pricing query source.
///
/// The projection not-found branches compare only a closed local kind copied from
/// `PortErrorKind`; every other failure retains the complete typed owner error for
/// the transport-owned GraphQL mapper.
pub(crate) struct PricingQueryReadPort {
    inner: Arc<dyn OwnerPricingReadPort>,
}

pub(crate) fn in_process_pricing_read_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    product_runtime: ::rustok_product::ProductCatalogReadRuntime,
) -> Arc<dyn PricingReadPort> {
    Arc::new(PricingQueryReadPort {
        inner: ::rustok_pricing::in_process_pricing_read_port(db, event_bus, product_runtime),
    })
}

#[::async_trait::async_trait]
impl PricingReadPort for PricingQueryReadPort {
    async fn resolve_product_prices(
        &self,
        context: PortContext,
        request: ResolveProductPricesRequest,
    ) -> Result<::rustok_pricing::ResolvedProductPricesSnapshot, PricingQueryPortError> {
        self.inner
            .resolve_product_prices(context, request)
            .await
            .map_err(Into::into)
    }

    async fn list_storefront_product_pricing(
        &self,
        context: PortContext,
        request: StorefrontProductPricingListRequest,
    ) -> Result<::rustok_pricing::StorefrontPricingProductList, PricingQueryPortError> {
        self.inner
            .list_storefront_product_pricing(context, request)
            .await
            .map_err(Into::into)
    }

    async fn list_active_price_list_projections(
        &self,
        context: PortContext,
        request: ActivePriceListProjectionRequest,
    ) -> Result<Vec<::rustok_pricing::ActivePriceListProjectionSnapshot>, PricingQueryPortError>
    {
        self.inner
            .list_active_price_list_projections(context, request)
            .await
            .map_err(Into::into)
    }

    async fn read_admin_product_pricing_projection(
        &self,
        context: PortContext,
        request: AdminProductPricingProjectionRequest,
    ) -> Result<::rustok_pricing::AdminPricingProductDetail, PricingQueryPortError> {
        self.inner
            .read_admin_product_pricing_projection(context, request)
            .await
            .map_err(Into::into)
    }

    async fn read_storefront_product_pricing_projection(
        &self,
        context: PortContext,
        request: StorefrontProductPricingProjectionRequest,
    ) -> Result<Option<::rustok_pricing::StorefrontPricingProductDetail>, PricingQueryPortError>
    {
        self.inner
            .read_storefront_product_pricing_projection(context, request)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_missing_product_can_be_presented_as_no_product() {
        for (code, expected) in [
            ("pricing.product_not_found", true),
            ("pricing.price_list_not_found", false),
            ("adapter.untrusted", false),
        ] {
            let error =
                PricingQueryPortError::from(PortError::not_found(code, "private-owner-detail"));
            assert_eq!(error.is_product_not_found(), expected);
        }
        let error = PricingQueryPortError::from(PortError::unavailable(
            "pricing.product_not_found",
            "private-owner-detail",
        ));
        assert!(!error.is_product_not_found());
    }

    #[test]
    fn unsupported_selected_owner_does_not_become_a_transient_retry_hint() {
        let error = PricingGraphqlMessage {
            error: PortError::new(
                PortErrorKind::Unavailable,
                "pricing.product_identity_capability_unavailable",
                "private-owner-detail",
                false,
            ),
        };
        match error.into_query_boundary() {
            BoundaryError::Public {
                code,
                retryable,
                message,
            } => {
                assert_eq!(code, "PRICING_CAPABILITY_UNAVAILABLE");
                assert!(!retryable);
                assert!(!message.contains("private-owner-detail"));
            }
            _ => panic!("capability refusal must use the static public envelope"),
        }
    }
}
