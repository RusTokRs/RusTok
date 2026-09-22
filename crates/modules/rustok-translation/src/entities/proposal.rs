use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_proposals")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub item_id: Uuid,
    pub proposal_revision: i64,
    pub origin: String,
    pub values: Json,
    pub values_digest: String,
    pub qa_issues: Json,
    pub created_by_actor_kind: String,
    pub created_by_actor_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub submitted_at: Option<DateTimeWithTimeZone>,
    pub submission_idempotency_key: Option<String>,
    pub submission_request_hash: Option<String>,
    pub approved_by_actor_kind: Option<String>,
    pub approved_by_actor_id: Option<String>,
    pub approved_at: Option<DateTimeWithTimeZone>,
    pub approval_receipt_id: Option<String>,
    pub approval_idempotency_key: Option<String>,
    pub approval_request_hash: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
