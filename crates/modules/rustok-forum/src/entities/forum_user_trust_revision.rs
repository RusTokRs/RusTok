use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ForumUserTrustChangeKind {
    #[sea_orm(string_value = "manual_override")]
    ManualOverride,
    #[sea_orm(string_value = "policy_evaluation")]
    PolicyEvaluation,
    #[sea_orm(string_value = "reconciliation")]
    Reconciliation,
    #[sea_orm(string_value = "migration")]
    Migration,
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_user_trust_revisions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub revision: i64,
    pub previous_trust_level: Option<i16>,
    pub trust_level: i16,
    pub change_kind: ForumUserTrustChangeKind,
    pub reason_code: String,
    pub reason_summary: String,
    pub changed_by_user_id: Option<Uuid>,
    pub idempotency_key: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
