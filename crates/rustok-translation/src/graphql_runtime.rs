use std::sync::Arc;

use rustok_outbox::TransactionalEventBus;
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
}

impl TranslationGraphqlRuntimeData {
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

    Ok(TranslationGraphqlRuntimeData {
        database,
        providers,
        tenant_locale_policies,
        event_bus,
    })
}
