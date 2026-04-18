use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum RegistryModuleReleaseStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "yanked")]
    Yanked,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "registry_module_releases")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub request_id: Option<String>,
    pub slug: String,
    pub version: String,
    pub crate_name: String,
    pub default_locale: String,
    pub ownership: String,
    pub trust_level: String,
    pub license: String,
    pub entry_type: Option<String>,
    pub marketplace: Json,
    pub ui_packages: Json,
    pub status: RegistryModuleReleaseStatus,
    #[sea_orm(column_name = "publisher_principal")]
    pub publisher: Json,
    pub artifact_storage_key: Option<String>,
    pub checksum_sha256: Option<String>,
    pub artifact_size: Option<i64>,
    pub yanked_reason: Option<String>,
    #[sea_orm(column_name = "yanked_by_principal")]
    pub yanked_by: Option<Json>,
    pub yanked_at: Option<DateTime<Utc>>,
    pub published_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
