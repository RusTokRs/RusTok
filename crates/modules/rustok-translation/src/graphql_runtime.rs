use std::sync::Arc;

use rustok_outbox::TransactionalEventBus;
use rustok_storage::StorageRuntime;
use rustok_tenant::{TenantLocalePolicyPort, TenantService};
use rustok_translation_targets::TranslationTargetRegistry;
use sea_orm::DatabaseConnection;

/// Translation-owned dependencies attached to the GraphQL schema by the
/// manifest-generated host composition.
#[derive(Clone)]
pub struct TranslationGraphqlRuntimeData {
    database: DatabaseConnection,
    providers: Arc<TranslationTargetRegistry>,
    tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
    event_bus: TransactionalEventBus,
    storage: Option<StorageRuntime>,
    machine_port: Option<Arc<dyn crate::MachineTranslationPort>>,
    machine_port_error_code: Option<String>,
}

impl TranslationGraphqlRuntimeData {
    /// Builds the Translation GraphQL runtime from its canonical dependencies.
    ///
    /// Host composition normally uses [`attach_schema_data`]. This constructor
    /// keeps embedded hosts and transport-level tests on the same runtime path
    /// while allowing them to supply their own locale-policy and machine ports.
    pub fn new(
        database: DatabaseConnection,
        providers: Arc<TranslationTargetRegistry>,
        tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
        event_bus: TransactionalEventBus,
        storage: Option<StorageRuntime>,
        machine_port: Option<Arc<dyn crate::MachineTranslationPort>>,
        machine_port_error_code: Option<String>,
    ) -> Self {
        Self {
            database,
            providers,
            tenant_locale_policies,
            event_bus,
            storage,
            machine_port,
            machine_port_error_code,
        }
    }

    pub(crate) fn policy_service(&self) -> crate::TranslationPolicyService {
        crate::TranslationPolicyService::new(
            self.database.clone(),
            Arc::clone(&self.tenant_locale_policies),
        )
    }

    pub(crate) fn progress_service(&self) -> crate::TranslationProgressService {
        crate::TranslationProgressService::new(
            self.database.clone(),
            Arc::clone(&self.providers),
            Arc::clone(&self.tenant_locale_policies),
        )
    }

    pub(crate) fn glossary_service(&self) -> crate::TranslationGlossaryService {
        crate::TranslationGlossaryService::new(
            self.database.clone(),
            Arc::clone(&self.tenant_locale_policies),
        )
    }

    pub(crate) fn memory_service(&self) -> crate::TranslationMemoryService {
        crate::TranslationMemoryService::new(self.database.clone())
    }

    pub(crate) fn inventory_service(&self) -> crate::TranslationInventoryService {
        crate::TranslationInventoryService::new(self.database.clone(), Arc::clone(&self.providers))
    }

    pub(crate) fn workflow_service(&self) -> crate::TranslationWorkflowService {
        crate::TranslationWorkflowService::new(
            self.database.clone(),
            Arc::clone(&self.providers),
            Arc::clone(&self.tenant_locale_policies),
            self.event_bus.clone(),
        )
    }

    pub(crate) fn exchange_service(
        &self,
    ) -> crate::TranslationResult<crate::TranslationExchangeService> {
        let storage = self
            .storage
            .clone()
            .ok_or_else(|| crate::TranslationError::Provider {
                code: "translation.interchange.storage_unavailable".to_string(),
                message: "translation interchange storage is unavailable".to_string(),
                retryable: true,
            })?;
        Ok(crate::TranslationExchangeService::new(
            self.database.clone(),
            Arc::clone(&self.providers),
            Arc::clone(&self.tenant_locale_policies),
            self.event_bus.clone(),
            storage,
        ))
    }

    pub(crate) fn machine_service(
        &self,
    ) -> crate::TranslationResult<crate::TranslationMachineService> {
        let machine_port = self.machine_port.as_ref().cloned().ok_or_else(|| {
            crate::TranslationError::Provider {
                code: self
                    .machine_port_error_code
                    .clone()
                    .unwrap_or_else(|| "translation.machine.provider_unavailable".to_string()),
                message: "machine translation provider is unavailable".to_string(),
                retryable: true,
            }
        })?;
        Ok(crate::TranslationMachineService::new(
            self.database.clone(),
            Arc::clone(&self.providers),
            Arc::clone(&self.tenant_locale_policies),
            self.event_bus.clone(),
            machine_port,
        ))
    }

    pub(crate) fn machine_control_service(&self) -> crate::TranslationMachineControlService {
        crate::TranslationMachineControlService::new(
            self.database.clone(),
            self.machine_port.as_ref().cloned(),
        )
    }

    pub(crate) fn providers(&self) -> &TranslationTargetRegistry {
        self.providers.as_ref()
    }
}

/// Capability-owned factory consumed by manifest-generated schema composition.
pub fn attach_schema_data(
    inputs: &rustok_api::graphql::GraphqlRuntimeInputs,
) -> Result<TranslationGraphqlRuntimeData, String> {
    let database = inputs.db_clone();
    let providers = inputs
        .shared_get::<Arc<TranslationTargetRegistry>>()
        .unwrap_or_else(|| Arc::new(TranslationTargetRegistry::default()));
    let event_bus = inputs
        .shared_get::<TransactionalEventBus>()
        .ok_or_else(|| "transactional event bus is unavailable".to_string())?;
    let tenant_locale_policies: Arc<dyn TenantLocalePolicyPort> =
        Arc::new(TenantService::new(database.clone()));
    let storage = inputs.shared_get::<StorageRuntime>();
    let (machine_port, machine_port_error_code) =
        match crate::machine_translation_port_from_context(inputs.host()) {
            Ok(machine_port) => (machine_port, None),
            Err(error) => (None, Some(error.code)),
        };

    Ok(TranslationGraphqlRuntimeData::new(
        database,
        providers,
        tenant_locale_policies,
        event_bus,
        storage,
        machine_port,
        machine_port_error_code,
    ))
}
