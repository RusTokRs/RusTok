use async_trait::async_trait;
use std::sync::Arc;

use crate::providers::PaymentProviderWebhookResult;

use super::{
    PaymentLifecycleEventApplier, PaymentProviderEventApplier, PaymentProviderEventApplyError,
    PaymentProviderEventContext, RefundLifecycleEventApplier,
};

#[derive(Clone)]
pub struct PaymentDomainEventApplier {
    payment: Arc<PaymentLifecycleEventApplier>,
    refund: Arc<RefundLifecycleEventApplier>,
}

impl PaymentDomainEventApplier {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            payment: Arc::new(PaymentLifecycleEventApplier::new(db.clone())),
            refund: Arc::new(RefundLifecycleEventApplier::new(db)),
        }
    }
}

#[async_trait]
impl PaymentProviderEventApplier for PaymentDomainEventApplier {
    async fn apply(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        if event.event_type.starts_with("payment.") {
            self.payment.apply(context, event).await
        } else if event.event_type.starts_with("refund.") {
            self.refund.apply(context, event).await
        } else {
            Err(PaymentProviderEventApplyError::new(
                "payment.webhook_event_unsupported",
                format!("unsupported normalized provider event `{}`", event.event_type),
                false,
            ))
        }
    }
}
