use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_exchange_jobs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub job_id: Uuid,
    pub direction: String,
    pub status: String,
    pub object_key: String,
    pub content_length: i64,
    pub checksum_sha256: String,
    pub created_by_actor_kind: String,
    pub created_by_actor_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub processing_idempotency_key: Option<String>,
    pub processing_request_hash: Option<String>,
    pub processed_by_actor_kind: Option<String>,
    pub processed_by_actor_id: Option<String>,
    pub processing_lease_token: Option<Uuid>,
    pub processing_lease_expires_at: Option<DateTimeWithTimeZone>,
    pub processed_at: Option<DateTimeWithTimeZone>,
    pub report: Json,
    pub expires_at: DateTimeWithTimeZone,
    pub storage_deleted_at: Option<DateTimeWithTimeZone>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
