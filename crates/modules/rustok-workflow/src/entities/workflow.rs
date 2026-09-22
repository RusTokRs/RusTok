use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "lowercase")]
pub enum WorkflowStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "paused")]
    Paused,
    #[sea_orm(string_value = "archived")]
    Archived,
}

impl std::fmt::Display for WorkflowStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "workflows")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: WorkflowStatus,
    /// Trigger type and params: {"type": "event", "event_type": "blog.post.published"}
    pub trigger_config: Json,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    /// Consecutive failure counter — reset on success, incremented on failure
    pub failure_count: i32,
    /// Set when the workflow is auto-disabled due to exceeding the failure threshold
    pub auto_disabled_at: Option<DateTimeWithTimeZone>,
    /// Unique slug for webhook trigger: POST /webhooks/{tenant_slug}/{webhook_slug}
    pub webhook_slug: Option<String>,
    /// HMAC-SHA256 secret for verifying webhook payloads (X-Webhook-Signature header)
    pub webhook_secret: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::workflow_step::Entity")]
    Steps,
    #[sea_orm(has_many = "super::workflow_execution::Entity")]
    Executions,
    #[sea_orm(has_many = "super::workflow_version::Entity")]
    Versions,
}

impl Related<super::workflow_step::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Steps.def()
    }
}

impl Related<super::workflow_execution::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Executions.def()
    }
}

impl Related<super::workflow_version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Versions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
