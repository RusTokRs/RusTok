use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::{CustomerResponse, ListCustomersInput};
use crate::error::CustomerError;

const MAX_CUSTOMERS_PER_PAGE: u64 = 100;
const CUSTOMER_READ_PORT_BOUNDARY: &str = "customer_read_port";

/// Transport-neutral owner boundary for customer read projections used by checkout/order flows.
#[async_trait]
pub trait CustomerReadPort: Send + Sync {
    async fn read_customer_projection(
        &self,
        context: PortContext,
        request: CustomerProjectionRequest,
    ) -> Result<CustomerResponse, PortError>;

    async fn read_customer_projection_by_user(
        &self,
        context: PortContext,
        request: CustomerUserProjectionRequest,
    ) -> Result<CustomerResponse, PortError>;

    async fn list_customer_projections(
        &self,
        context: PortContext,
        request: CustomerListProjectionRequest,
    ) -> Result<CustomerListProjectionResponse, PortError>;

    async fn list_profile_enrichment(
        &self,
        context: PortContext,
        request: CustomerProfileEnrichmentRequest,
    ) -> Result<Vec<CustomerProfileEnrichment>, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomerProjectionRequest {
    pub customer_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomerUserProjectionRequest {
    pub user_id: Uuid,
}

/// Builds the owner-managed in-process read provider for explicit consumers.
pub fn in_process_customer_read_port(db: DatabaseConnection) -> Arc<dyn CustomerReadPort> {
    Arc::new(crate::CustomerService::new(db))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomerListProjectionRequest {
    pub search: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerListProjectionResponse {
    pub items: Vec<CustomerResponse>,
    pub total: u64,
}

/// Customer-owned optional identity enrichments for profile provisioning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomerProfileEnrichmentRequest {
    pub user_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomerProfileEnrichment {
    pub user_id: Uuid,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub preferred_locale: Option<String>,
}

#[async_trait]
impl CustomerReadPort for crate::CustomerService {
    async fn read_customer_projection(
        &self,
        context: PortContext,
        request: CustomerProjectionRequest,
    ) -> Result<CustomerResponse, PortError> {
        let owner_operation = "read_customer_projection";
        require_customer_read_policy(&context, owner_operation)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        self.get_customer(tenant_id, request.customer_id)
            .await
            .map_err(|error| customer_error_to_port_error(&context, owner_operation, error))
    }

    async fn read_customer_projection_by_user(
        &self,
        context: PortContext,
        request: CustomerUserProjectionRequest,
    ) -> Result<CustomerResponse, PortError> {
        let owner_operation = "read_customer_projection_by_user";
        require_customer_read_policy(&context, owner_operation)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        self.get_customer_by_user(tenant_id, request.user_id)
            .await
            .map_err(|error| customer_error_to_port_error(&context, owner_operation, error))
    }

    async fn list_customer_projections(
        &self,
        context: PortContext,
        request: CustomerListProjectionRequest,
    ) -> Result<CustomerListProjectionResponse, PortError> {
        let owner_operation = "list_customer_projections";
        require_customer_read_policy(&context, owner_operation)?;
        validate_customer_list_projection_request(&context, owner_operation, &request)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        let (items, total) = self
            .list_customers(
                tenant_id,
                ListCustomersInput {
                    search: request.search,
                    page: request.page,
                    per_page: request.per_page,
                },
            )
            .await
            .map_err(|error| customer_error_to_port_error(&context, owner_operation, error))?;
        Ok(CustomerListProjectionResponse { items, total })
    }

    async fn list_profile_enrichment(
        &self,
        context: PortContext,
        request: CustomerProfileEnrichmentRequest,
    ) -> Result<Vec<CustomerProfileEnrichment>, PortError> {
        let owner_operation = "list_profile_enrichment";
        require_customer_read_policy(&context, owner_operation)?;
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        crate::CustomerService::list_profile_enrichment(self, tenant_id, &request.user_ids)
            .await
            .map_err(|error| customer_error_to_port_error(&context, owner_operation, error))
    }
}

fn require_customer_read_policy(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<(), PortError> {
    context
        .require_policy(PortCallPolicy::read())
        .map_err(|error| {
            tracing::warn!(
                error = ?error,
                owner = "rustok_customer",
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                actor = ?context.actor,
                channel = ?context.channel,
                locale = %context.locale,
                causation_id = ?context.causation_id,
                traceparent = ?context.traceparent,
                deadline_ms = ?context.deadline_ms,
                operation = owner_operation,
                code = %error.code,
                error_kind = ?error.kind,
                retryable = error.retryable,
                boundary = CUSTOMER_READ_PORT_BOUNDARY,
                "customer read port admission was rejected"
            );
            error
        })
}

fn validate_customer_list_projection_request(
    context: &PortContext,
    owner_operation: &'static str,
    request: &CustomerListProjectionRequest,
) -> Result<(), PortError> {
    if request.page == 0 {
        tracing::warn!(
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "customer.page_invalid",
            "customer projection page is invalid"
        );
        return Err(PortError::validation(
            "customer.page_invalid",
            "customer projection page is invalid",
        ));
    }
    if !(1..=MAX_CUSTOMERS_PER_PAGE).contains(&request.per_page) {
        tracing::warn!(
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "customer.per_page_invalid",
            "customer projection page size is invalid"
        );
        return Err(PortError::validation(
            "customer.per_page_invalid",
            "customer projection page size is invalid",
        ));
    }
    Ok(())
}

fn parse_port_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|error| {
        tracing::warn!(
            error = ?error,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "customer.context_invalid",
            "customer port context is invalid"
        );
        PortError::validation(
            "customer.context_invalid",
            "customer request context is invalid",
        )
    })
}

struct CustomerReadContextFacts {
    tenant_id_length: usize,
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

struct CustomerOwnerErrorFacts {
    error_variant: &'static str,
    text_field_count: usize,
    text_total_length: usize,
    uuid_field_count: usize,
    uuid_non_nil_count: usize,
    opaque_payload_present: bool,
}

fn customer_read_context_facts(context: &PortContext) -> CustomerReadContextFacts {
    let actor_kind = match &context.actor.kind {
        rustok_api::PortActorKind::User => "user",
        rustok_api::PortActorKind::Service => "service",
        rustok_api::PortActorKind::System => "system",
    };
    CustomerReadContextFacts {
        tenant_id_length: context.tenant_id.chars().count(),
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

fn customer_owner_error_facts(error: &CustomerError) -> CustomerOwnerErrorFacts {
    let (
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    ) = match error {
        CustomerError::Validation(message) => {
            ("validation", 1, message.chars().count(), 0, 0, false)
        }
        CustomerError::CustomerNotFound(customer_id) => (
            "customer_not_found",
            0,
            0,
            1,
            if customer_id.is_nil() { 0 } else { 1 },
            false,
        ),
        CustomerError::CustomerByUserNotFound(user_id) => (
            "customer_by_user_not_found",
            0,
            0,
            1,
            if user_id.is_nil() { 0 } else { 1 },
            false,
        ),
        CustomerError::DuplicateEmail(email) => {
            ("duplicate_email", 1, email.chars().count(), 0, 0, false)
        }
        CustomerError::DuplicateUserLink(user_id) => (
            "duplicate_user_link",
            0,
            0,
            1,
            if user_id.is_nil() { 0 } else { 1 },
            false,
        ),
        CustomerError::Profile(_) => ("profile", 0, 0, 0, 0, true),
        CustomerError::Database(_) => ("database", 0, 0, 0, 0, true),
    };
    CustomerOwnerErrorFacts {
        error_variant,
        text_field_count,
        text_total_length,
        uuid_field_count,
        uuid_non_nil_count,
        opaque_payload_present,
    }
}

fn log_customer_owner_failure(
    context: &PortContext,
    owner_operation: &'static str,
    code: &'static str,
    error_facts: &CustomerOwnerErrorFacts,
    technical_failure: bool,
) {
    let context_facts = customer_read_context_facts(context);
    if technical_failure {
        tracing::error!(
            owner = "rustok_customer",
            correlation_id = %context.correlation_id,
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
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = CUSTOMER_READ_PORT_BOUNDARY,
            "customer owner operation failed with bounded diagnostics"
        );
    } else {
        tracing::warn!(
            owner = "rustok_customer",
            correlation_id = %context.correlation_id,
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
            operation = owner_operation,
            code,
            error_variant = error_facts.error_variant,
            text_field_count = error_facts.text_field_count,
            text_total_length = error_facts.text_total_length,
            uuid_field_count = error_facts.uuid_field_count,
            uuid_non_nil_count = error_facts.uuid_non_nil_count,
            opaque_payload_present = error_facts.opaque_payload_present,
            boundary = CUSTOMER_READ_PORT_BOUNDARY,
            "customer owner operation was rejected with bounded diagnostics"
        );
    }
}

fn customer_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: CustomerError,
) -> PortError {
    let error_facts = customer_owner_error_facts(&error);
    match error {
        CustomerError::Database(_) => {
            log_customer_owner_failure(
                context,
                owner_operation,
                "customer.database_unavailable",
                &error_facts,
                true,
            );
            PortError::unavailable(
                "customer.database_unavailable",
                "customer storage is temporarily unavailable",
            )
        }
        CustomerError::CustomerNotFound(_) => {
            log_customer_owner_failure(
                context,
                owner_operation,
                "customer.customer_not_found",
                &error_facts,
                false,
            );
            PortError::not_found("customer.customer_not_found", "customer was not found")
        }
        CustomerError::CustomerByUserNotFound(_) => {
            log_customer_owner_failure(
                context,
                owner_operation,
                "customer.customer_by_user_not_found",
                &error_facts,
                false,
            );
            PortError::not_found(
                "customer.customer_by_user_not_found",
                "customer was not found for the requested user",
            )
        }
        CustomerError::DuplicateEmail(_) => {
            log_customer_owner_failure(
                context,
                owner_operation,
                "customer.duplicate_email",
                &error_facts,
                false,
            );
            PortError::conflict(
                "customer.duplicate_email",
                "customer email is already in use",
            )
        }
        CustomerError::DuplicateUserLink(_) => {
            log_customer_owner_failure(
                context,
                owner_operation,
                "customer.duplicate_user_link",
                &error_facts,
                false,
            );
            PortError::conflict(
                "customer.duplicate_user_link",
                "customer user link already exists",
            )
        }
        CustomerError::Validation(_) => {
            log_customer_owner_failure(
                context,
                owner_operation,
                "customer.validation",
                &error_facts,
                false,
            );
            PortError::validation("customer.validation", "customer request is invalid")
        }
        CustomerError::Profile(_) => {
            log_customer_owner_failure(
                context,
                owner_operation,
                "customer.profile_unavailable",
                &error_facts,
                true,
            );
            PortError::unavailable(
                "customer.profile_unavailable",
                "customer profile projection is temporarily unavailable",
            )
        }
    }
}
