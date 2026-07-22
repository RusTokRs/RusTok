use std::sync::Arc;

use async_trait::async_trait;

use crate::providers::PaymentProviderWebhookResult;

use super::{
    PaymentDomainEventApplier, PaymentProviderEventApplier, PaymentProviderEventApplyError,
    PaymentProviderEventContext,
};

#[async_trait]
pub trait PaymentProviderProcessedEventObserver: Send + Sync {
    async fn observe(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError>;
}

#[derive(Clone, Default)]
pub struct PaymentProviderEventObservers {
    observers: Arc<Vec<Arc<dyn PaymentProviderProcessedEventObserver>>>,
}

impl PaymentProviderEventObservers {
    pub fn new(observers: Vec<Arc<dyn PaymentProviderProcessedEventObserver>>) -> Self {
        Self {
            observers: Arc::new(observers),
        }
    }

    pub fn with_observer(
        mut self,
        observer: Arc<dyn PaymentProviderProcessedEventObserver>,
    ) -> Self {
        Arc::make_mut(&mut self.observers).push(observer);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.observers.is_empty()
    }

    pub async fn observe(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        for observer in self.observers.iter() {
            observer.observe(context.clone(), event.clone()).await?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct PaymentObservedDomainEventApplier {
    domain: PaymentDomainEventApplier,
    observers: PaymentProviderEventObservers,
}

impl PaymentObservedDomainEventApplier {
    pub fn new(
        db: sea_orm::DatabaseConnection,
        observers: PaymentProviderEventObservers,
    ) -> Self {
        Self {
            domain: PaymentDomainEventApplier::new(db),
            observers,
        }
    }
}

#[async_trait]
impl PaymentProviderEventApplier for PaymentObservedDomainEventApplier {
    async fn apply(
        &self,
        context: PaymentProviderEventContext,
        event: PaymentProviderWebhookResult,
    ) -> Result<(), PaymentProviderEventApplyError> {
        self.domain.apply(context.clone(), event.clone()).await?;
        self.observers.observe(context, event).await
    }
}
