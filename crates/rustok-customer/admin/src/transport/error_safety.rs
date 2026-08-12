use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::time::{SystemTime, UNIX_EPOCH};

const CUSTOMER_ADMIN_CLIENT_OWNER: &str = "rustok_customer.admin";
const CUSTOMER_ADMIN_CLIENT_BOUNDARY: &str = "customer_admin_client_transport";
const CUSTOMER_ADMIN_CLIENT_PUBLIC_MESSAGE: &str = "Customer admin request could not be completed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn,
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn => f.write_str(CUSTOMER_ADMIN_CLIENT_PUBLIC_MESSAGE),
        }
    }
}

impl std::error::Error for ApiError {}

pub(super) struct CustomerAdminTransportErrorContext {
    operation: &'static str,
    correlation_id: String,
    subject_id_length: Option<usize>,
    search_length: Option<usize>,
    pagination_present: bool,
    payload_present: bool,
}

impl CustomerAdminTransportErrorContext {
    pub(super) fn for_bootstrap() -> Self {
        Self::new("fetch_bootstrap")
    }

    pub(super) fn for_customers(search: &str) -> Self {
        let mut context = Self::new("fetch_customers");
        context.search_length = Some(search.chars().count());
        context.pagination_present = true;
        context
    }

    pub(super) fn for_customer_detail(customer_id: &str) -> Self {
        let mut context = Self::new("fetch_customer_detail");
        context.subject_id_length = Some(customer_id.chars().count());
        context
    }

    pub(super) fn for_create_customer() -> Self {
        let mut context = Self::new("create_customer");
        context.payload_present = true;
        context
    }

    pub(super) fn for_update_customer(customer_id: &str) -> Self {
        let mut context = Self::new("update_customer");
        context.subject_id_length = Some(customer_id.chars().count());
        context.payload_present = true;
        context
    }

    fn new(operation: &'static str) -> Self {
        Self {
            operation,
            correlation_id: customer_admin_client_correlation_id(operation),
            subject_id_length: None,
            search_length: None,
            pagination_present: false,
            payload_present: false,
        }
    }

    pub(super) fn map_error<E: std::fmt::Debug>(&self, error: E) -> ApiError {
        tracing::error!(
            raw_error = ?error,
            owner = CUSTOMER_ADMIN_CLIENT_OWNER,
            owner_operation = self.operation,
            correlation_id = %self.correlation_id,
            subject_id_present = self.subject_id_length.is_some(),
            subject_id_length = ?self.subject_id_length,
            search_present = self.search_length.is_some(),
            search_length = ?self.search_length,
            pagination_present = self.pagination_present,
            payload_present = self.payload_present,
            code = "customer.admin_client_transport_failed",
            boundary = CUSTOMER_ADMIN_CLIENT_BOUNDARY,
            "customer admin client transport request failed"
        );

        ApiError::ServerFn
    }
}

fn customer_admin_client_correlation_id(operation: &'static str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("customer-admin-client:{operation}:{timestamp}")
}
