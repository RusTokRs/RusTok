use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum MarketplacePayoutOperationTransferKind {
    #[sea_orm(string_value = "reserve_hold")]
    ReserveHold,
    #[sea_orm(string_value = "reserve_release")]
    ReserveRelease,
}

impl MarketplacePayoutOperationTransferKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReserveHold => "reserve_hold",
            Self::ReserveRelease => "reserve_release",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum MarketplacePayoutOperationTransferStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "executing")]
    Executing,
    #[sea_orm(string_value = "posted")]
    Posted,
    #[sea_orm(string_value = "retryable_error")]
    RetryableError,
    #[sea_orm(string_value = "reconciliation_required")]
    ReconciliationRequired,
    #[sea_orm(string_value = "compensated")]
    Compensated,
    #[sea_orm(string_value = "failed")]
    Failed,
}

impl MarketplacePayoutOperationTransferStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executing => "executing",
            Self::Posted => "posted",
            Self::RetryableError => "retryable_error",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::Compensated => "compensated",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_payout_operation_transfers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub operation_id: Uuid,
    pub sequence_no: i32,
    pub order_id: Uuid,
    pub transfer_kind: MarketplacePayoutOperationTransferKind,
    pub status: MarketplacePayoutOperationTransferStatus,
    pub idempotency_key: String,
    pub request_hash: String,
    pub request_json: Json,
    pub total_amount: i64,
    pub ledger_transfer_id: Option<Uuid>,
    pub ledger_transaction_id: Option<Uuid>,
    pub attempt_count: i32,
    pub revision: i64,
    pub last_error_code: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub completed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
