use std::{sync::Arc, time::Duration};

use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, PortActor, PortContext, PortError, PortErrorKind, TenantLocale,
};
use sea_orm::DatabaseConnection;
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

use rustok_region::dto::RegionResponse;
use rustok_region::{RegionReadPort, RegionReadRequest, RegionReadSelector};
use rustok_tenant::{
    TenantLocalePolicyPort, TenantReadPort, TenantReadRequest, TenantReadSelector, TenantService,
};

use crate::dto::{ResolveStoreContextInput, StoreContextResponse};

const STORE_CONTEXT_PORT_TIMEOUT: Duration = Duration::from_secs(3);

pub type StoreContextResult<T> = Result<T, StoreContextError>;

#[derive(Debug, Error)]
pub enum StoreContextError {
    #[error("tenant {0} not found")]
    TenantNotFound(Uuid),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error(
        "currency `{currency_code}` does not match region currency `{region_currency_code}` for region {region_id}"
    )]
    CurrencyRegionMismatch {
        currency_code: String,
        region_currency_code: String,
        region_id: Uuid,
    },
    #[error("tenant boundary `{code}` failed: {message}")]
    TenantBoundary { code: String, message: String },
    #[error("region boundary `{code}` failed: {message}")]
    RegionBoundary { code: String, message: String },
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub struct StoreContextService {
    tenant_read_port: Arc<dyn TenantReadPort>,
    tenant_locale_policy_port: Arc<dyn TenantLocalePolicyPort>,
    region_read_port: Arc<dyn RegionReadPort>,
}

impl StoreContextService {
    pub fn new(db: DatabaseConnection, region_read_port: Arc<dyn RegionReadPort>) -> Self {
        let tenant_service = Arc::new(TenantService::new(db));
        let tenant_read_port: Arc<dyn TenantReadPort> = tenant_service.clone();
        let tenant_locale_policy_port: Arc<dyn TenantLocalePolicyPort> = tenant_service;
        Self {
            tenant_read_port,
            tenant_locale_policy_port,
            region_read_port,
        }
    }

    pub fn with_ports(
        tenant_read_port: Arc<dyn TenantReadPort>,
        tenant_locale_policy_port: Arc<dyn TenantLocalePolicyPort>,
        region_read_port: Arc<dyn RegionReadPort>,
    ) -> Self {
        Self {
            tenant_read_port,
            tenant_locale_policy_port,
            region_read_port,
        }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn resolve_context(
        &self,
        tenant_id: Uuid,
        input: ResolveStoreContextInput,
    ) -> StoreContextResult<StoreContextResponse> {
        let (default_locale, mut available_locales) =
            self.load_tenant_locale_context(tenant_id).await?;
        if available_locales.is_empty() {
            available_locales.push(default_locale.clone());
        }
        if !available_locales.contains(&default_locale) {
            available_locales.insert(0, default_locale.clone());
        }

        let requested_locale = input.locale.as_deref().map(normalize_locale).transpose()?;
        let locale = requested_locale
            .as_ref()
            .filter(|locale| available_locales.iter().any(|item| item == *locale))
            .cloned()
            .unwrap_or_else(|| default_locale.clone());

        let region = self
            .resolve_region(
                tenant_id,
                &input,
                requested_locale.as_deref(),
                Some(&default_locale),
            )
            .await?;
        let currency_code = match (input.currency_code.as_deref(), region.as_ref()) {
            (Some(currency_code), Some(region)) => {
                let normalized = normalize_currency(currency_code)?;
                if normalized != region.currency_code {
                    return Err(StoreContextError::CurrencyRegionMismatch {
                        currency_code: normalized,
                        region_currency_code: region.currency_code.clone(),
                        region_id: region.id,
                    });
                }
                Some(normalized)
            }
            (Some(currency_code), None) => Some(normalize_currency(currency_code)?),
            (None, Some(region)) => Some(region.currency_code.clone()),
            (None, None) => None,
        };

        Ok(StoreContextResponse {
            region,
            locale,
            default_locale,
            available_locales,
            currency_code,
        })
    }

    async fn resolve_region(
        &self,
        tenant_id: Uuid,
        input: &ResolveStoreContextInput,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> StoreContextResult<Option<RegionResponse>> {
        let selector = if let Some(region_id) = input.region_id {
            RegionReadSelector::Id(region_id)
        } else if let Some(country_code) = input.country_code.as_deref() {
            RegionReadSelector::CountryCode(country_code.to_string())
        } else {
            return Ok(None);
        };
        let locale = requested_locale.or(tenant_default_locale).unwrap_or("und");
        let context = store_context_port_context(tenant_id, locale, "region");
        let projection = self
            .region_read_port
            .read_region(
                context,
                RegionReadRequest {
                    selector,
                    requested_locale: requested_locale.map(str::to_string),
                    tenant_default_locale: tenant_default_locale.map(str::to_string),
                },
            )
            .await
            .map_err(map_region_port_error)?;

        Ok(projection.map(|projection| projection.region))
    }

    async fn load_tenant_locale_context(
        &self,
        tenant_id: Uuid,
    ) -> StoreContextResult<(String, Vec<String>)> {
        let tenant = self
            .tenant_read_port
            .read_tenant(
                store_context_port_context(tenant_id, PLATFORM_FALLBACK_LOCALE, "tenant"),
                TenantReadRequest {
                    selector: TenantReadSelector::Id(tenant_id),
                    include_inactive: false,
                },
            )
            .await
            .map_err(|error| map_tenant_port_error(tenant_id, error))?;
        let policy = self
            .tenant_locale_policy_port
            .read_locale_policy(store_context_port_context(
                tenant_id,
                tenant.default_locale.as_str(),
                "locale-policy",
            ))
            .await
            .map_err(|error| map_tenant_port_error(tenant_id, error))?;
        let default_locale = policy.default_locale.into_inner();
        if tenant.default_locale != default_locale {
            return Err(StoreContextError::TenantBoundary {
                code: "tenant.locale_policy_default_mismatch".to_string(),
                message: "tenant default locale does not match the owner locale policy".to_string(),
            });
        }
        let available_locales = policy
            .locales
            .into_iter()
            .filter(|locale| locale.is_enabled)
            .map(|locale| locale.locale.into_inner())
            .collect();

        Ok((default_locale, available_locales))
    }
}

fn store_context_port_context(tenant_id: Uuid, locale: &str, operation: &str) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service("commerce.store-context"),
        locale,
        format!("store-context:{operation}:{tenant_id}"),
    )
    .with_deadline(STORE_CONTEXT_PORT_TIMEOUT)
}

fn map_tenant_port_error(tenant_id: Uuid, error: PortError) -> StoreContextError {
    if error.kind == PortErrorKind::NotFound {
        StoreContextError::TenantNotFound(tenant_id)
    } else {
        StoreContextError::TenantBoundary {
            code: error.code,
            message: error.message,
        }
    }
}

fn map_region_port_error(error: PortError) -> StoreContextError {
    StoreContextError::RegionBoundary {
        code: error.code,
        message: error.message,
    }
}

fn normalize_locale(value: &str) -> StoreContextResult<String> {
    TenantLocale::new(value)
        .map(TenantLocale::into_inner)
        .map_err(|error| StoreContextError::Validation(error.to_string()))
}

fn normalize_currency(value: &str) -> StoreContextResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 3 {
        Ok(normalized)
    } else {
        Err(StoreContextError::Validation(
            "currency_code must be a 3-letter code".to_string(),
        ))
    }
}
