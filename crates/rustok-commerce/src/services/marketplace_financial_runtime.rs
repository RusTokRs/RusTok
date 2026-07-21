use std::sync::Arc;

use rustok_marketplace::{
    MarketplaceFinancialCommandPort, MarketplaceFinancialOrchestrationService,
};
use rustok_marketplace_allocation::MarketplaceAllocationReadPort;
use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_outbox::TransactionalEventBus;
use rustok_payment::PaymentProviderEventObservers;
use sea_orm::DatabaseConnection;

#[path = "marketplace_reversal_fact_guard.rs"]
mod marketplace_reversal_fact_guard;

use self::marketplace_reversal_fact_guard::MarketplaceReversalFactGuardObserver;
use super::{
    MarketplaceFinancialOperatorService, MarketplacePaidEventInboxService,
    MarketplaceProviderReversalBackfillService, MarketplaceProviderReversalEventAdapter,
    MarketplaceReversalEventInboxService, MarketplaceReversalOperatorService,
};

/// Host-composed marketplace financial capability shared by HTTP, GraphQL,
/// event listeners, provider-event adapters, and background recovery workers.
#[derive(Clone)]
pub struct MarketplaceFinancialRuntime {
    ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
    allocation_reader: Option<Arc<dyn MarketplaceAllocationReadPort>>,
    financial_port: Option<Arc<dyn MarketplaceFinancialCommandPort>>,
}

impl MarketplaceFinancialRuntime {
    pub fn new(ledger_port: Arc<dyn MarketplaceLedgerCommandPort>) -> Self {
        Self {
            ledger_port,
            allocation_reader: None,
            financial_port: None,
        }
    }

    pub fn with_allocation_reader(
        mut self,
        allocation_reader: Arc<dyn MarketplaceAllocationReadPort>,
    ) -> Self {
        self.allocation_reader = Some(allocation_reader);
        self
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
                allocation.clone(),
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
        Self::new(ledger)
            .with_allocation_reader(allocation)
            .with_financial_port(financial)
    }

    pub fn ledger_port(&self) -> Arc<dyn MarketplaceLedgerCommandPort> {
        self.ledger_port.clone()
    }

    pub fn allocation_reader(&self) -> Arc<dyn MarketplaceAllocationReadPort> {
        self.allocation_reader.clone().expect(
            "MarketplaceAllocationReadPort must be host-composed for reversal fact guards",
        )
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

    pub fn provider_reversal_backfill(
        &self,
        db: DatabaseConnection,
    ) -> MarketplaceProviderReversalBackfillService {
        MarketplaceProviderReversalBackfillService::new(db, self.financial_port())
    }

    pub fn payment_provider_event_observers(
        &self,
        db: DatabaseConnection,
    ) -> PaymentProviderEventObservers {
        let delegate = self.provider_reversal_event_adapter(db.clone());
        let guarded = MarketplaceReversalFactGuardObserver::new(
            db,
            self.allocation_reader(),
            delegate,
        );
        PaymentProviderEventObservers::default().with_observer(Arc::new(guarded))
    }

    pub fn operator_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> MarketplaceFinancialOperatorService {
        MarketplaceFinancialOperatorService::new(db, event_bus, self.ledger_port())
    }

    pub fn reversal_operator_service(
        &self,
        db: DatabaseConnection,
    ) -> MarketplaceReversalOperatorService {
        MarketplaceReversalOperatorService::new(db, self.financial_port())
    }
}
