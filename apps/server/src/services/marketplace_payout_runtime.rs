use std::sync::Arc;

use rustok_api::HostRuntimeContext;
use rustok_marketplace_allocation::{MarketplaceAllocationReadPort, MarketplaceAllocationService};
use rustok_marketplace_commission::{MarketplaceCommissionReadPort, MarketplaceCommissionService};
use rustok_marketplace_ledger::{
    MarketplaceLedgerCommandPort, MarketplaceLedgerReadPort, MarketplaceLedgerService,
};
use rustok_marketplace_payout::{
    MarketplacePayoutProviderSubmissionService, MarketplacePayoutService, PayoutProviderRegistry,
};
use sea_orm::DatabaseConnection;

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Server-owned in-process composition for the marketplace payout command path.
///
/// The runtime keeps the entire owner chain behind typed ports:
/// allocation -> commission -> ledger -> payout -> provider submission. The same
/// ledger instance is exposed as both payout read and command dependency, while
/// provider submission and recovery share one process-scoped provider registry.
#[derive(Clone)]
pub struct MarketplacePayoutRuntime {
    ledger_service: Arc<MarketplaceLedgerService>,
    payout_service: Arc<MarketplacePayoutService>,
    provider_registry: Arc<PayoutProviderRegistry>,
    provider_submission_service: Arc<MarketplacePayoutProviderSubmissionService>,
}

impl MarketplacePayoutRuntime {
    pub fn in_process(db: DatabaseConnection) -> Self {
        Self::in_process_with_registry(db, Arc::new(PayoutProviderRegistry::with_manual_provider()))
    }

    pub fn in_process_with_registry(
        db: DatabaseConnection,
        provider_registry: Arc<PayoutProviderRegistry>,
    ) -> Self {
        let allocation_reader: Arc<dyn MarketplaceAllocationReadPort> =
            Arc::new(MarketplaceAllocationService::new(db.clone()));
        let commission_reader: Arc<dyn MarketplaceCommissionReadPort> = Arc::new(
            MarketplaceCommissionService::new(db.clone(), allocation_reader),
        );
        let ledger_service = Arc::new(MarketplaceLedgerService::new(db.clone(), commission_reader));
        let ledger_reader: Arc<dyn MarketplaceLedgerReadPort> = ledger_service.clone();
        let ledger_writer: Arc<dyn MarketplaceLedgerCommandPort> = ledger_service.clone();
        let payout_service = Arc::new(
            MarketplacePayoutService::new(db.clone(), ledger_reader)
                .with_ledger_writer(ledger_writer),
        );
        let provider_submission_service = Arc::new(
            MarketplacePayoutProviderSubmissionService::new(db, provider_registry.clone()),
        );

        Self {
            ledger_service,
            payout_service,
            provider_registry,
            provider_submission_service,
        }
    }

    pub fn ledger_service(&self) -> Arc<MarketplaceLedgerService> {
        self.ledger_service.clone()
    }

    pub fn payout_service(&self) -> Arc<MarketplacePayoutService> {
        self.payout_service.clone()
    }

    pub fn provider_registry(&self) -> Arc<PayoutProviderRegistry> {
        self.provider_registry.clone()
    }

    pub fn provider_submission_service(&self) -> Arc<MarketplacePayoutProviderSubmissionService> {
        self.provider_submission_service.clone()
    }

    fn has_same_owner_chain(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ledger_service, &other.ledger_service)
            && Arc::ptr_eq(&self.payout_service, &other.payout_service)
            && Arc::ptr_eq(&self.provider_registry, &other.provider_registry)
            && Arc::ptr_eq(
                &self.provider_submission_service,
                &other.provider_submission_service,
            )
    }
}

/// Attach one process-wide marketplace payout runtime to both the server and
/// module host contexts. A runtime or provider registry already supplied by the
/// deployment host is preserved instead of being replaced by the manual baseline.
pub fn attach_marketplace_payout_runtime(
    host: HostRuntimeContext,
    server: &ServerRuntimeContext,
) -> HostRuntimeContext {
    let runtime = resolve_marketplace_payout_runtime(&host, server);
    let ledger_service = runtime.ledger_service();
    let payout_service = runtime.payout_service();
    let provider_registry = runtime.provider_registry();
    let provider_submission_service = runtime.provider_submission_service();

    install_provider_registry(server, provider_registry.clone());
    server.shared_insert_if_absent(ledger_service.clone());
    server.shared_insert_if_absent(payout_service.clone());
    server.shared_insert_if_absent(provider_submission_service.clone());

    host.with_shared_value(runtime)
        .with_shared_value(ledger_service)
        .with_shared_value(payout_service)
        .with_shared_value(provider_registry)
        .with_shared_value(provider_submission_service)
}

fn resolve_marketplace_payout_runtime(
    host: &HostRuntimeContext,
    server: &ServerRuntimeContext,
) -> MarketplacePayoutRuntime {
    if let Some(host_runtime) = host.shared_get::<MarketplacePayoutRuntime>() {
        let runtime_registry = host_runtime.provider_registry();
        if let Some(host_registry) = host.shared_get::<Arc<PayoutProviderRegistry>>() {
            assert_registry_identity(&runtime_registry, &host_registry);
        }
        install_provider_registry(server, runtime_registry);

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

    if let Some(server_runtime) = server.shared_get::<MarketplacePayoutRuntime>() {
        let runtime_registry = server_runtime.provider_registry();
        if let Some(host_registry) = host.shared_get::<Arc<PayoutProviderRegistry>>() {
            assert_registry_identity(&runtime_registry, &host_registry);
        }
        install_provider_registry(server, runtime_registry);
        return server_runtime;
    }

    let provider_registry = resolve_provider_registry(host, server);
    let candidate = MarketplacePayoutRuntime::in_process_with_registry(
        server.db_clone(),
        provider_registry.clone(),
    );
    if server.shared_insert_if_absent(candidate.clone()) {
        candidate
    } else {
        let installed = server
            .shared_get::<MarketplacePayoutRuntime>()
            .expect("marketplace payout runtime insertion winner must remain available");
        assert_registry_identity(&provider_registry, &installed.provider_registry());
        installed
    }
}

fn resolve_provider_registry(
    host: &HostRuntimeContext,
    server: &ServerRuntimeContext,
) -> Arc<PayoutProviderRegistry> {
    if let Some(host_registry) = host.shared_get::<Arc<PayoutProviderRegistry>>() {
        install_provider_registry(server, host_registry.clone());
        return host_registry;
    }
    if let Some(server_registry) = server.shared_get::<Arc<PayoutProviderRegistry>>() {
        return server_registry;
    }

    let candidate = Arc::new(PayoutProviderRegistry::with_manual_provider());
    if server.shared_insert_if_absent(candidate.clone()) {
        candidate
    } else {
        server
            .shared_get::<Arc<PayoutProviderRegistry>>()
            .expect("payout provider registry insertion winner must remain available")
    }
}

fn install_provider_registry(
    server: &ServerRuntimeContext,
    provider_registry: Arc<PayoutProviderRegistry>,
) {
    if let Some(installed) = server.shared_get::<Arc<PayoutProviderRegistry>>() {
        assert_registry_identity(&provider_registry, &installed);
        return;
    }
    if server.shared_insert_if_absent(provider_registry.clone()) {
        return;
    }
    let installed = server
        .shared_get::<Arc<PayoutProviderRegistry>>()
        .expect("payout provider registry insertion winner must remain available");
    assert_registry_identity(&provider_registry, &installed);
}

fn assert_runtime_identity(expected: &MarketplacePayoutRuntime, actual: &MarketplacePayoutRuntime) {
    assert!(
        expected.has_same_owner_chain(actual),
        "conflicting marketplace payout runtimes would split ledger or provider command ownership"
    );
}

fn assert_registry_identity(
    expected: &Arc<PayoutProviderRegistry>,
    actual: &Arc<PayoutProviderRegistry>,
) {
    assert!(
        Arc::ptr_eq(expected, actual),
        "conflicting payout provider registries would split external effect ownership"
    );
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustok_marketplace_payout::{
        MarketplacePayoutProviderSubmissionService, PayoutProviderRegistry,
    };
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

        assert!(Arc::ptr_eq(
            &first_runtime.payout_service(),
            &second_runtime.payout_service()
        ));
        assert!(Arc::ptr_eq(
            &first_runtime.ledger_service(),
            &second_runtime.ledger_service()
        ));
        assert!(Arc::ptr_eq(
            &first_runtime.provider_registry(),
            &second_runtime.provider_registry()
        ));
        assert!(Arc::ptr_eq(
            &first_runtime.provider_submission_service(),
            &second_runtime.provider_submission_service()
        ));
        assert!(first
            .shared_get::<Arc<rustok_marketplace_payout::MarketplacePayoutService>>()
            .is_some());
        assert!(first
            .shared_get::<Arc<rustok_marketplace_ledger::MarketplaceLedgerService>>()
            .is_some());
        assert!(first.shared_get::<Arc<PayoutProviderRegistry>>().is_some());
        assert!(first
            .shared_get::<Arc<MarketplacePayoutProviderSubmissionService>>()
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
        let supplied_registry = supplied.provider_registry();
        let supplied_submission = supplied.provider_submission_service();
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
        assert!(Arc::ptr_eq(
            &supplied_registry,
            &attached_runtime.provider_registry()
        ));
        assert!(Arc::ptr_eq(
            &supplied_submission,
            &attached_runtime.provider_submission_service()
        ));
    }

    #[tokio::test]
    async fn preserves_a_provider_registry_supplied_by_the_host_extension_layer() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        let server = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
        let supplied_registry = Arc::new(PayoutProviderRegistry::with_manual_provider());
        let host =
            rustok_api::HostRuntimeContext::new(db).with_shared_value(supplied_registry.clone());

        let attached = attach_marketplace_payout_runtime(host, &server);
        let attached_registry = attached
            .shared_get::<Arc<PayoutProviderRegistry>>()
            .expect("host provider registry should remain attached");
        let attached_runtime = attached
            .shared_get::<MarketplacePayoutRuntime>()
            .expect("payout runtime should be attached");

        assert!(Arc::ptr_eq(&supplied_registry, &attached_registry));
        assert!(Arc::ptr_eq(
            &supplied_registry,
            &attached_runtime.provider_registry()
        ));
    }
}
