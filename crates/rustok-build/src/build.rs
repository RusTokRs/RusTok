//! Build persistence model.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum BuildStatus {
    #[sea_orm(string_value = "queued")]
    Queued,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "success")]
    Success,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}
impl BuildStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Success | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum BuildStage {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "checkout")]
    Checkout,
    #[sea_orm(string_value = "build")]
    Build,
    #[sea_orm(string_value = "test")]
    Test,
    #[sea_orm(string_value = "deploy")]
    Deploy,
    #[sea_orm(string_value = "complete")]
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum DeploymentProfile {
    #[sea_orm(string_value = "monolith")]
    Monolith,
    #[sea_orm(string_value = "server-with-admin")]
    ServerWithAdmin,
    #[sea_orm(string_value = "server-with-storefront")]
    ServerWithStorefront,
    #[sea_orm(string_value = "headless-api")]
    HeadlessApi,
    #[sea_orm(string_value = "worker")]
    Worker,
    #[sea_orm(string_value = "registry")]
    Registry,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "builds")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub status: BuildStatus,
    pub stage: BuildStage,
    pub progress: i32,
    pub profile: DeploymentProfile,
    pub manifest_ref: String,
    pub manifest_hash: String,
    pub manifest_revision: i64,
    pub manifest_snapshot: Json,
    pub modules_delta: Option<Json>,
    pub requested_by: String,
    pub reason: Option<String>,
    pub release_id: Option<String>,
    pub logs_url: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
impl Model {
    pub fn new(
        manifest_ref: String,
        manifest_hash: String,
        manifest_revision: i64,
        manifest_snapshot: Json,
        requested_by: String,
        profile: DeploymentProfile,
    ) -> Self {
        Self {
            id: rustok_core::generate_id(),
            status: BuildStatus::Queued,
            stage: BuildStage::Pending,
            progress: 0,
            profile,
            manifest_ref,
            manifest_hash,
            manifest_revision,
            manifest_snapshot,
            modules_delta: None,
            requested_by,
            reason: None,
            release_id: None,
            logs_url: None,
            error_message: None,
            started_at: None,
            finished_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
    pub fn is_final(&self) -> bool {
        self.status.is_final()
    }
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => Some(end - start),
            _ => None,
        }
    }
}
