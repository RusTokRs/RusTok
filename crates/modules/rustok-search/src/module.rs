use async_trait::async_trait;
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleKind,
    RusToKModule, module::HealthStatus,
};
use sea_orm_migration::MigrationTrait;

use crate::{
    SearchConnectorDescriptor, SearchIngestionHandler,
    search_projection_source_registry_from_extensions,
};

/// Core search module that owns engine selection and connector-facing contracts.
pub struct SearchModule;

impl SearchModule {
    pub fn available_engines(&self) -> Vec<SearchConnectorDescriptor> {
        vec![SearchConnectorDescriptor::postgres_default()]
    }
}

impl MigrationSource for SearchModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }
}

#[async_trait]
impl RusToKModule for SearchModule {
    fn slug(&self) -> &'static str {
        "search"
    }

    fn name(&self) -> &'static str {
        "Search"
    }

    fn description(&self) -> &'static str {
        "Postgres-first search capability with settings-driven engine selection."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        ctx: &ModuleEventListenerContext<'_>,
    ) {
        let sources = search_projection_source_registry_from_extensions(ctx.extensions);
        let forum_source = sources
            .as_ref()
            .and_then(|s| s.build_with_extensions("forum", ctx.db.clone(), ctx.extensions));
        let blog_source = sources
            .as_ref()
            .and_then(|s| s.build_with_extensions("blog", ctx.db.clone(), ctx.extensions));
        let pages_source = sources
            .as_ref()
            .and_then(|s| s.build_with_extensions("pages", ctx.db.clone(), ctx.extensions));
        let docs_source = sources
            .as_ref()
            .and_then(|s| s.build_with_extensions("docs", ctx.db.clone(), ctx.extensions));
        registry.register(SearchIngestionHandler::with_all_projection_sources(
            ctx.db.clone(),
            blog_source,
            forum_source,
            pages_source,
            docs_source,
        ));
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Degraded
    }
}

#[cfg(test)]
mod tests {
    use super::SearchModule;
    use rustok_core::module::{HealthStatus, RusToKModule};

    #[tokio::test]
    async fn search_module_health_defers_to_host_readiness() {
        assert_eq!(SearchModule.health().await, HealthStatus::Degraded);
    }
}
