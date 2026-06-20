use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::{CustomerResponse, ListCustomersInput};
use crate::error::CustomerError;

/// Transport-neutral owner boundary for customer read projections used by checkout/order flows.
#[async_trait]
pub trait CustomerReadPort: Send + Sync {
    async fn read_customer_projection(
        &self,
        context: PortContext,
        request: CustomerProjectionRequest,
    ) -> Result<CustomerResponse, PortError>;

    async fn list_customer_projections(
        &self,
        context: PortContext,
        request: CustomerListProjectionRequest,
    ) -> Result<CustomerListProjectionResponse, PortError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomerProjectionRequest {
    pub customer_id: Uuid,
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

#[async_trait]
impl CustomerReadPort for crate::CustomerService {
    async fn read_customer_projection(
        &self,
        context: PortContext,
        request: CustomerProjectionRequest,
    ) -> Result<CustomerResponse, PortError> {
        context.require_deadline_semantics()?;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
        self.get_customer(tenant_id, request.customer_id)
            .await
            .map_err(customer_error_to_port_error)
    }

    async fn list_customer_projections(
        &self,
        context: PortContext,
        request: CustomerListProjectionRequest,
    ) -> Result<CustomerListProjectionResponse, PortError> {
        context.require_deadline_semantics()?;
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_port_tenant_id(&context)?;
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
            .map_err(customer_error_to_port_error)?;
        Ok(CustomerListProjectionResponse { items, total })
    }
}

fn parse_port_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "customer.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for customer ports",
        )
    })
}

fn customer_error_to_port_error(error: CustomerError) -> PortError {
    match error {
        CustomerError::Database(error) => PortError::unavailable(
            "customer.database_unavailable",
            format!("customer storage unavailable: {error}"),
        ),
        CustomerError::CustomerNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "customer.customer_not_found",
            format!("customer {id} not found"),
            false,
        ),
        CustomerError::CustomerByUserNotFound(id) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "customer.customer_by_user_not_found",
            format!("customer for user {id} not found"),
            false,
        ),
        CustomerError::DuplicateEmail(email) => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "customer.duplicate_email",
            format!("duplicate customer email `{email}`"),
            false,
        ),
        CustomerError::DuplicateUserLink(user_id) => PortError::new(
            rustok_api::PortErrorKind::Conflict,
            "customer.duplicate_user_link",
            format!("customer already linked to user {user_id}"),
            false,
        ),
        CustomerError::Validation(message) => PortError::validation("customer.validation", message),
        CustomerError::Profile(error) => PortError::unavailable(
            "customer.profile_unavailable",
            format!("customer profile projection unavailable: {error}"),
        ),
    }
}
