use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "workflow_cron_reconciliation_decisions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub firing_id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub decision: String,
    pub evidence: String,
    pub previous_attempt_token: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::workflow_cron_firing::Entity",
        from = "Column::FiringId",
        to = "super::workflow_cron_firing::Column::Id"
    )]
    Firing,
}

impl Related<super::workflow_cron_firing::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Firing.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
