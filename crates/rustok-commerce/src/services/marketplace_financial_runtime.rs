use std::sync::Arc;

use rustok_marketplace::{MarketplaceFinancialCommandPort, MarketplaceFinancialOrchestrationService};
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::PaymentProviderEventObservers;
use sea_orm::DatabaseConnection;

use super::{
    MarketplaceFinancialOperatorService, MarketplacePaidEventInboxService,
    MarketplaceProviderReversalEventAdapter, MarketplaceReversalEventInboxService,
};

/// Host-composed marketplace financial capability shared by HTTP, GraphQL,
/// event listeners, provider-event adapters, and background recovery workers.
#[derive(Clone)]
pub struct MarketplaceFinancialRuntime {
    ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    financial_port: Option<Arc<dyn MarketplaceFinancialCommandPort>>,
}

impl MarketplaceFinancialRuntime {
    pub fn new(ledger_port: Arc<dyn MarketplaceLedgerCommandPort>) -> Self {
        Self {
            ledger_port,
            financial_port: None,
        }
    }

    pub fn with_financial_port(
        mut self,
        financial_port: Arc<dyn MarketplaceFinancialCommandPort>,
    ) -> Self {
        self.financial_port = Some(financial_port);
        self
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        let allocation = Arc::new(
            rustok_marketplace_allocation::MarketplaceAllocationService::new(db.clone()),
        );
        let commission = Arc::new(
            rustok_marketplace_commission::MarketplaceCommissionService::new(
                db.clone(),
                allocation,
            ),
        );
        let ledger = Arc::new(rustok_marketplace_ledger::MarketplaceLedgerService::new(
            db,
            commission.clone(),
        ));
        let financial = Arc::new(MarketplaceFinancialOrchestrationService::new(
            commission,
            ledger.clone(),
        ));
        Self::new(ledger).with_financial_port(financial)
    }

    pub fn ledger_port(&self) -> Arc<dyn MarketplaceLedgerCommandPort> {
        self.ledger_port.clone()
    }

    pub fn financial_port(&self) -> Arc<dyn MarketplaceFinancialCommandPort> {
        self.financial_port
            .clone()
            .expect("MarketplaceFinancialCommandPort must be host-composed for reversal workflows")
    }

    pub fn paid_event_inbox(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> MarketplacePaidEventInboxService {
        MarketplacePaidEventInboxService::new(db, event_bus, self.ledger_port())
    }

    pub fn reversal_event_inbox(
        &self,
        db: DatabaseConnection,
    ) -> MarketplaceReversalEventInboxService {
        MarketplaceReversalEventInboxService::new(db, self.financial_port())
    }

    pub fn provider_reversal_event_adapter(
        &self,
        db: DatabaseConnection,
    ) -> MarketplaceProviderReversalEventAdapter {
        MarketplaceProviderReversalEventAdapter::new(db, self.financial_port())
    }

    pub fn payment_provider_event_observers(
        &self,
        db: DatabaseConnection,
    ) -> PaymentProviderEventObservers {
        PaymentProviderEventObservers::default().with_observer(Arc::new(
            self.provider_reversal_event_adapter(db),
        ))
    }

    pub fn operator_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> MarketplaceFinancialOperatorService {
        MarketplaceFinancialOperatorService::new(db, event_bus, self.ledger_port())
    }
}
