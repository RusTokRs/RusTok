use std::collections::HashMap;
use std::sync::Arc;

use crate::events::EventHandler;
use crate::migrations::ModuleMigration;
use crate::module::{
    ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleKind, ModuleRuntimeExtensions,
    RusToKModule,
};

/// Registry of all platform modules.
///
/// Modules are split into two immutable buckets:
/// - `core_modules`     — `ModuleKind::Core`: always active, cannot be disabled.
/// - `optional_modules` — `ModuleKind::Optional`: per-tenant toggle via `ModuleLifecycleService`.
///
/// # Core modules (DO NOT REMOVE OR RECLASSIFY without an ADR)
/// | slug     | crate            | reason                                        |
/// |----------|------------------|-----------------------------------------------|
/// | `index`  | rustok-index     | CQRS read-path, storefront depends on it      |
/// | `tenant` | rustok-tenant    | tenant resolution, every request passes here  |
/// | `rbac`   | rustok-rbac      | RBAC enforcement on all CRUD handlers         |
#[derive(Clone, Default)]
pub struct ModuleRegistry {
    core_modules: Arc<HashMap<String, Arc<dyn RusToKModule>>>,
    optional_modules: Arc<HashMap<String, Arc<dyn RusToKModule>>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            core_modules: Arc::new(HashMap::new()),
            optional_modules: Arc::new(HashMap::new()),
        }
    }

    pub fn register<M: RusToKModule + 'static>(mut self, module: M) -> Self {
        match module.kind() {
            ModuleKind::Core => {
                let map = Arc::make_mut(&mut self.core_modules);
                map.insert(module.slug().to_string(), Arc::new(module));
            }
            ModuleKind::Optional => {
                let map = Arc::make_mut(&mut self.optional_modules);
                map.insert(module.slug().to_string(), Arc::new(module));
            }
        }
        self
    }

    pub fn get(&self, slug: &str) -> Option<&dyn RusToKModule> {
        self.core_modules
            .get(slug)
            .or_else(|| self.optional_modules.get(slug))
            .map(|module| module.as_ref())
    }

    /// Returns `true` if the module is registered as `ModuleKind::Core`.
    pub fn is_core(&self, slug: &str) -> bool {
        self.core_modules.contains_key(slug)
    }

    pub fn list(&self) -> Vec<&dyn RusToKModule> {
        let mut modules: Vec<&dyn RusToKModule> = self
            .core_modules
            .values()
            .chain(self.optional_modules.values())
            .map(|module| module.as_ref())
            .collect();
        modules.sort_by_key(|module| module.slug());
        modules
    }

    /// Returns an iterator over all registered modules (core + optional).
    pub fn modules(&self) -> impl Iterator<Item = &Arc<dyn RusToKModule>> {
        self.core_modules
            .values()
            .chain(self.optional_modules.values())
    }

    pub fn migrations(&self) -> Vec<ModuleMigration> {
        self.list()
            .into_iter()
            .map(|module| ModuleMigration {
                module_slug: module.slug(),
                migrations: module.migrations(),
            })
            .collect()
    }

    pub fn build_runtime_extensions(&self) -> ModuleRuntimeExtensions {
        let mut extensions = ModuleRuntimeExtensions::default();
        for module in self.list() {
            module.register_runtime_extensions(&mut extensions);
        }
        extensions
    }

    pub fn build_event_listeners(
        &self,
        ctx: &ModuleEventListenerContext<'_>,
    ) -> Vec<Arc<dyn EventHandler>> {
        let mut registry = ModuleEventListenerRegistry::new();
        for module in self.list() {
            module.register_event_listeners(&mut registry, ctx);
        }
        registry.into_handlers()
    }

    pub fn contains(&self, slug: &str) -> bool {
        self.core_modules.contains_key(slug) || self.optional_modules.contains_key(slug)
    }
}

#[cfg(test)]
mod tests {
    use super::ModuleRegistry;
    use crate::events::{DomainEvent, EventEnvelope, EventHandler, HandlerResult};
    use crate::module::{
        MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
        ModuleRuntimeExtensions, RusToKModule,
    };
    use async_trait::async_trait;
    use sea_orm::{Database, DatabaseConnection};
    use sea_orm_migration::MigrationTrait;
    use std::sync::Arc;

    #[derive(Clone)]
    struct TestRuntimeValue(&'static str);

    struct TestHandler {
        name: &'static str,
    }

    #[async_trait]
    impl EventHandler for TestHandler {
        fn name(&self) -> &'static str {
            self.name
        }

        fn handles(&self, _event: &DomainEvent) -> bool {
            true
        }

        async fn handle(&self, _envelope: &EventEnvelope) -> HandlerResult {
            Ok(())
        }
    }

    struct DemoModule {
        slug: &'static str,
    }

    impl MigrationSource for DemoModule {
        fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
            Vec::new()
        }
    }

    #[async_trait]
    impl RusToKModule for DemoModule {
        fn slug(&self) -> &'static str {
            self.slug
        }

        fn name(&self) -> &'static str {
            self.slug
        }

        fn description(&self) -> &'static str {
            "demo module"
        }

        fn version(&self) -> &'static str {
            "0.1.0"
        }

        fn register_runtime_extensions(&self, extensions: &mut ModuleRuntimeExtensions) {
            extensions
                .get_or_insert_with::<Vec<&'static str>, _>(Vec::new)
                .push(self.slug);
        }

        fn register_event_listeners(
            &self,
            registry: &mut ModuleEventListenerRegistry,
            ctx: &ModuleEventListenerContext<'_>,
        ) {
            let runtime = ctx
                .extensions
                .get::<TestRuntimeValue>()
                .expect("runtime value should be present");
            registry.register_boxed(Arc::new(TestHandler { name: runtime.0 }));
        }
    }

    #[test]
    fn build_runtime_extensions_collects_module_owned_capabilities() {
        let registry = ModuleRegistry::new()
            .register(DemoModule { slug: "one" })
            .register(DemoModule { slug: "two" });

        let extensions = registry.build_runtime_extensions();

        assert_eq!(
            extensions
                .get::<Vec<&'static str>>()
                .expect("runtime extension vector should be present"),
            &vec!["one", "two"]
        );
    }

    #[tokio::test]
    async fn build_event_listeners_collects_handlers_from_registered_modules() {
        let registry = ModuleRegistry::new()
            .register(DemoModule { slug: "one" })
            .register(DemoModule { slug: "two" });
        let db = in_memory_db().await;
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(TestRuntimeValue("demo_handler"));
        let ctx = ModuleEventListenerContext {
            db,
            extensions: &extensions,
        };

        let handlers = registry.build_event_listeners(&ctx);

        assert_eq!(handlers.len(), 2);
        assert!(handlers
            .iter()
            .all(|handler| handler.name() == "demo_handler"));
    }

    async fn in_memory_db() -> DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite should connect")
    }
}
