mod native_server_adapter;

pub use native_server_adapter::ApiError;

use crate::model::{CustomerAdminBootstrap, CustomerDetail, CustomerDraft, CustomerList};
use native_server_adapter as native;

pub async fn fetch_bootstrap() -> Result<CustomerAdminBootstrap, ApiError> {
    native::fetch_bootstrap().await
}

pub async fn fetch_customers(
    search: String,
    page: u64,
    per_page: u64,
) -> Result<CustomerList, ApiError> {
    native::fetch_customers(search, page, per_page).await
}

pub async fn fetch_customer_detail(customer_id: String) -> Result<CustomerDetail, ApiError> {
    native::fetch_customer_detail(customer_id).await
}

pub async fn create_customer(payload: CustomerDraft) -> Result<CustomerDetail, ApiError> {
    native::create_customer(payload).await
}

pub async fn update_customer(
    customer_id: String,
    payload: CustomerDraft,
) -> Result<CustomerDetail, ApiError> {
    native::update_customer(customer_id, payload).await
}
