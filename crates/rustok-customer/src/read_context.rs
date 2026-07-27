use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::dto::CustomerResponse;
use crate::ports::{
    CustomerListProjectionRequest, CustomerListProjectionResponse, CustomerProfileEnrichment,
    CustomerProfileEnrichmentRequest, CustomerProjectionRequest, CustomerReadPort,
    CustomerUserProjectionRequest,
};
use crate::services::CustomerService;

const CUSTOMER_OWNER: &str = "rustok_customer";
const CUSTOMER_READ_BOUNDARY: &str = "customer_read_port";
const READ_CUSTOMER_PROJECTION_OPERATION: &str = "read_customer_projection";
const READ_CUSTOMER_PROJECTION_BY_USER_OPERATION: &str = "read_customer_projection_by_user";
const LIST_CUSTOMER_PROJECTIONS_OPERATION: &str = "list_customer_projections";
const LIST_PROFILE_ENRICHMENT_OPERATION: &str = "list_profile_enrichment";

#[derive(Debug, Clone, Default)]
struct CustomerReadDiagnosticFacts {
    customer_id: Option<Uuid>,
    user_id: Option<Uuid>,
    page: Option<u64>,
    per_page: Option<u64>,
    search_length: Option<usize>,
    requested_user_count: Option<usize>,
    unique_user_count: Option<usize>,
}

/// Canonical in-process customer read provider with retained local outcome context.
pub struct InProcessCustomerReadPort {
    inner: CustomerService,
}

impl InProcessCustomerReadPort {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            inner: CustomerService::new(db),
        }
    }

    /// Wraps a host-composed customer service without changing owner behavior.
    pub fn from_service(inner: CustomerService) -> Self {
        Self { inner }
    }
}

/// Builds the canonical owner-managed in-process customer read provider.
pub fn in_process_customer_read_port(db: DatabaseConnection) -> Arc<dyn CustomerReadPort> {
    Arc::new(InProcessCustomerReadPort::new(db))
}

#[async_trait]
impl CustomerReadPort for InProcessCustomerReadPort {
    async fn read_customer_projection(
        &self,
        context: PortContext,
        request: CustomerProjectionRequest,
    ) -> Result<CustomerResponse, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = CustomerReadDiagnosticFacts {
            customer_id: Some(request.customer_id),
            ..CustomerReadDiagnosticFacts::default()
        };
        let result = CustomerReadPort::read_customer_projection(&self.inner, context, request).await;
        result.map_err(|error| {
            map_customer_read_local_port_error(
                &diagnostic_context,
                READ_CUSTOMER_PROJECTION_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn read_customer_projection_by_user(
        &self,
        context: PortContext,
        request: CustomerUserProjectionRequest,
    ) -> Result<CustomerResponse, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = CustomerReadDiagnosticFacts {
            user_id: Some(request.user_id),
            ..CustomerReadDiagnosticFacts::default()
        };
        let result =
            CustomerReadPort::read_customer_projection_by_user(&self.inner, context, request).await;
        result.map_err(|error| {
            map_customer_read_local_port_error(
                &diagnostic_context,
                READ_CUSTOMER_PROJECTION_BY_USER_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn list_customer_projections(
        &self,
        context: PortContext,
        request: CustomerListProjectionRequest,
    ) -> Result<CustomerListProjectionResponse, PortError> {
        let diagnostic_context = context.clone();
        let diagnostic_facts = CustomerReadDiagnosticFacts {
            page: Some(request.page),
            per_page: Some(request.per_page),
            search_length: request
                .search
                .as_ref()
                .map(|value| value.chars().count()),
            ..CustomerReadDiagnosticFacts::default()
        };
        let result =
            CustomerReadPort::list_customer_projections(&self.inner, context, request).await;
        result.map_err(|error| {
            map_customer_read_local_port_error(
                &diagnostic_context,
                LIST_CUSTOMER_PROJECTIONS_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }

    async fn list_profile_enrichment(
        &self,
        context: PortContext,
        request: CustomerProfileEnrichmentRequest,
    ) -> Result<Vec<CustomerProfileEnrichment>, PortError> {
        let diagnostic_context = context.clone();
        let requested_user_count = request.user_ids.len();
        let unique_user_count = request.user_ids.iter().copied().collect::<HashSet<_>>().len();
        let diagnostic_facts = CustomerReadDiagnosticFacts {
            requested_user_count: Some(requested_user_count),
            unique_user_count: Some(unique_user_count),
            ..CustomerReadDiagnosticFacts::default()
        };
        let result = CustomerReadPort::list_profile_enrichment(&self.inner, context, request).await;
        result.map_err(|error| {
            map_customer_read_local_port_error(
                &diagnostic_context,
                LIST_PROFILE_ENRICHMENT_OPERATION,
                &diagnostic_facts,
                error,
            )
        })
    }
}

fn map_customer_read_local_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    facts: &CustomerReadDiagnosticFacts,
    error: PortError,
) -> PortError {
    let local_operation = match (
        owner_operation,
        error.code.as_str(),
        error.message.as_str(),
    ) {
        (_, "customer.context_invalid", "customer request context is invalid") => {
            "validate_tenant_context"
        }
        (
            LIST_CUSTOMER_PROJECTIONS_OPERATION,
            "customer.page_invalid",
            "customer projection page is invalid",
        ) => "validate_page",
        (
            LIST_CUSTOMER_PROJECTIONS_OPERATION,
            "customer.per_page_invalid",
            "customer projection page size is invalid",
        ) => "validate_page_size",
        (
            _,
            "customer.database_unavailable",
            "customer storage is temporarily unavailable",
        ) => "owner_storage",
        (
            READ_CUSTOMER_PROJECTION_OPERATION,
            "customer.customer_not_found",
            "customer was not found",
        ) => "load_customer",
        (
            READ_CUSTOMER_PROJECTION_BY_USER_OPERATION,
            "customer.customer_by_user_not_found",
            "customer was not found for the requested user",
        ) => "load_customer_by_user",
        (_, "customer.validation", "customer request is invalid") => "validate_owner_request",
        (
            _,
            "customer.profile_unavailable",
            "customer profile projection is temporarily unavailable",
        ) => "load_profile_projection",
        _ => return error,
    };

    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );

    if technical_failure {
        tracing::error!(
            error = ?error,
            owner = CUSTOMER_OWNER,
            operation = owner_operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            customer_id = ?facts.customer_id,
            user_id = ?facts.user_id,
            page = ?facts.page,
            per_page = ?facts.per_page,
            search_length = ?facts.search_length,
            requested_user_count = ?facts.requested_user_count,
            unique_user_count = ?facts.unique_user_count,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = CUSTOMER_READ_BOUNDARY,
            "customer read local technical outcome retained delegated context"
        );
    } else {
        tracing::warn!(
            error = ?error,
            owner = CUSTOMER_OWNER,
            operation = owner_operation,
            local_operation,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            actor = ?context.actor,
            channel = ?context.channel,
            locale = %context.locale,
            causation_id = ?context.causation_id,
            traceparent = ?context.traceparent,
            idempotency_key = ?context.idempotency_key,
            deadline_ms = ?context.deadline_ms,
            customer_id = ?facts.customer_id,
            user_id = ?facts.user_id,
            page = ?facts.page,
            per_page = ?facts.per_page,
            search_length = ?facts.search_length,
            requested_user_count = ?facts.requested_user_count,
            unique_user_count = ?facts.unique_user_count,
            internal_code = %error.code,
            internal_message = %error.message,
            error_kind = ?error.kind,
            retryable = error.retryable,
            boundary = CUSTOMER_READ_BOUNDARY,
            "customer read local outcome retained delegated context"
        );
    }

    error
}
