use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::RegionResponse;

/// Transport-neutral selector for region read-projection consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionReadSelector {
    Id(Uuid),
    CountryCode(String),
}

/// Transport-neutral request for region read-projection consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionReadRequest {
    pub selector: RegionReadSelector,
    pub requested_locale: Option<String>,
    pub tenant_default_locale: Option<String>,
}

/// Transport-neutral request for region list consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionListRequest {
    pub requested_locale: Option<String>,
    pub tenant_default_locale: Option<String>,
}

/// Transport-neutral region projection exposed by the region owner module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegionReadProjection {
    pub region: RegionResponse,
}

/// Transport-neutral owner boundary for region read projections.
#[async_trait]
pub trait RegionReadPort: Send + Sync {
    async fn read_region(
        &self,
        context: PortContext,
        request: RegionReadRequest,
    ) -> Result<Option<RegionReadProjection>, PortError>;

    async fn list_regions_for_tenant(
        &self,
        context: PortContext,
        request: RegionListRequest,
    ) -> Result<Vec<RegionReadProjection>, PortError>;
}
