use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use thiserror::Error;
use tracing::instrument;
use uuid::Uuid;

use rustok_region::dto::RegionResponse;
use rustok_region::{RegionError, RegionService};

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
    #[error(transparent)]
    Region(#[from] RegionError),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

pub struct StoreContextService {
    db: DatabaseConnection,
    region_service: RegionService,
}

impl StoreContextService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            region_service: RegionService::new(db.clone()),
            db,
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
            .filter(|locale| available_locales.iter().any(|item| item == locale))
            .unwrap_or_else(|| default_locale.clone());

        let region = self.resolve_region(tenant_id, &input).await?;
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
    ) -> StoreContextResult<Option<RegionResponse>> {
        if let Some(region_id) = input.region_id {
            return Ok(Some(
                self.region_service.get_region(tenant_id, region_id).await?,
            ));
        }

        if let Some(country_code) = input.country_code.as_deref() {
            return self
                .region_service
                .resolve_region_for_country(tenant_id, country_code)
                .await
                .map_err(StoreContextError::from);
        }

        Ok(None)
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
        let default_locale = row
            .try_get::<String>("", "default_locale")
            .map_err(sea_orm::DbErr::from)?;
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
            let locale = row
                .try_get::<String>("", "locale")
                .map_err(sea_orm::DbErr::from)?;
            let normalized = normalize_locale(&locale)?;
            if !locales.contains(&normalized) {
                locales.push(normalized);
            }
        }

        Ok(locales)
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
