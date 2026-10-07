use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Append-only journal of operator decisions taken on parked checkout
/// operations (`checkout_operations.status = 'reconciliation_required'`).
///
/// Rows are never updated or deleted: the journal answers "who decided what,
/// based on what evidence, and what did it do to the money", and the current
/// state of the operation lives on `checkout_operations`. There is deliberately
/// no `updated_at` column.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "checkout_reconciliation_actions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub checkout_operation_id: Uuid,
    pub cart_id: Uuid,
    /// Action taken, see `CheckoutReconciliationAction`.
    pub action: String,
    /// Status the operation was moved to by this action.
    pub result_status: String,
    /// Refunded amount for `refund_full` / `refund_partial`; `None` when the
    /// action moved no money.
    pub amount: Option<Decimal>,
    pub currency_code: Option<String>,
    /// Operator justification, bounded by the service before insert.
    pub reason: String,
    /// External reference for out-of-band reconciliation (ticket, bank
    /// statement, provider dashboard id). Mandatory for `attest_external`.
    pub evidence_ref: Option<String>,
    pub operator_id: Uuid,
    /// Second approver for `write_off` decisions; must differ from the operator.
    pub approver_id: Option<Uuid>,
    pub idempotency_key: String,
    /// Fingerprint of the request that produced this row; a replay of the same
    /// key with a different fingerprint is rejected as a conflict.
    pub request_hash: String,
    /// Refund created through the payment owner, when the action issued one.
    pub refund_id: Option<Uuid>,
    pub refund_status: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
