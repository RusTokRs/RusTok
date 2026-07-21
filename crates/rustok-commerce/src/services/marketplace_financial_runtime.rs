use std::sync::Arc;

use rustok_marketplace_ledger::MarketplaceLedgerCommandPort;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use super::{MarketplaceFinancialOperatorService, MarketplacePaidEventInboxService};

/// Host-composed marketplace financial capability shared by HTTP, GraphQL,
/// event listeners, and background recovery workers.
///
/// The runtime owns only the typed ledger command port. Request-scoped
/// orchestration services receive their database and transactional event bus
/// from the host so transports never construct owner services independently.
#[derive(Clone)]
pub struct MarketplaceFinancialRuntime {
    ledger_port: Arc<dyn MarketplaceLedgerCommandPort>,
}

impl MarketplaceFinancialRuntime {
    pub fn new(ledger_port: Arc<dyn MarketplaceLedgerCommandPort>) -> Self {
        Self { ledger_port }
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
            commission,
        ));
        Self::new(ledger)
    }

    pub fn ledger_port(&self) -> Arc<dyn MarketplaceLedgerCommandPort> {
        self.ledger_port.clone()
    }

    pub fn paid_event_inbox(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> MarketplacePaidEventInboxService {
        MarketplacePaidEventInboxService::new(db, event_bus, self.ledger_port())
    }

    pub fn operator_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> MarketplaceFinancialOperatorService {
        MarketplaceFinancialOperatorService::new(db, event_bus, self.ledger_port())
    }
}
