pub mod native_server_adapter;

pub use native_server_adapter::ApiError;

use crate::model::TenantAdminBootstrap;

pub async fn fetch_bootstrap() -> Result<TenantAdminBootstrap, ApiError> {
    native_server_adapter::tenant_bootstrap_native()
        .await
        .map_err(Into::into)
}
