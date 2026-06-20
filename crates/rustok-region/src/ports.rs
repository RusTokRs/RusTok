use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::RegionResponse;

/// Transport-agnostic region port context for host/runtime boundary calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortContext {
    pub tenant_id: String,
    pub correlation_id: String,
    pub deadline_ms: Option<u64>,
}

impl PortContext {
    pub fn require_deadline_semantics(&self) -> Result<(), PortError> {
        if self.deadline_ms.unwrap_or_default() == 0 {
            return Err(PortError::new(
                PortErrorKind::Timeout,
                "port.deadline_required",
                "region read port calls require deadline semantics",
                true,
            ));
        }
        Ok(())
    }
}

/// Transport-neutral error returned by region owner ports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortError {
    pub kind: PortErrorKind,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl PortError {
    pub fn new(
        kind: PortErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortErrorKind {
    Validation,
    NotFound,
    Unavailable,
    Timeout,
}

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
        context.require_deadline_semantics()?;
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
        context.require_deadline_semantics()?;
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
        PortError::new(
            PortErrorKind::Validation,
            "region.tenant_id_invalid",
            "region read port requires a UUID tenant_id in context",
            false,
        )
    })
}

fn validate_region_read_request(request: &RegionReadRequest) -> Result<(), PortError> {
    if let RegionReadSelector::CountryCode(country_code) = &request.selector {
        if country_code.trim().is_empty() {
            return Err(PortError::new(
                PortErrorKind::Validation,
                "region.country_code_empty",
                "region read port requires a non-empty country code selector",
                false,
            ));
        }
    }
    Ok(())
}

fn map_region_error(error: crate::RegionError) -> PortError {
    match error {
        crate::RegionError::RegionNotFound(_) => PortError::new(
            PortErrorKind::NotFound,
            "region.not_found",
            "region read projection was not found",
            false,
        ),
        crate::RegionError::Validation(message)
        | crate::RegionError::InvalidCountryCode(message) => PortError::new(
            PortErrorKind::Validation,
            "region.validation",
            message,
            false,
        ),
        crate::RegionError::Database(error) => PortError::new(
            PortErrorKind::Unavailable,
            "region.read_failed",
            error.to_string(),
            true,
        ),
    }
}
