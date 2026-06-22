use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use rustok_api::{PortActor, PortCallPolicy, PortContext, PortError, PortErrorKind};

/// Transport-neutral selector for tenant resolution/read consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantReadSelector {
    Id(Uuid),
    Slug(String),
    Domain(String),
}

/// Transport-neutral request for tenant read-projection consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantReadRequest {
    pub selector: TenantReadSelector,
    pub include_inactive: bool,
}

/// Transport-neutral tenant projection exposed by the tenant owner module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantReadProjection {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
    pub is_active: bool,
    pub default_locale: String,
    pub settings: serde_json::Value,
}

/// Transport-neutral owner boundary for tenant read projections.
#[async_trait]
pub trait TenantReadPort: Send + Sync {
    async fn read_tenant(
        &self,
        context: PortContext,
        request: TenantReadRequest,
    ) -> Result<TenantReadProjection, PortError>;
}

#[async_trait]
impl TenantReadPort for crate::TenantService {
    async fn read_tenant(
        &self,
        context: PortContext,
        request: TenantReadRequest,
    ) -> Result<TenantReadProjection, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_tenant_read_request(&request)?;

        let tenant = match request.selector {
            TenantReadSelector::Id(id) => self.get_tenant(id).await,
            TenantReadSelector::Slug(slug) => self.get_tenant_by_slug(&slug).await,
            TenantReadSelector::Domain(domain) => self.get_tenant_by_domain(&domain).await,
        }
        .map_err(map_tenant_error)?;

        if !request.include_inactive && !tenant.is_active {
            return Err(PortError::new(
                PortErrorKind::NotFound,
                "tenant.inactive",
                "tenant read port hides inactive tenants unless explicitly requested",
                false,
            ));
        }

        Ok(TenantReadProjection {
            id: tenant.id,
            name: tenant.name,
            slug: tenant.slug,
            domain: tenant.domain,
            is_active: tenant.is_active,
            default_locale: "en".to_string(),
            settings: tenant.settings,
        })
    }
}

fn validate_tenant_read_request(request: &TenantReadRequest) -> Result<(), PortError> {
    match &request.selector {
        TenantReadSelector::Slug(slug) if slug.trim().is_empty() => {
            return Err(PortError::new(
                PortErrorKind::Validation,
                "tenant.slug_empty",
                "tenant read port requires a non-empty slug selector",
                false,
            ));
        }
        TenantReadSelector::Domain(domain) if domain.trim().is_empty() => {
            return Err(PortError::new(
                PortErrorKind::Validation,
                "tenant.domain_empty",
                "tenant read port requires a non-empty domain selector",
                false,
            ));
        }
        _ => {}
    }
    Ok(())
}

fn map_tenant_error(error: crate::TenantError) -> PortError {
    match error {
        crate::TenantError::NotFound => PortError::new(
            PortErrorKind::NotFound,
            "tenant.not_found",
            "tenant read projection was not found",
            false,
        ),
        other => PortError::new(
            PortErrorKind::Unavailable,
            "tenant.read_failed",
            other.to_string(),
            true,
        ),
    }
}
