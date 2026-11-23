use sea_orm::entity::prelude::*;

pub const PENDING: &str = "pending";
pub const RUNNING: &str = "running";
pub const COMPLETED: &str = "completed";
pub const SKIPPED: &str = "skipped";
pub const RECONCILIATION_REQUIRED: &str = "reconciliation_required";

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "workflow_cron_firings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub workflow_id: Uuid,
    pub tenant_id: Uuid,
    pub schedule_revision: String,
    pub scheduled_at: DateTimeWithTimeZone,
    pub status: String,
    pub attempt_token: Option<Uuid>,
    pub claimed_at: Option<DateTimeWithTimeZone>,
    pub execution_id: Option<Uuid>,
    pub reason: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
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
