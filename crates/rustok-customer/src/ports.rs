use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::{CustomerResponse, ListCustomersInput};
use crate::error::CustomerError;

const MAX_CUSTOMERS_PER_PAGE: u64 = 100;

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
        context.require_policy(PortCallPolicy::read())?;
        let owner_operation = "read_customer_projection";
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
        context.require_policy(PortCallPolicy::read())?;
        let owner_operation = "read_customer_projection_by_user";
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
        context.require_policy(PortCallPolicy::read())?;
        let owner_operation = "list_customer_projections";
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
        context.require_policy(PortCallPolicy::read())?;
        let owner_operation = "list_profile_enrichment";
        let tenant_id = parse_port_tenant_id(&context, owner_operation)?;
        crate::CustomerService::list_profile_enrichment(self, tenant_id, &request.user_ids)
            .await
            .map_err(|error| customer_error_to_port_error(&context, owner_operation, error))
    }
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

fn customer_error_to_port_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: CustomerError,
) -> PortError {
    match error {
        CustomerError::Database(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "customer.database_unavailable",
                "customer owner storage operation failed"
            );
            PortError::unavailable(
                "customer.database_unavailable",
                "customer storage is temporarily unavailable",
            )
        }
        CustomerError::CustomerNotFound(customer_id) => {
            tracing::warn!(
                customer_id = %customer_id,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "customer.customer_not_found",
                "customer projection was not found"
            );
            PortError::not_found("customer.customer_not_found", "customer was not found")
        }
        CustomerError::CustomerByUserNotFound(user_id) => {
            tracing::warn!(
                user_id = %user_id,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "customer.customer_by_user_not_found",
                "customer projection was not found for user"
            );
            PortError::not_found(
                "customer.customer_by_user_not_found",
                "customer was not found for the requested user",
            )
        }
        CustomerError::DuplicateEmail(_) => {
            tracing::warn!(
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "customer.duplicate_email",
                "customer email conflict"
            );
            PortError::conflict(
                "customer.duplicate_email",
                "customer email is already in use",
            )
        }
        CustomerError::DuplicateUserLink(user_id) => {
            tracing::warn!(
                user_id = %user_id,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "customer.duplicate_user_link",
                "customer user link conflict"
            );
            PortError::conflict(
                "customer.duplicate_user_link",
                "customer user link already exists",
            )
        }
        CustomerError::Validation(message) => {
            tracing::warn!(
                error = %message,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "customer.validation",
                "customer owner validation failed"
            );
            PortError::validation("customer.validation", "customer request is invalid")
        }
        CustomerError::Profile(error) => {
            tracing::error!(
                error = ?error,
                correlation_id = %context.correlation_id,
                tenant_id = %context.tenant_id,
                operation = owner_operation,
                code = "customer.profile_unavailable",
                "customer profile projection failed"
            );
            PortError::unavailable(
                "customer.profile_unavailable",
                "customer profile projection is temporarily unavailable",
            )
        }
    }
}
