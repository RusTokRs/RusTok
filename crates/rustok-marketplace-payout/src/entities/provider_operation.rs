use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum MarketplacePayoutProviderOperationKind {
    #[sea_orm(string_value = "submit")]
    Submit,
    #[sea_orm(string_value = "lookup")]
    Lookup,
    #[sea_orm(string_value = "cancel")]
    Cancel,
}

impl MarketplacePayoutProviderOperationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::Lookup => "lookup",
            Self::Cancel => "cancel",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum MarketplacePayoutProviderOperationStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "executing")]
    Executing,
    #[sea_orm(string_value = "provider_succeeded")]
    ProviderSucceeded,
    #[sea_orm(string_value = "provider_failed")]
    ProviderFailed,
    #[sea_orm(string_value = "retryable_error")]
    RetryableError,
    #[sea_orm(string_value = "reconciliation_required")]
    ReconciliationRequired,
    #[sea_orm(string_value = "committed")]
    Committed,
}

impl MarketplacePayoutProviderOperationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Executing => "executing",
            Self::ProviderSucceeded => "provider_succeeded",
            Self::ProviderFailed => "provider_failed",
            Self::RetryableError => "retryable_error",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::Committed => "committed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "marketplace_payout_provider_operations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub payout_id: Uuid,
    pub operation: MarketplacePayoutProviderOperationKind,
    pub provider_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub request_json: Json,
    pub status: MarketplacePayoutProviderOperationStatus,
    pub provider_reference: Option<String>,
    pub provider_result_json: Option<Json>,
    pub attempt_count: i32,
    pub revision: i64,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTimeWithTimeZone>,
    pub last_error_code: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub provider_completed_at: Option<DateTimeWithTimeZone>,
    pub committed_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
