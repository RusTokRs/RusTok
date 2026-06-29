use std::{sync::Arc, time::Duration};

use rustok_api::{PortActor, PortContext, PortError};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

use rustok_region::dto::RegionResponse;
use rustok_region::{RegionReadPort, RegionReadRequest, RegionReadSelector};

use crate::dto::{ResolveStoreContextInput, StoreContextResponse};

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
    #[error("region boundary `{code}` failed: {message}")]
    RegionBoundary { code: String, message: String },
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub struct StoreContextService {
    db: DatabaseConnection,
    region_read_port: Arc<dyn RegionReadPort>,
}

impl StoreContextService {
    pub fn new(db: DatabaseConnection, region_read_port: Arc<dyn RegionReadPort>) -> Self {
        Self {
            db,
            region_read_port,
        }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn resolve_context(
        &self,
        tenant_id: Uuid,
        input: ResolveStoreContextInput,
    ) -> StoreContextResult<StoreContextResponse> {
        let default_locale = self.load_default_locale(tenant_id).await?;
        let mut available_locales = self.load_enabled_locales(tenant_id).await?;
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
        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::service("commerce.store-context"),
            locale,
            format!("store-context:{tenant_id}"),
        )
        .with_deadline(Duration::from_secs(3));
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

    async fn load_default_locale(&self, tenant_id: Uuid) -> StoreContextResult<String> {
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                "SELECT default_locale FROM tenants WHERE id = ?",
                vec![tenant_id.into()],
            ))
            .await?;

        let row = row.ok_or(StoreContextError::TenantNotFound(tenant_id))?;
        let default_locale = row.try_get::<String>("", "default_locale")?;
        normalize_locale(&default_locale)
    }

    async fn load_enabled_locales(&self, tenant_id: Uuid) -> StoreContextResult<Vec<String>> {
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                "SELECT locale FROM tenant_locales WHERE tenant_id = ? AND is_enabled = TRUE ORDER BY is_default DESC, locale ASC",
                vec![tenant_id.into()],
            ))
            .await?;

        let mut locales = Vec::new();
        for row in rows {
            let locale = row.try_get::<String>("", "locale")?;
            let normalized = normalize_locale(&locale)?;
            if !locales.contains(&normalized) {
                locales.push(normalized);
            }
        }

        Ok(locales)
    }
}

fn map_region_port_error(error: PortError) -> StoreContextError {
    StoreContextError::RegionBoundary {
        code: error.code,
        message: error.message,
    }
}

fn normalize_locale(value: &str) -> StoreContextResult<String> {
    let normalized = value.trim().replace('_', "-").to_ascii_lowercase();
    if (2..=10).contains(&normalized.len()) {
        Ok(normalized)
    } else {
        Err(StoreContextError::Validation(format!(
            "locale `{value}` is invalid"
        )))
    }
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
