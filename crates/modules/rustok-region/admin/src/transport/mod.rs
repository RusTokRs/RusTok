mod native_server_adapter;

use crate::model::{RegionAdminBootstrap, RegionDetail, RegionDraft, RegionList};

pub type TransportError = native_server_adapter::ApiError;

pub async fn fetch_bootstrap() -> Result<RegionAdminBootstrap, TransportError> {
    native_server_adapter::fetch_bootstrap().await
}

pub async fn fetch_regions() -> Result<RegionList, TransportError> {
    native_server_adapter::fetch_regions().await
}

pub async fn fetch_region_detail(region_id: String) -> Result<RegionDetail, TransportError> {
    native_server_adapter::fetch_region_detail(region_id).await
}

pub async fn create_region(payload: RegionDraft) -> Result<RegionDetail, TransportError> {
    native_server_adapter::create_region(payload).await
}

pub async fn update_region(
    region_id: String,
    payload: RegionDraft,
) -> Result<RegionDetail, TransportError> {
    native_server_adapter::update_region(region_id, payload).await
}
