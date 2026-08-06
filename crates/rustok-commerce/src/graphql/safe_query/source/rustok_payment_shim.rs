use ::rustok_payment::{
    dto::{
        ListPaymentCollectionsInput, ListRefundsInput, PaymentCollectionResponse, RefundResponse,
    },
    error::PaymentError as OwnerPaymentError,
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

fn text_shape(value: &str) -> &'static str {
    if value.is_empty() { "empty" } else { "present" }
}

fn uuid_shape(value: &Uuid) -> &'static str {
    if value.is_nil() {
        "uuid_nil"
    } else {
        "uuid_non_nil"
    }
}

fn owner_detail(error: &OwnerPaymentError) -> (&'static str, usize) {
    match error {
        OwnerPaymentError::Validation(value) => (text_shape(value), value.chars().count()),
        OwnerPaymentError::PaymentCollectionNotFound(value)
        | OwnerPaymentError::PaymentNotFound(value)
        | OwnerPaymentError::RefundNotFound(value) => (uuid_shape(value), 0),
        OwnerPaymentError::InvalidTransition { from, to } => (
            "two_status_values",
            from.chars().count().saturating_add(to.chars().count()),
        ),
        OwnerPaymentError::ProviderUnavailable {
            provider_id,
            operation,
        }
        | OwnerPaymentError::ProviderRejected {
            provider_id,
            operation,
        }
        | OwnerPaymentError::ProviderInvalidResponse {
            provider_id,
            operation,
        }
        | OwnerPaymentError::ProviderOutcomeUnknown {
            provider_id,
            operation,
        } => (
            "provider_operation_values",
            provider_id
                .chars()
                .count()
                .saturating_add(operation.chars().count()),
        ),
        OwnerPaymentError::ProviderConfiguration { provider_id } => {
            (text_shape(provider_id), provider_id.chars().count())
        }
        OwnerPaymentError::Database(_) => ("database_redacted", 0),
    }
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
    error: OwnerPaymentError,
    tenant_id: Uuid,
    operation: &'static str,
    resource: PaymentQueryResource,
}

impl PaymentQueryError {
    fn new(
        error: OwnerPaymentError,
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
        let (message, code, retryable, error_kind, technical) = match &self.error {
            OwnerPaymentError::Validation(_) => (
                "Payment query is invalid",
                "PAYMENT_REQUEST_INVALID",
                false,
                "validation",
                false,
            ),
            OwnerPaymentError::PaymentCollectionNotFound(_)
            | OwnerPaymentError::PaymentNotFound(_)
            | OwnerPaymentError::RefundNotFound(_) => (
                "Payment resource was not found",
                "PAYMENT_RESOURCE_NOT_FOUND",
                false,
                "not_found",
                false,
            ),
            OwnerPaymentError::InvalidTransition { .. }
            | OwnerPaymentError::ProviderRejected { .. } => (
                "Payment state conflicts with this query",
                "PAYMENT_STATE_CONFLICT",
                false,
                "state_conflict",
                false,
            ),
            OwnerPaymentError::ProviderUnavailable { .. }
            | OwnerPaymentError::Database(_) => (
                "Payment data is temporarily unavailable",
                "PAYMENT_TEMPORARILY_UNAVAILABLE",
                true,
                "temporarily_unavailable",
                true,
            ),
            OwnerPaymentError::ProviderInvalidResponse { .. }
            | OwnerPaymentError::ProviderOutcomeUnknown { .. } => (
                "Payment state requires reconciliation",
                "PAYMENT_RECONCILIATION_REQUIRED",
                false,
                "reconciliation_required",
                true,
            ),
            OwnerPaymentError::ProviderConfiguration { .. } => (
                "Payment provider configuration is invalid",
                "PAYMENT_CONFIGURATION_ERROR",
                false,
                "configuration",
                true,
            ),
        };
        let (owner_detail_shape, owner_detail_length) = owner_detail(&self.error);
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
        BoundaryError, OwnerPaymentError, PaymentQueryError, PaymentQueryResource, Uuid,
    };

    #[derive(Debug)]
    pub(crate) enum PaymentError {
        PaymentCollectionNotFound(PaymentQueryError),
        RefundNotFound(PaymentQueryError),
        Other(PaymentQueryError),
    }

    impl PaymentError {
        fn from_owner(
            error: OwnerPaymentError,
            tenant_id: Uuid,
            operation: &'static str,
            resource: PaymentQueryResource,
        ) -> Self {
            let error = PaymentQueryError::new(error, tenant_id, operation, resource);
            if matches!(
                &error.error,
                OwnerPaymentError::PaymentCollectionNotFound(_)
            ) {
                Self::PaymentCollectionNotFound(error)
            } else if matches!(&error.error, OwnerPaymentError::RefundNotFound(_)) {
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
    inner: ::rustok_payment::PaymentService,
}

impl PaymentService {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: ::rustok_payment::PaymentService::new(db),
        }
    }

    pub(crate) async fn find_reusable_collection_by_cart(
        &self,
        tenant_id: Uuid,
        cart_id: Uuid,
    ) -> Result<Option<PaymentCollectionResponse>, PaymentError> {
        self.inner
            .find_reusable_collection_by_cart(tenant_id, cart_id)
            .await
            .map_err(|error| {
                PaymentError::from_owner(
                    error,
                    tenant_id,
                    "find_reusable_collection_by_cart",
                    PaymentQueryResource::Cart(cart_id),
                )
            })
    }

    pub(crate) async fn find_latest_collection_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> Result<Option<PaymentCollectionResponse>, PaymentError> {
        self.inner
            .find_latest_collection_by_order(tenant_id, order_id)
            .await
            .map_err(|error| {
                PaymentError::from_owner(
                    error,
                    tenant_id,
                    "find_latest_collection_by_order",
                    PaymentQueryResource::Order(order_id),
                )
            })
    }

    pub(crate) async fn get_collection(
        &self,
        tenant_id: Uuid,
        collection_id: Uuid,
    ) -> Result<PaymentCollectionResponse, PaymentError> {
        self.inner
            .get_collection(tenant_id, collection_id)
            .await
            .map_err(|error| {
                PaymentError::from_owner(
                    error,
                    tenant_id,
                    "get_collection",
                    PaymentQueryResource::Collection(collection_id),
                )
            })
    }

    pub(crate) async fn list_collections(
        &self,
        tenant_id: Uuid,
        input: ListPaymentCollectionsInput,
    ) -> Result<(Vec<PaymentCollectionResponse>, u64), PaymentError> {
        self.inner
            .list_collections(tenant_id, input)
            .await
            .map_err(|error| {
                PaymentError::from_owner(
                    error,
                    tenant_id,
                    "list_collections",
                    PaymentQueryResource::Tenant(tenant_id),
                )
            })
    }

    pub(crate) async fn get_refund(
        &self,
        tenant_id: Uuid,
        refund_id: Uuid,
    ) -> Result<RefundResponse, PaymentError> {
        self.inner
            .get_refund(tenant_id, refund_id)
            .await
            .map_err(|error| {
                PaymentError::from_owner(
                    error,
                    tenant_id,
                    "get_refund",
                    PaymentQueryResource::Refund(refund_id),
                )
            })
    }

    pub(crate) async fn list_refunds(
        &self,
        tenant_id: Uuid,
        input: ListRefundsInput,
    ) -> Result<(Vec<RefundResponse>, u64), PaymentError> {
        self.inner
            .list_refunds(tenant_id, input)
            .await
            .map_err(|error| {
                PaymentError::from_owner(
                    error,
                    tenant_id,
                    "list_refunds",
                    PaymentQueryResource::Tenant(tenant_id),
                )
            })
    }
}
