mod error_safety;
mod native_server_adapter;

pub use error_safety::ApiError;

use crate::model::{CustomerAdminBootstrap, CustomerDetail, CustomerDraft, CustomerList};
use error_safety::CustomerAdminTransportErrorContext;
use native_server_adapter as native;

pub async fn fetch_bootstrap() -> Result<CustomerAdminBootstrap, ApiError> {
    let context = CustomerAdminTransportErrorContext::for_bootstrap();
    native::fetch_bootstrap()
        .await
        .map_err(|server_error| context.map_error(server_error))
}

pub async fn fetch_customers(
    search: String,
    page: u64,
    per_page: u64,
) -> Result<CustomerList, ApiError> {
    let context = CustomerAdminTransportErrorContext::for_customers(search.as_str());
    native::fetch_customers(search, page, per_page)
        .await
        .map_err(|server_error| context.map_error(server_error))
}

pub async fn fetch_customer_detail(customer_id: String) -> Result<CustomerDetail, ApiError> {
    let context = CustomerAdminTransportErrorContext::for_customer_detail(customer_id.as_str());
    native::fetch_customer_detail(customer_id)
        .await
        .map_err(|server_error| context.map_error(server_error))
}

pub async fn create_customer(payload: CustomerDraft) -> Result<CustomerDetail, ApiError> {
    let context = CustomerAdminTransportErrorContext::for_create_customer();
    native::create_customer(payload)
        .await
        .map_err(|server_error| context.map_error(server_error))
}

pub async fn update_customer(
    customer_id: String,
    payload: CustomerDraft,
) -> Result<CustomerDetail, ApiError> {
    let context = CustomerAdminTransportErrorContext::for_update_customer(customer_id.as_str());
    native::update_customer(customer_id, payload)
        .await
        .map_err(|server_error| context.map_error(server_error))
}
