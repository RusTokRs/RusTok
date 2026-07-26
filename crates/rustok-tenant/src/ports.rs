use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use rustok_api::{
    PortActor, PortCallPolicy, PortContext, PortError, PortErrorKind, TenantLocale,
};

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

    async fn read_default_active_tenant(
        &self,
        context: PortContext,
    ) -> Result<TenantReadProjection, PortError>;
}

/// One canonical tenant locale-policy entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantLocalePolicyEntry {
    pub locale: TenantLocale,
    pub name: String,
    pub native_name: String,
    pub is_default: bool,
    pub is_enabled: bool,
    pub fallback_locale: Option<TenantLocale>,
}

/// Revisioned tenant-owned locale policy consumed by runtime and translation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantLocalePolicyProjection {
    pub tenant_id: Uuid,
    pub revision: i64,
    pub default_locale: TenantLocale,
    pub locales: Vec<TenantLocalePolicyEntry>,
}

/// Atomic replacement command for the locale-policy aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceTenantLocalePolicyRequest {
    pub expected_revision: i64,
    pub locales: Vec<TenantLocalePolicyEntry>,
}

/// Tenant-owned locale-policy boundary.
#[async_trait]
pub trait TenantLocalePolicyPort: Send + Sync {
    async fn read_locale_policy(
        &self,
        context: PortContext,
    ) -> Result<TenantLocalePolicyProjection, PortError>;

    async fn replace_locale_policy(
        &self,
        context: PortContext,
        request: ReplaceTenantLocalePolicyRequest,
    ) -> Result<TenantLocalePolicyProjection, PortError>;
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
            default_locale: tenant.default_locale,
            settings: tenant.settings,
        })
    }

    async fn read_default_active_tenant(
        &self,
        context: PortContext,
    ) -> Result<TenantReadProjection, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant = self.first_active_tenant().await.map_err(map_tenant_error)?;

        Ok(TenantReadProjection {
            id: tenant.id,
            name: tenant.name,
            slug: tenant.slug,
            domain: tenant.domain,
            is_active: tenant.is_active,
            default_locale: tenant.default_locale,
            settings: tenant.settings,
        })
    }
}

#[async_trait]
impl TenantLocalePolicyPort for crate::TenantService {
    async fn read_locale_policy(
        &self,
        context: PortContext,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let tenant_id = parse_context_tenant_id(&context)?;
        self.read_locale_policy_owned(tenant_id)
            .await
            .map_err(map_tenant_error)
    }

    async fn replace_locale_policy(
        &self,
        context: PortContext,
        request: ReplaceTenantLocalePolicyRequest,
    ) -> Result<TenantLocalePolicyProjection, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_context_tenant_id(&context)?;
        let idempotency_key = context
            .idempotency_key
            .as_deref()
            .ok_or_else(|| {
                PortError::validation(
                    "tenant.locale_policy_idempotency_required",
                    "tenant locale policy writes require an idempotency key",
                )
            })?
            .to_string();
        self.replace_locale_policy_owned(tenant_id, request, &idempotency_key)
            .await
            .map_err(map_tenant_error)
    }
}

fn parse_context_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        PortError::validation(
            "tenant.context_tenant_id_invalid",
            "tenant port context requires a UUID tenant_id",
        )
    })
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
        crate::TenantError::InvalidLocalePolicy(message) => {
            PortError::validation("tenant.locale_policy_invalid", message)
        }
        crate::TenantError::LocalePolicyConflict { expected, actual } => PortError::conflict(
            "tenant.locale_policy_conflict",
            format!("tenant locale policy revision conflict: expected {expected}, actual {actual}"),
        ),
        crate::TenantError::LocalePolicyIdempotencyConflict => PortError::conflict(
            "tenant.locale_policy_idempotency_conflict",
            "tenant locale policy idempotency key was already used for a different request",
        ),
        crate::TenantError::LocalePolicyInvariant(message) => {
            PortError::invariant_violation("tenant.locale_policy_invariant", message)
        }
        other => PortError::new(
            PortErrorKind::Unavailable,
            "tenant.read_failed",
            other.to_string(),
            true,
        ),
    }
}
