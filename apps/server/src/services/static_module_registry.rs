use std::{collections::HashMap, fmt::Display, sync::Arc};

use async_trait::async_trait;
use rustok_api::StaticModuleRegistryView;
use rustok_core::ModuleRegistry;
use rustok_modules::{
    ModuleControlPlane, STATIC_MODULE_REGISTRY_MAX_LIMIT, SharedStaticModuleRegistryReader,
    StaticModuleRegistryQuery, StaticModuleRegistryReader, StaticModuleRegistryReaderError,
};
use uuid::Uuid;

use crate::modules::ManifestManager;
use crate::services::marketplace_catalog::MarketplaceCatalogQuery;
use crate::services::marketplace_catalog_adapter::project_marketplace_catalog_entries;
use crate::services::platform_composition::PlatformCompositionService;
use crate::services::server_runtime_context::ServerRuntimeContext;

/// Server host adapter for the canonical static module-registry projection.
///
/// It resolves all fields from one active composition before exposing the
/// browser-safe DTO to GraphQL or native Admin server functions.
#[derive(Clone)]
pub struct ServerStaticModuleRegistryReader {
    runtime: ServerRuntimeContext,
    registry: ModuleRegistry,
}

impl ServerStaticModuleRegistryReader {
    pub fn shared(
        runtime: ServerRuntimeContext,
        registry: ModuleRegistry,
    ) -> SharedStaticModuleRegistryReader {
        SharedStaticModuleRegistryReader(Arc::new(Self { runtime, registry }))
    }
}

/// Returns the single server-composed static registry reader for this runtime.
///
/// GraphQL and Leptos use the exact same reader instance so host composition
/// cannot drift across their static registry transports.
pub fn static_module_registry_reader_from_context(
    runtime: &ServerRuntimeContext,
    registry: ModuleRegistry,
) -> SharedStaticModuleRegistryReader {
    if let Some(reader) = runtime.shared_get::<SharedStaticModuleRegistryReader>() {
        return reader;
    }

    let candidate = ServerStaticModuleRegistryReader::shared(runtime.clone(), registry);
    let _ = runtime.shared_insert_if_absent(candidate.clone());
    runtime
        .shared_get::<SharedStaticModuleRegistryReader>()
        .unwrap_or(candidate)
}

#[async_trait]
impl StaticModuleRegistryReader for ServerStaticModuleRegistryReader {
    async fn list(
        &self,
        query: StaticModuleRegistryQuery,
    ) -> Result<Vec<StaticModuleRegistryView>, StaticModuleRegistryReaderError> {
        let manifest = PlatformCompositionService::active_manifest(self.runtime.db())
            .await
            .map_err(|error| unavailable(query.tenant_id, "active composition", error))?;
        let co_requisites = ManifestManager::module_policy_corequisites(&manifest)
            .map_err(|error| unavailable(query.tenant_id, "module policy co-requisites", error))?;
        let registry_modules = self
            .registry
            .list()
            .into_iter()
            .take(query.limit.clamp(1, STATIC_MODULE_REGISTRY_MAX_LIMIT) as usize)
            .collect::<Vec<_>>();
        let lifecycle = ModuleControlPlane::new(self.runtime.db_clone())
            .lifecycle(&self.registry, manifest.settings.default_enabled.clone())
            .with_corequisites(co_requisites);
        let enabled_modules = lifecycle
            .effective_policy(query.tenant_id)
            .await
            .map_err(|error| unavailable(query.tenant_id, "effective policy", error))?
            .into_enabled_modules();
        let lifecycle_revisions = lifecycle
            .static_lifecycle_snapshots(
                query.tenant_id,
                registry_modules
                    .iter()
                    .map(|module| module.slug().to_string()),
            )
            .await
            .map_err(|error| unavailable(query.tenant_id, "lifecycle revisions", error))?;
        let catalog_by_slug = project_marketplace_catalog_entries(
            &self.runtime,
            &manifest,
            &self.registry,
            &MarketplaceCatalogQuery::default(),
            Some(query.preferred_locale.as_str()),
            Some(query.fallback_locale.as_str()),
        )
        .await
        .map_err(|error| unavailable(query.tenant_id, "catalog projection", error))?
        .into_iter()
        .map(|entry| (entry.slug.clone(), entry))
        .collect::<HashMap<_, _>>();

        registry_modules
            .into_iter()
            .map(|module| {
                let lifecycle_snapshot =
                    lifecycle_revisions.get(module.slug()).ok_or_else(|| {
                        unavailable(
                            query.tenant_id,
                            "lifecycle revisions",
                            format!("missing static lifecycle revision for '{}'", module.slug()),
                        )
                    })?;
                let lifecycle_revision = browser_safe_lifecycle_revision(
                    lifecycle_snapshot.revision,
                )
                .inspect_err(|_error| {
                    tracing::error!(
                        tenant_id = %query.tenant_id,
                        module_slug = module.slug(),
                        "static module lifecycle revision is outside the browser-safe range"
                    );
                })?;
                let catalog_entry = catalog_by_slug.get(module.slug());

                Ok(StaticModuleRegistryView {
                    module_slug: module.slug().to_string(),
                    name: module.name().to_string(),
                    description: module.description().to_string(),
                    version: module.version().to_string(),
                    kind: if self.registry.is_core(module.slug()) {
                        "core".to_string()
                    } else {
                        "optional".to_string()
                    },
                    dependencies: module
                        .dependencies()
                        .iter()
                        .map(|dependency| dependency.to_string())
                        .collect(),
                    enabled: enabled_modules.contains(module.slug()),
                    lifecycle_revision,
                    ownership: catalog_entry
                        .map(|entry| entry.ownership.clone())
                        .unwrap_or_else(|| "third_party".to_string()),
                    trust_level: catalog_entry
                        .map(|entry| entry.trust_level.clone())
                        .unwrap_or_else(|| "unverified".to_string()),
                    has_admin_ui: catalog_entry.is_some_and(|entry| entry.has_admin_ui),
                    has_storefront_ui: catalog_entry.is_some_and(|entry| entry.has_storefront_ui),
                    ui_classification: catalog_entry
                        .map(|entry| entry.ui_classification.clone())
                        .unwrap_or_else(|| "no_ui".to_string()),
                    recommended_admin_surfaces: catalog_entry
                        .map(|entry| entry.recommended_admin_surfaces.clone())
                        .unwrap_or_default(),
                    showcase_admin_surfaces: catalog_entry
                        .map(|entry| entry.showcase_admin_surfaces.clone())
                        .unwrap_or_default(),
                })
            })
            .collect()
    }
}

fn browser_safe_lifecycle_revision(revision: u64) -> Result<i64, StaticModuleRegistryReaderError> {
    i64::try_from(revision).map_err(|_| StaticModuleRegistryReaderError::Unavailable)
}

fn unavailable(
    tenant_id: Uuid,
    stage: &'static str,
    error: impl Display,
) -> StaticModuleRegistryReaderError {
    tracing::error!(
        %tenant_id,
        stage,
        error = %error,
        "failed to resolve the static module registry projection"
    );
    StaticModuleRegistryReaderError::Unavailable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_revision_must_fit_the_browser_safe_range() {
        assert_eq!(browser_safe_lifecycle_revision(0), Ok(0));
        assert_eq!(
            browser_safe_lifecycle_revision(i64::MAX as u64 + 1),
            Err(StaticModuleRegistryReaderError::Unavailable)
        );
    }
}
