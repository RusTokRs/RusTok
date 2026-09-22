use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use rustok_region::dto::RegionResponse;

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ResolveStoreContextInput {
    pub region_id: Option<Uuid>,
    pub country_code: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StoreContextResponse {
    pub region: Option<RegionResponse>,
    pub locale: String,
    pub default_locale: String,
    pub available_locales: Vec<String>,
    pub currency_code: Option<String>,
}
