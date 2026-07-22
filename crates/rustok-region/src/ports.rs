use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
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

#[async_trait]
impl RegionReadPort for crate::RegionService {
    async fn read_region(
        &self,
        context: PortContext,
        request: RegionReadRequest,
    ) -> Result<Option<RegionReadProjection>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        validate_region_read_request(&request)?;

        let result = match request.selector {
            RegionReadSelector::Id(region_id) => self
                .get_region(
                    tenant_id,
                    region_id,
                    request.requested_locale.as_deref(),
                    request.tenant_default_locale.as_deref(),
                )
                .await
                .map(Some),
            RegionReadSelector::CountryCode(country_code) => {
                self.resolve_region_for_country(
                    tenant_id,
                    &country_code,
                    request.requested_locale.as_deref(),
                    request.tenant_default_locale.as_deref(),
                )
                .await
            }
        }
        .map_err(map_region_error)?;

        Ok(result.map(|region| RegionReadProjection { region }))
    }

    async fn list_regions_for_tenant(
        &self,
        context: PortContext,
        request: RegionListRequest,
    ) -> Result<Vec<RegionReadProjection>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context)?;
        self.list_regions(
            tenant_id,
            request.requested_locale.as_deref(),
            request.tenant_default_locale.as_deref(),
        )
        .await
        .map_err(map_region_error)
        .map(|regions| {
            regions
                .into_iter()
                .map(|region| RegionReadProjection { region })
                .collect()
        })
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    context.tenant_id.parse::<Uuid>().map_err(|_| {
        PortError::validation(
            "region.tenant_id_invalid",
            "region read port requires a UUID tenant_id in context",
        )
    })
}

fn validate_region_read_request(request: &RegionReadRequest) -> Result<(), PortError> {
    if let RegionReadSelector::CountryCode(country_code) = &request.selector {
        if country_code.trim().is_empty() {
            return Err(PortError::validation(
                "region.country_code_empty",
                "region read port requires a non-empty country code selector",
            ));
        }
    }
    Ok(())
}

fn map_region_error(error: crate::RegionError) -> PortError {
    match error {
        crate::RegionError::RegionNotFound(_) => {
            PortError::not_found("region.not_found", "region read projection was not found")
        }
        crate::RegionError::Validation(message)
        | crate::RegionError::InvalidCountryCode(message) => {
            PortError::validation("region.validation", message)
        }
        crate::RegionError::Database(error) => {
            tracing::error!(error = ?error, "region port storage operation failed");
            PortError::unavailable(
                "region.read_failed",
                "region storage is temporarily unavailable",
            )
        }
    }
}
