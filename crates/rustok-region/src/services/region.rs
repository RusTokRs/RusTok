use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use tracing::instrument;
use uuid::Uuid;
use validator::Validate;

use rustok_commerce_foundation::entities;
use rustok_core::generate_id;

use crate::dto::{CreateRegionInput, RegionResponse, UpdateRegionInput};
use crate::error::{RegionError, RegionResult};

pub struct RegionService {
    db: DatabaseConnection,
}

impl RegionService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id))]
    pub async fn create_region(
        &self,
        tenant_id: Uuid,
        input: CreateRegionInput,
    ) -> RegionResult<RegionResponse> {
        input
            .validate()
            .map_err(|error| RegionError::Validation(error.to_string()))?;

        let currency_code = normalize_currency_code(&input.currency_code)?;
        let countries = normalize_countries(input.countries)?;
        let now = Utc::now();
        let region_id = generate_id();

        entities::region::ActiveModel {
            id: Set(region_id),
            tenant_id: Set(tenant_id),
            name: Set(input.name.trim().to_string()),
            currency_code: Set(currency_code),
            tax_rate: Set(input.tax_rate),
            tax_included: Set(input.tax_included),
            countries: Set(serde_json::to_value(&countries)
                .map_err(|error| RegionError::Validation(error.to_string()))?),
            metadata: Set(input.metadata),
            created_at: Set(now.into()),
            updated_at: Set(now.into()),
        }
        .insert(&self.db)
        .await?;

        self.get_region(tenant_id, region_id).await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, region_id = %region_id))]
    pub async fn get_region(
        &self,
        tenant_id: Uuid,
        region_id: Uuid,
    ) -> RegionResult<RegionResponse> {
        let model = entities::region::Entity::find_by_id(region_id)
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(RegionError::RegionNotFound(region_id))?;
        to_response(model)
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn list_regions(&self, tenant_id: Uuid) -> RegionResult<Vec<RegionResponse>> {
        entities::region::Entity::find()
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .order_by_asc(entities::region::Column::Name)
            .all(&self.db)
            .await?
            .into_iter()
            .map(to_response)
            .collect()
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, region_id = %region_id))]
    pub async fn update_region(
        &self,
        tenant_id: Uuid,
        region_id: Uuid,
        input: UpdateRegionInput,
    ) -> RegionResult<RegionResponse> {
        input
            .validate()
            .map_err(|error| RegionError::Validation(error.to_string()))?;

        let existing = entities::region::Entity::find_by_id(region_id)
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(RegionError::RegionNotFound(region_id))?;

        let mut active: entities::region::ActiveModel = existing.into();
        if let Some(name) = input.name {
            active.name = Set(name.trim().to_string());
        }
        if let Some(currency_code) = input.currency_code {
            active.currency_code = Set(normalize_currency_code(&currency_code)?);
        }
        if let Some(tax_rate) = input.tax_rate {
            active.tax_rate = Set(tax_rate);
        }
        if let Some(tax_included) = input.tax_included {
            active.tax_included = Set(tax_included);
        }
        if let Some(countries) = input.countries {
            active.countries = Set(serde_json::to_value(normalize_countries(countries)?)
                .map_err(|error| RegionError::Validation(error.to_string()))?);
        }
        if let Some(metadata) = input.metadata {
            active.metadata = Set(metadata);
        }
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.get_region(tenant_id, region_id).await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, country_code = %country_code))]
    pub async fn resolve_region_for_country(
        &self,
        tenant_id: Uuid,
        country_code: &str,
    ) -> RegionResult<Option<RegionResponse>> {
        let normalized_country = normalize_country_code(country_code)?;
        let regions = self.list_regions(tenant_id).await?;
        Ok(regions.into_iter().find(|region| {
            region
                .countries
                .iter()
                .any(|country| country.eq_ignore_ascii_case(&normalized_country))
        }))
    }
}

fn to_response(model: entities::region::Model) -> RegionResult<RegionResponse> {
    let countries = serde_json::from_value::<Vec<String>>(model.countries)
        .map_err(|error| RegionError::Validation(error.to_string()))?;

    Ok(RegionResponse {
        id: model.id,
        tenant_id: model.tenant_id,
        name: model.name,
        currency_code: model.currency_code,
        tax_rate: model.tax_rate,
        tax_included: model.tax_included,
        countries,
        metadata: model.metadata,
        created_at: model.created_at.with_timezone(&Utc),
        updated_at: model.updated_at.with_timezone(&Utc),
    })
}

fn normalize_currency_code(value: &str) -> RegionResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 3 {
        Ok(normalized)
    } else {
        Err(RegionError::Validation(
            "currency_code must be a 3-letter code".to_string(),
        ))
    }
}

fn normalize_countries(values: Vec<String>) -> RegionResult<Vec<String>> {
    if values.is_empty() {
        return Err(RegionError::Validation(
            "countries must contain at least one country code".to_string(),
        ));
    }

    values
        .into_iter()
        .map(|value| normalize_country_code(&value))
        .collect()
}

fn normalize_country_code(value: &str) -> RegionResult<String> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.len() == 2 && normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        Ok(normalized)
    } else {
        Err(RegionError::InvalidCountryCode(value.to_string()))
    }
}
