use std::sync::Arc;

use rustok_api::HostRuntimeContext;
use rustok_marketplace_allocation::{
    MarketplaceAllocationReadPort, MarketplaceAllocationService,
};
use rustok_marketplace_commission::{
    MarketplaceCommissionReadPort, MarketplaceCommissionService,
};
use rustok_marketplace_ledger::{
    MarketplaceLedgerCommandPort, MarketplaceLedgerReadPort, MarketplaceLedgerService,
};
use rustok_marketplace_payout::MarketplacePayoutService;
use sea_orm::DatabaseConnection;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Server-owned in-process composition for the marketplace payout command path.
///
/// The runtime keeps the entire owner chain behind typed ports:
/// allocation -> commission -> ledger -> payout. The same ledger instance is
/// exposed as both the payout read and command dependency so reserve/release
/// calls and seller-entry reads observe one database-backed owner.
#[derive(Clone)]
pub struct MarketplacePayoutRuntime {
    ledger_service: Arc<MarketplaceLedgerService>,
    payout_service: Arc<MarketplacePayoutService>,
}

impl MarketplacePayoutRuntime {
    pub fn in_process(db: DatabaseConnection) -> Self {
        let allocation_reader: Arc<dyn MarketplaceAllocationReadPort> =
            Arc::new(MarketplaceAllocationService::new(db.clone()));
        let commission_reader: Arc<dyn MarketplaceCommissionReadPort> = Arc::new(
            MarketplaceCommissionService::new(db.clone(), allocation_reader),
        );
        let ledger_service = Arc::new(MarketplaceLedgerService::new(
            db.clone(),
            commission_reader,
        ));
        let ledger_reader: Arc<dyn MarketplaceLedgerReadPort> = ledger_service.clone();
        let ledger_writer: Arc<dyn MarketplaceLedgerCommandPort> = ledger_service.clone();
        let payout_service = Arc::new(
            MarketplacePayoutService::new(db, ledger_reader).with_ledger_writer(ledger_writer),
        );

        Self {
            ledger_service,
            payout_service,
        }
    }

    pub fn ledger_service(&self) -> Arc<MarketplaceLedgerService> {
        self.ledger_service.clone()
    }

    pub fn payout_service(&self) -> Arc<MarketplacePayoutService> {
        self.payout_service.clone()
    }

    fn has_same_owner_chain(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ledger_service, &other.ledger_service)
            && Arc::ptr_eq(&self.payout_service, &other.payout_service)
    }
}

/// Attach one process-wide marketplace payout runtime to both the server and
/// module host contexts. A runtime already supplied by the deployment host is
/// preserved instead of being replaced by the default in-process composition.
pub fn attach_marketplace_payout_runtime(
    host: HostRuntimeContext,
    server: &ServerRuntimeContext,
) -> HostRuntimeContext {
    let runtime = resolve_marketplace_payout_runtime(&host, server);
    let ledger_service = runtime.ledger_service();
    let payout_service = runtime.payout_service();

    server.shared_insert_if_absent(ledger_service.clone());
    server.shared_insert_if_absent(payout_service.clone());

    host.with_shared_value(runtime)
        .with_shared_value(ledger_service)
        .with_shared_value(payout_service)
}

fn resolve_marketplace_payout_runtime(
    host: &HostRuntimeContext,
    server: &ServerRuntimeContext,
) -> MarketplacePayoutRuntime {
    if let Some(host_runtime) = host.shared_get::<MarketplacePayoutRuntime>() {
        if let Some(server_runtime) = server.shared_get::<MarketplacePayoutRuntime>() {
            assert_runtime_identity(&host_runtime, &server_runtime);
            return host_runtime;
        }

        if server.shared_insert_if_absent(host_runtime.clone()) {
            return host_runtime;
        }

        let installed = server
            .shared_get::<MarketplacePayoutRuntime>()
            .expect("marketplace payout runtime insertion winner must remain available");
        assert_runtime_identity(&host_runtime, &installed);
        return host_runtime;
    }

    if let Some(runtime) = server.shared_get::<MarketplacePayoutRuntime>() {
        return runtime;
    }

    let candidate = MarketplacePayoutRuntime::in_process(server.db_clone());
    if server.shared_insert_if_absent(candidate.clone()) {
        candidate
    } else {
        server
            .shared_get::<MarketplacePayoutRuntime>()
            .expect("marketplace payout runtime insertion winner must remain available")
    }
}

fn assert_runtime_identity(expected: &MarketplacePayoutRuntime, actual: &MarketplacePayoutRuntime) {
    assert!(
        expected.has_same_owner_chain(actual),
        "conflicting marketplace payout runtimes would split ledger command ownership"
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sea_orm::Database;

    use super::{attach_marketplace_payout_runtime, MarketplacePayoutRuntime};
    use crate::common::settings::RustokSettings;
    use crate::services::server_runtime_context::ServerRuntimeContext;

    #[tokio::test]
    async fn attaches_and_reuses_one_process_owned_payout_runtime() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());

        let first = attach_marketplace_payout_runtime(
            rustok_api::HostRuntimeContext::new(db.clone()),
            &server,
        );
        let second =
            attach_marketplace_payout_runtime(rustok_api::HostRuntimeContext::new(db), &server);

        let first_runtime = first
            .shared_get::<MarketplacePayoutRuntime>()
            .expect("payout runtime should be attached");
        let second_runtime = second
            .shared_get::<MarketplacePayoutRuntime>()
            .expect("payout runtime should be reused");
        let first_payout = first_runtime.payout_service();
        let second_payout = second_runtime.payout_service();
        let first_ledger = first_runtime.ledger_service();
        let second_ledger = second_runtime.ledger_service();

        assert!(Arc::ptr_eq(&first_payout, &second_payout));
        assert!(Arc::ptr_eq(&first_ledger, &second_ledger));
        assert!(first
            .shared_get::<Arc<rustok_marketplace_payout::MarketplacePayoutService>>()
            .is_some());
        assert!(first
            .shared_get::<Arc<rustok_marketplace_ledger::MarketplaceLedgerService>>()
            .is_some());
    }

    #[tokio::test]
    async fn preserves_a_runtime_supplied_by_the_host_extension_layer() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let supplied = MarketplacePayoutRuntime::in_process(db.clone());
        let supplied_payout = supplied.payout_service();
        let supplied_ledger = supplied.ledger_service();
        let host = rustok_api::HostRuntimeContext::new(db).with_shared_value(supplied);

        let attached = attach_marketplace_payout_runtime(host, &server);
        let attached_runtime = attached
            .shared_get::<MarketplacePayoutRuntime>()
            .expect("host runtime should remain attached");

        assert!(Arc::ptr_eq(
            &supplied_payout,
            &attached_runtime.payout_service()
        ));
        assert!(Arc::ptr_eq(
            &supplied_ledger,
            &attached_runtime.ledger_service()
        ));
    }
}
