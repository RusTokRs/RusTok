use std::collections::HashMap;
use std::sync::Arc;

use crate::migrations::ModuleMigration;
use crate::module::{EventListener, ModuleKind, RusToKModule};

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
            .map(|m| m.as_ref())
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
            .map(|m| m.as_ref())
            .collect();
        modules.sort_by_key(|m| m.slug());
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

    pub fn event_listeners(&self) -> Vec<Box<dyn EventListener>> {
        self.list()
            .into_iter()
            .flat_map(|module| module.event_listeners())
            .collect()
    }

    pub fn contains(&self, slug: &str) -> bool {
        self.core_modules.contains_key(slug) || self.optional_modules.contains_key(slug)
    }
}
