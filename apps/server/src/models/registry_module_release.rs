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
    pub module_name: String,
    pub description: String,
    pub ownership: String,
    pub trust_level: String,
    pub license: String,
    pub entry_type: Option<String>,
    pub marketplace: Json,
    pub ui_packages: Json,
    pub status: RegistryModuleReleaseStatus,
    pub publisher: String,
    pub artifact_path: Option<String>,
    pub artifact_url: Option<String>,
    pub checksum_sha256: Option<String>,
    pub artifact_size: Option<i64>,
    pub yanked_reason: Option<String>,
    pub yanked_by: Option<String>,
    pub yanked_at: Option<DateTime<Utc>>,
    pub published_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
