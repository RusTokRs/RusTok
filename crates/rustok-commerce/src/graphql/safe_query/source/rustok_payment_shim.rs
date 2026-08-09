use std::sync::Arc;

use ::rustok_api::{PortContext, PortError as OwnerPortError, PortErrorKind};
use ::rustok_payment::{
    LatestPaymentCollectionByOrderRequest, ListPaymentCollectionProjectionsRequest,
    ListRefundProjectionsRequest, PaymentAdminReadPort, PaymentCartReadPort,
    PaymentCollectionResponse, PaymentOrderReadPort, ReadPaymentCollectionProjectionRequest,
    ReadRefundProjectionRequest, RefundResponse, ReusablePaymentCollectionByCartRequest,
    dto::{ListPaymentCollectionsInput, ListRefundsInput},
};
use ::sea_orm::DatabaseConnection;
use ::uuid::Uuid;

use super::super::query_error_boundary::BoundaryError;

const GRAPHQL_QUERY_PAYMENT_BOUNDARY: &str = "commerce_graphql_query_payment";

struct PaymentQueryDiagnosticError;

impl std::fmt::Debug for PaymentQueryDiagnosticError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("redacted")
    }
}

fn uuid_shape(value: &Uuid) -> &'static str {
    if value.is_nil() {
        "uuid_nil"
    } else {
        "uuid_non_nil"
    }
}

fn port_error_kind(kind: &PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::InvariantViolation => "invariant_violation",
    }
}

fn is_configuration_error(error: &OwnerPortError) -> bool {
    matches!(
        error.code.as_str(),
        "payment.admin_read_configuration"
            | "payment.order_read_configuration"
            | "payment.cart_read_configuration"
    )
}

#[derive(Clone, Copy, Debug)]
enum PaymentQueryResource {
    Cart(Uuid),
    Order(Uuid),
    Collection(Uuid),
    Refund(Uuid),
    Tenant(Uuid),
}

impl PaymentQueryResource {
    fn kind(self) -> &'static str {
        match self {
            Self::Cart(_) => "cart",
            Self::Order(_) => "order",
            Self::Collection(_) => "payment_collection",
            Self::Refund(_) => "refund",
            Self::Tenant(_) => "tenant",
        }
    }

    fn id(self) -> Uuid {
        match self {
            Self::Cart(id)
            | Self::Order(id)
            | Self::Collection(id)
            | Self::Refund(id)
            | Self::Tenant(id) => id,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PaymentQueryError {
    error: OwnerPortError,
    tenant_id: Uuid,
    operation: &'static str,
    resource: PaymentQueryResource,
}

impl PaymentQueryError {
    fn new(
        error: OwnerPortError,
        tenant_id: Uuid,
        operation: &'static str,
        resource: PaymentQueryResource,
    ) -> Self {
        Self {
            error,
            tenant_id,
            operation,
            resource,
        }
    }

    fn into_boundary(self) -> BoundaryError {
        let configuration = is_configuration_error(&self.error);
        let (message, code, retryable, error_kind, technical) = if configuration {
            (
                "Payment provider configuration is invalid",
                "PAYMENT_CONFIGURATION_ERROR",
                false,
                "configuration",
                true,
            )
        } else {
            match &self.error.kind {
                PortErrorKind::Validation => (
                    "Payment query is invalid",
                    "PAYMENT_REQUEST_INVALID",
                    false,
                    "validation",
                    false,
                ),
                PortErrorKind::NotFound => (
                    "Payment resource was not found",
                    "PAYMENT_RESOURCE_NOT_FOUND",
                    false,
                    "not_found",
                    false,
                ),
                PortErrorKind::Conflict => (
                    "Payment state conflicts with this query",
                    "PAYMENT_STATE_CONFLICT",
                    false,
                    "state_conflict",
                    false,
                ),
                PortErrorKind::Unavailable | PortErrorKind::Timeout => (
                    "Payment data is temporarily unavailable",
                    "PAYMENT_TEMPORARILY_UNAVAILABLE",
                    true,
                    "temporarily_unavailable",
                    true,
                ),
                PortErrorKind::InvariantViolation => (
                    "Payment state requires reconciliation",
                    "PAYMENT_RECONCILIATION_REQUIRED",
                    false,
                    "reconciliation_required",
                    true,
                ),
                PortErrorKind::Forbidden => (
                    "Payment query is invalid",
                    "PAYMENT_REQUEST_INVALID",
                    false,
                    "forbidden",
                    false,
                ),
            }
        };
        let owner_detail_shape = port_error_kind(&self.error.kind);
        let owner_detail_length = self.error.code.chars().count();
        let resource_kind = self.resource.kind();
        let resource_id = self.resource.id();
        let resource_id_shape = uuid_shape(&resource_id);
        let correlation_id = format!(
            "commerce-graphql-payment:{}:{}",
            self.operation, resource_id
        );
        let diagnostic_error = PaymentQueryDiagnosticError;
        if technical {
            tracing::error!(
                error = ?diagnostic_error,
                owner = "rustok_payment",
                owner_operation = self.operation,
                correlation_id,
                tenant_id = %self.tenant_id,
                resource_kind,
                resource_id_shape,
                error_kind,
                owner_detail_shape,
                owner_detail_length,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_PAYMENT_BOUNDARY,
                "commerce GraphQL payment query failed"
            );
        } else {
            tracing::warn!(
                error = ?diagnostic_error,
                owner = "rustok_payment",
                owner_operation = self.operation,
                correlation_id,
                tenant_id = %self.tenant_id,
                resource_kind,
                resource_id_shape,
                error_kind,
                owner_detail_shape,
                owner_detail_length,
                public_code = code,
                retryable,
                boundary = GRAPHQL_QUERY_PAYMENT_BOUNDARY,
                "commerce GraphQL payment query was rejected"
            );
        }
        BoundaryError::Public {
            message,
            code,
            retryable,
        }
    }
}

pub(crate) mod error {
    use super::{
        BoundaryError, OwnerPortError, PaymentQueryError, PaymentQueryResource, PortErrorKind, Uuid,
    };

    #[derive(Debug)]
    pub(crate) enum PaymentError {
        PaymentCollectionNotFound(PaymentQueryError),
        RefundNotFound(PaymentQueryError),
        Other(PaymentQueryError),
    }

    impl PaymentError {
        pub(super) fn from_owner_port(
            error: OwnerPortError,
            tenant_id: Uuid,
            operation: &'static str,
            resource: PaymentQueryResource,
        ) -> Self {
            let not_found = error.kind == PortErrorKind::NotFound;
            let collection_not_found =
                not_found && matches!(resource, PaymentQueryResource::Collection(_));
            let refund_not_found =
                not_found && matches!(resource, PaymentQueryResource::Refund(_));
            let error = PaymentQueryError::new(error, tenant_id, operation, resource);
            if collection_not_found {
                Self::PaymentCollectionNotFound(error)
            } else if refund_not_found {
                Self::RefundNotFound(error)
            } else {
                Self::Other(error)
            }
        }

        #[allow(clippy::inherent_to_string, clippy::wrong_self_convention)]
        pub(crate) fn to_string(self) -> BoundaryError {
            match self {
                Self::PaymentCollectionNotFound(error)
                | Self::RefundNotFound(error)
                | Self::Other(error) => error.into_boundary(),
            }
        }
    }
}

use error::PaymentError;

pub(crate) struct PaymentService {
    admin_reads: Arc<dyn PaymentAdminReadPort>,
    order_reads: Arc<dyn PaymentOrderReadPort>,
    cart_reads: Arc<dyn PaymentCartReadPort>,
}

impl PaymentService {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        let runtime = crate::graphql_runtime::payment_read_runtime_for_current_graphql_scope(db);
        Self {
            admin_reads: runtime.admin_read_port(),
            order_reads: runtime.order_read_port(),
            cart_reads: runtime.cart_read_port(),
        }
    }

    fn context(
        &self,
        tenant_id: Uuid,
        operation: &'static str,
        resource: PaymentQueryResource,
    ) -> PortContext {
        let (actor, channel, locale) =
            crate::graphql_runtime::payment_read_call_context_for_current_graphql_scope();
        let context = PortContext::new(
            tenant_id.to_string(),
            actor,
            locale.as_deref().unwrap_or("und"),
            format!("commerce-graphql-payment:{operation}:{}", resource.id()),
        )
        .with_deadline(std::time::Duration::from_secs(2));
        match channel.as_deref() {
            Some(channel) => context.with_channel(channel),
            None => context,
        }
    }

    pub(crate) async fn find_reusable_collection_by_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> Result<Option<PaymentCollectionResponse>, PaymentError> {
        const OPERATION: &str = "find_reusable_collection_by_cart";
        let resource = PaymentQueryResource::Cart(cart_id);
        self.cart_reads
            .find_reusable_collection_by_cart(
                self.context(tenant_id, OPERATION, resource),
                ReusablePaymentCollectionByCartRequest { cart_id },
            )
            .await
            .map_err(|error| PaymentError::from_owner_port(error, tenant_id, OPERATION, resource))
    }

    pub(crate) async fn find_latest_collection_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> Result<Option<PaymentCollectionResponse>, PaymentError> {
        const OPERATION: &str = "find_latest_collection_by_order";
        let resource = PaymentQueryResource::Order(order_id);
        self.order_reads
            .find_latest_collection_by_order(
                self.context(tenant_id, OPERATION, resource),
                LatestPaymentCollectionByOrderRequest { order_id },
            )
            .await
            .map_err(|error| PaymentError::from_owner_port(error, tenant_id, OPERATION, resource))
    }

    pub(crate) async fn get_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
    ) -> Result<PaymentCollectionResponse, PaymentError> {
        const OPERATION: &str = "get_collection";
        let resource = PaymentQueryResource::Collection(collection_id);
        self.admin_reads
            .read_payment_collection_projection(
                self.context(tenant_id, OPERATION, resource),
                ReadPaymentCollectionProjectionRequest { collection_id },
            )
            .await
            .map_err(|error| PaymentError::from_owner_port(error, tenant_id, OPERATION, resource))
    }

    pub(crate) async fn list_collections(
        &self,
        tenant_id: Uuid,
        input: ListPaymentCollectionsInput,
    ) -> Result<(Vec<PaymentCollectionResponse>, u64), PaymentError> {
        const OPERATION: &str = "list_collections";
        let resource = PaymentQueryResource::Tenant(tenant_id);
        let page = self
            .admin_reads
            .list_payment_collection_projections(
                self.context(tenant_id, OPERATION, resource),
                ListPaymentCollectionProjectionsRequest {
                    page: input.page,
                    per_page: input.per_page,
                    status: input.status,
                    order_id: input.order_id,
                    cart_id: input.cart_id,
                    customer_id: input.customer_id,
                },
            )
            .await
            .map_err(|error| PaymentError::from_owner_port(error, tenant_id, OPERATION, resource))?;
        Ok((page.items, page.total))
    }

    pub(crate) async fn get_refund(
        &self,
        tenant_id: Uuid,
        refund_id: Uuid,
    ) -> Result<RefundResponse, PaymentError> {
        const OPERATION: &str = "get_refund";
        let resource = PaymentQueryResource::Refund(refund_id);
        self.admin_reads
            .read_refund_projection(
                self.context(tenant_id, OPERATION, resource),
                ReadRefundProjectionRequest { refund_id },
            )
            .await
            .map_err(|error| PaymentError::from_owner_port(error, tenant_id, OPERATION, resource))
    }

    pub(crate) async fn list_refunds(
        &self,
        tenant_id: Uuid,
        input: ListRefundsInput,
    ) -> Result<(Vec<RefundResponse>, u64), PaymentError> {
        const OPERATION: &str = "list_refunds";
        let resource = PaymentQueryResource::Tenant(tenant_id);
        let page = self
            .admin_reads
            .list_refund_projections(
                self.context(tenant_id, OPERATION, resource),
                ListRefundProjectionsRequest {
                    page: input.page,
                    per_page: input.per_page,
                    payment_collection_id: input.payment_collection_id,
                    order_id: input.order_id,
                    status: input.status,
                },
            )
            .await
            .map_err(|error| PaymentError::from_owner_port(error, tenant_id, OPERATION, resource))?;
        Ok((page.items, page.total))
    }
}
