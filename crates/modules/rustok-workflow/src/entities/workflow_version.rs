use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "workflow_versions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub version: i32,
    /// Full workflow snapshot: name, description, trigger_config + all steps
    pub snapshot: Json,
    pub created_by: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::workflow::Entity",
        from = "Column::WorkflowId",
        to = "super::workflow::Column::Id"
    )]
    Workflow,
}

impl Related<super::workflow::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Workflow.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
