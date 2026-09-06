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

struct CustomerReadContextFacts {
    tenant_id_length: usize,
    correlation_id_length: usize,
    actor_kind: &'static str,
    actor_id_length: usize,
    claim_count: usize,
    role_count: usize,
    channel_present: bool,
    channel_length: Option<usize>,
    locale_length: usize,
    causation_id_present: bool,
    causation_id_length: Option<usize>,
    traceparent_present: bool,
    traceparent_length: Option<usize>,
    idempotency_key_present: bool,
    idempotency_key_length: Option<usize>,
    deadline_ms: Option<u64>,
}

#[derive(Debug, Clone, Default)]
struct CustomerReadDiagnosticFacts {
    customer_id_present: bool,
    customer_id_non_nil: bool,
    user_id_present: bool,
    user_id_non_nil: bool,
    page_present: bool,
    page_nonzero: bool,
    per_page_present: bool,
    per_page_nonzero: bool,
    search_present: bool,
    search_length: Option<usize>,
    requested_user_ids_present: bool,
    requested_user_ids_empty: bool,
    duplicate_user_ids_present: bool,
}

impl CustomerReadDiagnosticFacts {
    fn customer(customer_id: Uuid) -> Self {
        Self {
            customer_id_present: true,
            customer_id_non_nil: !customer_id.is_nil(),
            ..Self::default()
        }
    }

    fn user(user_id: Uuid) -> Self {
        Self {
            user_id_present: true,
            user_id_non_nil: !user_id.is_nil(),
            ..Self::default()
        }
    }

    fn list(request: &CustomerListProjectionRequest) -> Self {
        Self {
            page_present: true,
            page_nonzero: request.page != 0,
            per_page_present: true,
            per_page_nonzero: request.per_page != 0,
            search_present: request.search.is_some(),
            search_length: request.search.as_ref().map(|value| value.chars().count()),
            ..Self::default()
        }
    }

    fn enrichment(request: &CustomerProfileEnrichmentRequest) -> Self {
        let requested_user_count = request.user_ids.len();
        let unique_user_count = request
            .user_ids
            .iter()
            .copied()
            .collect::<HashSet<_>>()
            .len();
        Self {
            requested_user_ids_present: true,
            requested_user_ids_empty: request.user_ids.is_empty(),
            duplicate_user_ids_present: unique_user_count < requested_user_count,
            ..Self::default()
        }
    }
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
        let diagnostic_facts = CustomerReadDiagnosticFacts::customer(request.customer_id);
        let result =
            CustomerReadPort::read_customer_projection(&self.inner, context, request).await;
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
        let diagnostic_facts = CustomerReadDiagnosticFacts::user(request.user_id);
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
        let diagnostic_facts = CustomerReadDiagnosticFacts::list(&request);
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
        let diagnostic_facts = CustomerReadDiagnosticFacts::enrichment(&request);
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

fn customer_read_context_facts(context: &PortContext) -> CustomerReadContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    CustomerReadContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
        correlation_id_length: context.correlation_id.chars().count(),
        actor_kind,
        actor_id_length: context.actor.id.chars().count(),
        claim_count: context.claims.len(),
        role_count: context.roles.len(),
        channel_present: context.channel.is_some(),
        channel_length: context.channel.as_ref().map(|value| value.chars().count()),
        locale_length: context.locale.chars().count(),
        causation_id_present: context.causation_id.is_some(),
        causation_id_length: context
            .causation_id
            .as_ref()
            .map(|value| value.chars().count()),
        traceparent_present: context.traceparent.is_some(),
        traceparent_length: context
            .traceparent
            .as_ref()
            .map(|value| value.chars().count()),
        idempotency_key_present: context.idempotency_key.is_some(),
        idempotency_key_length: context
            .idempotency_key
            .as_ref()
            .map(|value| value.chars().count()),
        deadline_ms: context.deadline_ms,
    }
}

fn customer_read_port_error_kind(kind: &PortErrorKind) -> &'static str {
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

fn map_customer_read_local_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    facts: &CustomerReadDiagnosticFacts,
    error: PortError,
) -> PortError {
    let local_operation = match (owner_operation, error.code.as_str(), error.message.as_str()) {
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
        (_, "customer.database_unavailable", "customer storage is temporarily unavailable") => {
            "owner_storage"
        }
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

    log_customer_read_local_outcome(context, owner_operation, local_operation, facts, &error);
    error
}

fn log_customer_read_local_outcome(
    context: &PortContext,
    owner_operation: &'static str,
    local_operation: &'static str,
    request_facts: &CustomerReadDiagnosticFacts,
    error: &PortError,
) {
    let context_facts = customer_read_context_facts(context);
    let technical_failure = matches!(
        &error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout | PortErrorKind::InvariantViolation
    );

    if technical_failure {
        tracing::error!(
            owner = CUSTOMER_OWNER,
            operation = owner_operation,
            local_operation,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            customer_id_present = request_facts.customer_id_present,
            customer_id_non_nil = request_facts.customer_id_non_nil,
            user_id_present = request_facts.user_id_present,
            user_id_non_nil = request_facts.user_id_non_nil,
            page_present = request_facts.page_present,
            page_nonzero = request_facts.page_nonzero,
            per_page_present = request_facts.per_page_present,
            per_page_nonzero = request_facts.per_page_nonzero,
            search_present = request_facts.search_present,
            search_length = ?request_facts.search_length,
            requested_user_ids_present = request_facts.requested_user_ids_present,
            requested_user_ids_empty = request_facts.requested_user_ids_empty,
            duplicate_user_ids_present = request_facts.duplicate_user_ids_present,
            code = %error.code,
            error_message_present = !error.message.is_empty(),
            error_message_length = error.message.chars().count(),
            error_kind = customer_read_port_error_kind(&error.kind),
            retryable = error.retryable,
            boundary = CUSTOMER_READ_BOUNDARY,
            "customer read local technical outcome retained bounded delegated context"
        );
    } else {
        tracing::warn!(
            owner = CUSTOMER_OWNER,
            operation = owner_operation,
            local_operation,
            correlation_id_length = context_facts.correlation_id_length,
            tenant_id_length = context_facts.tenant_id_length,
            actor_kind = context_facts.actor_kind,
            actor_id_length = context_facts.actor_id_length,
            claim_count = context_facts.claim_count,
            role_count = context_facts.role_count,
            channel_present = context_facts.channel_present,
            channel_length = ?context_facts.channel_length,
            locale_length = context_facts.locale_length,
            causation_id_present = context_facts.causation_id_present,
            causation_id_length = ?context_facts.causation_id_length,
            traceparent_present = context_facts.traceparent_present,
            traceparent_length = ?context_facts.traceparent_length,
            idempotency_key_present = context_facts.idempotency_key_present,
            idempotency_key_length = ?context_facts.idempotency_key_length,
            deadline_ms = ?context_facts.deadline_ms,
            customer_id_present = request_facts.customer_id_present,
            customer_id_non_nil = request_facts.customer_id_non_nil,
            user_id_present = request_facts.user_id_present,
            user_id_non_nil = request_facts.user_id_non_nil,
            page_present = request_facts.page_present,
            page_nonzero = request_facts.page_nonzero,
            per_page_present = request_facts.per_page_present,
            per_page_nonzero = request_facts.per_page_nonzero,
            search_present = request_facts.search_present,
            search_length = ?request_facts.search_length,
            requested_user_ids_present = request_facts.requested_user_ids_present,
            requested_user_ids_empty = request_facts.requested_user_ids_empty,
            duplicate_user_ids_present = request_facts.duplicate_user_ids_present,
            code = %error.code,
            error_message_present = !error.message.is_empty(),
            error_message_length = error.message.chars().count(),
            error_kind = customer_read_port_error_kind(&error.kind),
            retryable = error.retryable,
            boundary = CUSTOMER_READ_BOUNDARY,
            "customer read local outcome retained bounded delegated context"
        );
    }
}
