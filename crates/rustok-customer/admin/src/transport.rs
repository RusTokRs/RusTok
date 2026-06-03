use crate::api::{self, ApiError};
use crate::model::{CustomerAdminBootstrap, CustomerDetail, CustomerDraft, CustomerList};

pub async fn fetch_bootstrap() -> Result<CustomerAdminBootstrap, ApiError> {
    api::fetch_bootstrap().await
}

pub async fn fetch_customers(
    search: String,
    page: u64,
    per_page: u64,
) -> Result<CustomerList, ApiError> {
    api::fetch_customers(search, page, per_page).await
}

pub async fn fetch_customer_detail(customer_id: String) -> Result<CustomerDetail, ApiError> {
    api::fetch_customer_detail(customer_id).await
}

pub async fn create_customer(payload: CustomerDraft) -> Result<CustomerDetail, ApiError> {
    api::create_customer(payload).await
}

pub async fn update_customer(
    customer_id: String,
    payload: CustomerDraft,
) -> Result<CustomerDetail, ApiError> {
    api::update_customer(customer_id, payload).await
}
