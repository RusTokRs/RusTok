use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::{PaymentService, ports::PaymentCollectionPort};

/// Host-selectable owner runtime for transport-neutral payment collection create/reuse and status reads.
///
/// The runtime owns only the typed Payment boundary. Hosts may provide an external implementation,
/// while the built-in adapter keeps concrete `PaymentService` construction inside `rustok-payment`.
#[derive(Clone)]
pub struct PaymentCollectionRuntime {
    port: Arc<dyn PaymentCollectionPort>,
}

impl PaymentCollectionRuntime {
    pub fn new(port: Arc<dyn PaymentCollectionPort>) -> Self {
        Self { port }
    }

    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::new(Arc::new(PaymentService::new(db)))
    }

    pub fn port(&self) -> Arc<dyn PaymentCollectionPort> {
        self.port.clone()
    }
}
