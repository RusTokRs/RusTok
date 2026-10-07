use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "payment_provider_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub payment_collection_id: Uuid,
    pub refund_id: Option<Uuid>,
    pub operation: String,
    pub provider_id: String,
    pub idempotency_key: String,
    pub status: String,
    pub request_payload: Json,
    pub provider_reference: Option<String>,
    pub provider_result: Option<Json>,
    pub error_message: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub provider_completed_at: Option<DateTimeWithTimeZone>,
    pub committed_at: Option<DateTimeWithTimeZone>,
    /// Checkout admission generation the claim gate observed on this operation.
    ///
    /// `0` marks a row created before the admission contract; such a row is
    /// adopted into the generation the checkout owner reports, but only while
    /// the projected level is `open`.
    pub admission_epoch: i64,
    /// Bounded reason of the last refused claim (see `CheckoutAdmissionRefusal`).
    pub admission_refusal_code: Option<String>,
    /// When the last refused claim was recorded.
    pub admission_refused_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
