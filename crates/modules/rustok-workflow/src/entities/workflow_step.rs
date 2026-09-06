use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    #[sea_orm(string_value = "action")]
    Action,
    #[sea_orm(string_value = "condition")]
    Condition,
    #[sea_orm(string_value = "delay")]
    Delay,
    #[sea_orm(string_value = "alloy_script")]
    AlloyScript,
    #[sea_orm(string_value = "emit_event")]
    EmitEvent,
    #[sea_orm(string_value = "http")]
    Http,
    #[sea_orm(string_value = "notify")]
    Notify,
    #[sea_orm(string_value = "transform")]
    Transform,
}

impl std::fmt::Display for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Action => write!(f, "action"),
            Self::Condition => write!(f, "condition"),
            Self::Delay => write!(f, "delay"),
            Self::AlloyScript => write!(f, "alloy_script"),
            Self::EmitEvent => write!(f, "emit_event"),
            Self::Http => write!(f, "http"),
            Self::Notify => write!(f, "notify"),
            Self::Transform => write!(f, "transform"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    #[sea_orm(string_value = "stop")]
    Stop,
    #[sea_orm(string_value = "skip")]
    Skip,
    #[sea_orm(string_value = "retry")]
    Retry,
}

impl std::fmt::Display for OnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stop => write!(f, "stop"),
            Self::Skip => write!(f, "skip"),
            Self::Retry => write!(f, "retry"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "workflow_steps")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub position: i32,
    pub step_type: StepType,
    /// Step-specific configuration as JSONB
    pub config: Json,
    pub on_error: OnError,
    pub timeout_ms: Option<i64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::workflow::Entity",
        from = "Column::WorkflowId",
        to = "super::workflow::Column::Id"
    )]
    Workflow,
    #[sea_orm(has_many = "super::workflow_step_execution::Entity")]
    StepExecutions,
}

impl Related<super::workflow::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Workflow.def()
    }
}

impl Related<super::workflow_step_execution::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::StepExecutions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
