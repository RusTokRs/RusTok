use std::sync::Arc;

use ::rustok_api::{PortContext, PortError, PortErrorKind};
use ::rustok_outbox::TransactionalEventBus;
use ::rustok_pricing::PricingReadPort as OwnerPricingReadPort;
pub(crate) use ::rustok_pricing::{
    ActivePriceListProjectionRequest, AdminProductPricingProjectionRequest, PriceResolutionContext,
    ResolveProductPriceRequest, ResolvedPrice, StorefrontProductPricingProjectionRequest,
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
        let (message, code, retryable, error_kind, technical) = match &self.error.kind {
            PortErrorKind::Validation => (
                "Pricing query is invalid",
                "PRICING_REQUEST_INVALID",
                false,
                "validation",
                false,
            ),
            PortErrorKind::NotFound => (
                "Pricing data was not found",
                "PRICING_RESOURCE_NOT_FOUND",
                false,
                "not_found",
                false,
            ),
            PortErrorKind::Conflict => (
                "Pricing state conflicts with this query",
                "PRICING_STATE_CONFLICT",
                false,
                "conflict",
                false,
            ),
            PortErrorKind::Forbidden => (
                "Pricing query is not permitted",
                "PRICING_ACCESS_DENIED",
                false,
                "forbidden",
                false,
            ),
            PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                "Pricing data is temporarily unavailable",
                "PRICING_TEMPORARILY_UNAVAILABLE",
                true,
                "unavailable",
                true,
            ),
            PortErrorKind::InvariantViolation => (
                "Pricing query could not be completed safely",
                "PRICING_OPERATION_FAILED",
                false,
                "invariant",
                true,
            ),
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
    async fn resolve_product_price(
        &self,
        context: PortContext,
        request: ResolveProductPriceRequest,
    ) -> Result<::rustok_pricing::ResolvedProductPriceSnapshot, PricingQueryPortError>;

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

/// Compatibility facade for the unchanged Commerce pricing query source.
///
/// The two legacy not-found branches compare only a closed local kind copied from
/// `PortErrorKind`; every other failure retains the complete typed owner error for
/// the transport-owned GraphQL mapper.
pub(crate) struct PricingQueryReadPort {
    inner: Arc<dyn OwnerPricingReadPort>,
}

pub(crate) fn in_process_pricing_read_port(
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
) -> Arc<dyn PricingReadPort> {
    Arc::new(PricingQueryReadPort {
        inner: ::rustok_pricing::in_process_pricing_read_port(db, event_bus),
    })
}

#[::async_trait::async_trait]
impl PricingReadPort for PricingQueryReadPort {
    async fn resolve_product_price(
        &self,
        context: PortContext,
        request: ResolveProductPriceRequest,
    ) -> Result<::rustok_pricing::ResolvedProductPriceSnapshot, PricingQueryPortError> {
        self.inner
            .resolve_product_price(context, request)
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
