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
    #[sea_orm(string_value = "changes_requested")]
    ChangesRequested,
    #[sea_orm(string_value = "on_hold")]
    OnHold,
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
    pub default_locale: String,
    pub ownership: String,
    pub trust_level: String,
    pub license: String,
    pub entry_type: Option<String>,
    pub marketplace: Json,
    pub ui_packages: Json,
    pub status: RegistryPublishRequestStatus,
    #[sea_orm(column_name = "requested_by_principal")]
    pub requested_by: Json,
    #[sea_orm(column_name = "publisher_principal")]
    pub publisher_principal: Option<Json>,
    #[sea_orm(column_name = "approved_by_principal")]
    pub approved_by: Option<Json>,
    #[sea_orm(column_name = "rejected_by_principal")]
    pub rejected_by: Option<Json>,
    pub rejection_reason: Option<String>,
    #[sea_orm(column_name = "changes_requested_by_principal")]
    pub changes_requested_by: Option<Json>,
    pub changes_requested_reason: Option<String>,
    pub changes_requested_reason_code: Option<String>,
    pub changes_requested_at: Option<DateTime<Utc>>,
    #[sea_orm(column_name = "held_by_principal")]
    pub held_by: Option<Json>,
    pub held_reason: Option<String>,
    pub held_reason_code: Option<String>,
    pub held_at: Option<DateTime<Utc>>,
    pub held_from_status: Option<String>,
    pub validation_warnings: Json,
    pub validation_errors: Json,
    pub artifact_storage_key: Option<String>,
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
