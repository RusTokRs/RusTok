use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum MarketplacePayoutOperationStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "executing")]
    Executing,
    #[sea_orm(string_value = "retryable_error")]
    RetryableError,
    #[sea_orm(string_value = "compensation_required")]
    CompensationRequired,
    #[sea_orm(string_value = "compensating")]
    Compensating,
    #[sea_orm(string_value = "reconciliation_required")]
    ReconciliationRequired,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
    #[sea_orm(string_value = "failed")]
    Failed,
}

impl MarketplacePayoutOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executing => "executing",
            Self::RetryableError => "retryable_error",
            Self::CompensationRequired => "compensation_required",
            Self::Compensating => "compensating",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum MarketplacePayoutOperationStage {
    #[sea_orm(string_value = "created")]
    Created,
    #[sea_orm(string_value = "reserving")]
    Reserving,
    #[sea_orm(string_value = "reserved")]
    Reserved,
    #[sea_orm(string_value = "payout_created")]
    PayoutCreated,
    #[sea_orm(string_value = "releasing")]
    Releasing,
    #[sea_orm(string_value = "released")]
    Released,
    #[sea_orm(string_value = "completed")]
    Completed,
}

impl MarketplacePayoutOperationStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Reserving => "reserving",
            Self::Reserved => "reserved",
            Self::PayoutCreated => "payout_created",
            Self::Releasing => "releasing",
            Self::Released => "released",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_payout_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub seller_id: Uuid,
    pub currency_code: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub request_json: Json,
    pub status: MarketplacePayoutOperationStatus,
    pub stage: MarketplacePayoutOperationStage,
    pub payout_id: Option<Uuid>,
    pub attempt_count: i32,
    pub revision: i64,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTimeWithTimeZone>,
    pub last_error_code: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
