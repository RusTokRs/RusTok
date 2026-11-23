use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "workflow_cron_cursors")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub workflow_id: Uuid,
    pub tenant_id: Uuid,
    /// Digest of trigger expression plus workflow update version.
    pub schedule_revision: String,
    pub last_enqueued_at: DateTimeWithTimeZone,
    /// Compare-and-set generation for competing scheduler replicas.
    pub generation: i64,
    pub updated_at: DateTimeWithTimeZone,
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
