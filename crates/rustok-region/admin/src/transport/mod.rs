use crate::api;
use crate::model::{RegionAdminBootstrap, RegionDetail, RegionDraft, RegionList};

pub type TransportError = api::ApiError;

pub async fn fetch_bootstrap() -> Result<RegionAdminBootstrap, TransportError> {
    api::fetch_bootstrap().await
}

pub async fn fetch_regions() -> Result<RegionList, TransportError> {
    api::fetch_regions().await
}

pub async fn fetch_region_detail(region_id: String) -> Result<RegionDetail, TransportError> {
    api::fetch_region_detail(region_id).await
}

pub async fn create_region(payload: RegionDraft) -> Result<RegionDetail, TransportError> {
    api::create_region(payload).await
}

pub async fn update_region(
    region_id: String,
    payload: RegionDraft,
) -> Result<RegionDetail, TransportError> {
    api::update_region(region_id, payload).await
}
