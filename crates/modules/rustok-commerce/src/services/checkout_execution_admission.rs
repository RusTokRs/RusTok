use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortError, PortErrorKind};
use rustok_payment::{
    CheckoutExecutionAdmissionPort, CheckoutExecutionAdmissionRecord, ProviderExecutionAdmission,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::checkout_operation;
use crate::services::checkout_operation::CheckoutExecutionAdmission;

/// Commerce implementation of the payment-side checkout admission port.
///
/// The checkout journal owns the level and its generation; the payment claim
/// gate reads them through this port instead of keeping a derived copy that
/// could drift. The read is tenant-scoped and typed: a row with a level outside
/// the vocabulary is an invariant violation, not an admission the gate may
/// guess about.
pub struct CheckoutExecutionAdmissionReader {
    db: DatabaseConnection,
}

impl CheckoutExecutionAdmissionReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

/// Builds the admission reader the payment claim gate fences on.
pub fn checkout_execution_admission_port(
    db: DatabaseConnection,
) -> Arc<dyn CheckoutExecutionAdmissionPort> {
    Arc::new(CheckoutExecutionAdmissionReader::new(db))
}

#[async_trait]
impl CheckoutExecutionAdmissionPort for CheckoutExecutionAdmissionReader {
    async fn read_checkout_execution_admission(
        &self,
        tenant_id: Uuid,
        checkout_operation_id: Uuid,
    ) -> Result<Option<CheckoutExecutionAdmissionRecord>, PortError> {
        if tenant_id.is_nil() || checkout_operation_id.is_nil() {
            return Err(PortError::validation(
                "commerce.checkout_admission_identity_invalid",
                "checkout admission read requires non-nil tenant and operation identifiers",
            ));
        }
        let row = checkout_operation::Entity::find_by_id(checkout_operation_id)
            .filter(checkout_operation::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await
            .map_err(|_| {
                PortError::new(
                    PortErrorKind::Unavailable,
                    "commerce.checkout_admission_unavailable",
                    "checkout admission is temporarily unavailable",
                    true,
                )
            })?;
        let Some(row) = row else {
            // The payment collection points at an operation this tenant cannot
            // see: the claim gate fails closed on extending effects.
            return Ok(None);
        };
        let admission = CheckoutExecutionAdmission::parse(row.execution_admission.as_str())
            .ok_or_else(|| {
                PortError::invariant_violation(
                    "commerce.checkout_admission_unknown_level",
                    "checkout operation carries an admission level outside the bounded vocabulary",
                )
            })?;
        Ok(Some(CheckoutExecutionAdmissionRecord {
            admission: admission.into_payment_admission(),
            admission_epoch: row.admission_epoch,
        }))
    }
}

impl CheckoutExecutionAdmission {
    /// Maps the journal's level onto the payment-side vocabulary.
    ///
    /// Both enums are bounded by the same `rustok-events` labels, so the claim
    /// gate and the journal can never disagree about what `open`, `settling` or
    /// `closed` mean.
    pub const fn into_payment_admission(self) -> ProviderExecutionAdmission {
        match self {
            Self::Open => ProviderExecutionAdmission::Open,
            Self::Settling => ProviderExecutionAdmission::Settling,
            Self::Closed => ProviderExecutionAdmission::Closed,
        }
    }
}
