use rustok_commerce_foundation::entities;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use tracing::instrument;
use uuid::Uuid;

use crate::domain::region::normalize_country_code;
use crate::dto::RegionResponse;
use crate::error::{RegionError, RegionResult};

use super::RegionService;
use super::repository::{load_region, load_regions_with_translations};

impl RegionService {
    #[instrument(skip(self), fields(tenant_id = %tenant_id, region_id = %region_id))]
    pub async fn get_region(
        &self,
        tenant_id: Uuid,
        region_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> RegionResult<RegionResponse> {
        if tenant_id.is_nil() {
            return Err(RegionError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        if region_id.is_nil() {
            return Err(RegionError::Validation(
                "region_id must not be nil".to_string(),
            ));
        }

        load_region(
            &self.db,
            tenant_id,
            region_id,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id))]
    pub async fn list_regions(
        &self,
        tenant_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> RegionResult<Vec<RegionResponse>> {
        if tenant_id.is_nil() {
            return Err(RegionError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }

        let rows = entities::region::Entity::find()
            .filter(entities::region::Column::TenantId.eq(tenant_id))
            .order_by_asc(entities::region::Column::CreatedAt)
            .all(&self.db)
            .await?;

        load_regions_with_translations(&self.db, rows, requested_locale, tenant_default_locale)
            .await
    }

    #[instrument(skip(self), fields(tenant_id = %tenant_id, country_code = %country_code))]
    pub async fn resolve_region_for_country(
        &self,
        tenant_id: Uuid,
        country_code: &str,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> RegionResult<Option<RegionResponse>> {
        if tenant_id.is_nil() {
            return Err(RegionError::Validation(
                "tenant_id must not be nil".to_string(),
            ));
        }
        let normalized_country = normalize_country_code(country_code)?;
        let regions = self
            .list_regions(tenant_id, requested_locale, tenant_default_locale)
            .await?;
        Ok(regions.into_iter().find(|region| {
            region
                .countries
                .iter()
                .any(|country| country.eq_ignore_ascii_case(&normalized_country))
        }))
    }
}
