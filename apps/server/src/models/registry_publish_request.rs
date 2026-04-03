use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum RegistryPublishRequestStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "artifact_uploaded")]
    ArtifactUploaded,
    #[sea_orm(string_value = "submitted")]
    Submitted,
    #[sea_orm(string_value = "validating")]
    Validating,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
    #[sea_orm(string_value = "published")]
    Published,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "registry_publish_requests")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
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
    pub status: RegistryPublishRequestStatus,
    pub requested_by: String,
    pub approved_by: Option<String>,
    pub rejected_by: Option<String>,
    pub rejection_reason: Option<String>,
    pub validation_warnings: Json,
    pub validation_errors: Json,
    pub artifact_path: Option<String>,
    pub artifact_url: Option<String>,
    pub artifact_checksum_sha256: Option<String>,
    pub artifact_size: Option<i64>,
    pub artifact_content_type: Option<String>,
    pub submitted_at: Option<DateTime<Utc>>,
    pub validated_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
