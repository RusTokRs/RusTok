use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use sea_orm_migration::MigrationTrait;

use crate::migrations::MigrationDependencyDescriptor;
use serde_json::Value;

use crate::events::EventHandler;
use rustok_api::Permission;

pub struct ModuleContext<'a> {
    pub db: &'a DatabaseConnection,
    pub tenant_id: uuid::Uuid,
    pub config: &'a Value,
}

pub trait MigrationSource: Send + Sync {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>>;

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        Vec::new()
    }
}

#[derive(Clone, Default)]
pub struct ModuleRuntimeExtensions {
    entries: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ModuleRuntimeExtensions {
    pub fn insert<T>(&mut self, value: T)
    where
        T: Any + Send + Sync,
    {
        self.entries.insert(TypeId::of::<T>(), Arc::new(value));
    }

    pub fn contains<T>(&self) -> bool
    where
        T: Any + Send + Sync,
    {
        self.entries.contains_key(&TypeId::of::<T>())
    }

    pub fn get<T>(&self) -> Option<&T>
    where
        T: Any + Send + Sync,
    {
        self.entries
            .get(&TypeId::of::<T>())
            .and_then(|value| value.as_ref().downcast_ref::<T>())
    }

    pub fn get_mut<T>(&mut self) -> Option<&mut T>
    where
        T: Any + Send + Sync,
    {
        self.entries
            .get_mut(&TypeId::of::<T>())
            .and_then(Arc::get_mut)
            .and_then(|value| value.downcast_mut::<T>())
    }

    pub fn get_or_insert_with<T, F>(&mut self, init: F) -> &mut T
    where
        T: Any + Send + Sync,
        F: FnOnce() -> T,
    {
        if !self.contains::<T>() {
            self.insert(init());
        }

        self.get_mut::<T>()
            .expect("runtime extension should be uniquely owned during registration")
    }

    /// Transfers all module-published typed values into a neutral host runtime
    /// context. The host does not inspect individual capability types.
    #[cfg(feature = "server")]
    pub fn apply_to_host_runtime(
        &self,
        host: rustok_api::HostRuntimeContext,
    ) -> rustok_api::HostRuntimeContext {
        host.with_extension_values(
            self.entries
                .iter()
                .map(|(type_id, value)| (*type_id, value.clone())),
        )
    }
}

pub struct ModuleEventListenerContext<'a> {
    pub db: DatabaseConnection,
    pub extensions: &'a ModuleRuntimeExtensions,
}

#[derive(Default)]
pub struct ModuleEventListenerRegistry {
    handlers: Vec<Arc<dyn EventHandler>>,
}

impl ModuleEventListenerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<H>(&mut self, handler: H)
    where
        H: EventHandler,
    {
        self.handlers.push(Arc::new(handler));
    }

    pub fn register_boxed(&mut self, handler: Arc<dyn EventHandler>) {
        self.handlers.push(handler);
    }

    pub fn into_handlers(self) -> Vec<Arc<dyn EventHandler>> {
        self.handlers
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Classifies a module as part of the platform kernel or as an optional domain module.
///
/// # IMPORTANT — DO NOT CHANGE LIGHTLY
/// Modules with `ModuleKind::Core` are **permanently active** and cannot be disabled
/// by any tenant or operator. They are essential for platform correctness:
/// - `index`   — CQRS read-path; storefront reads from index tables
/// - `tenant`  — tenant resolution middleware; every HTTP request passes through it
/// - `rbac`    — RBAC enforcement; all CRUD handlers check permissions here
///
/// Removing or downgrading any of these to `Optional` will break platform guarantees.
/// Any such change requires an ADR in `DECISIONS/`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleKind {
    /// Always active. Cannot be disabled per-tenant.
    /// Registered in the `core_modules` bucket of `ModuleRegistry`.
    Core,
    /// Can be enabled or disabled per-tenant via `ModuleLifecycleService`.
    Optional,
}

#[async_trait]
pub trait RusToKModule: Send + Sync + MigrationSource {
    fn slug(&self) -> &'static str;

    fn name(&self) -> &'static str;

    fn description(&self) -> &'static str;

    fn version(&self) -> &'static str;

    /// Returns `ModuleKind::Core` for platform-critical modules that must never be disabled.
    /// Defaults to `ModuleKind::Optional` — safe for all domain modules.
    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    /// Returns the list of permissions this module declares.
    /// Used for dynamic RBAC permission registration.
    fn permissions(&self) -> Vec<Permission> {
        Vec::new()
    }

    fn register_event_listeners(
        &self,
        _registry: &mut ModuleEventListenerRegistry,
        _ctx: &ModuleEventListenerContext<'_>,
    ) {
    }

    /// Registers module-owned typed runtime capabilities.
    ///
    /// Duplicate providers and invalid deployment configuration must be returned
    /// as errors so the host can fail startup cleanly instead of panicking.
    fn register_runtime_extensions(
        &self,
        _extensions: &mut ModuleRuntimeExtensions,
    ) -> crate::Result<()> {
        Ok(())
    }

    /// Legacy lifecycle hook kept for backward compatibility.
    ///
    /// Runtime now calls `pre_enable` by default before tenant-state commit.
    async fn on_enable(&self, _ctx: ModuleContext<'_>) -> crate::Result<()> {
        Ok(())
    }

    /// Legacy lifecycle hook kept for backward compatibility.
    ///
    /// Runtime now calls `pre_disable` by default before tenant-state commit.
    async fn on_disable(&self, _ctx: ModuleContext<'_>) -> crate::Result<()> {
        Ok(())
    }

    async fn pre_enable(&self, ctx: ModuleContext<'_>) -> crate::Result<()> {
        self.on_enable(ctx).await
    }

    async fn pre_disable(&self, ctx: ModuleContext<'_>) -> crate::Result<()> {
        self.on_disable(ctx).await
    }

    async fn post_enable(&self, _ctx: ModuleContext<'_>) -> crate::Result<()> {
        Ok(())
    }

    async fn post_disable(&self, _ctx: ModuleContext<'_>) -> crate::Result<()> {
        Ok(())
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::ModuleRuntimeExtensions;

    #[derive(Debug, Eq, PartialEq)]
    struct DemoRuntimeValue(&'static str);

    #[test]
    fn module_runtime_extensions_store_and_resolve_typed_values() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(DemoRuntimeValue("demo"));

        let value = extensions
            .get::<DemoRuntimeValue>()
            .expect("typed runtime value should be available");

        assert_eq!(value, &DemoRuntimeValue("demo"));
        assert!(extensions.get::<String>().is_none());
    }

    #[test]
    fn module_runtime_extensions_support_mutation_and_seeded_registration() {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(Vec::<&'static str>::new());
        extensions
            .get_mut::<Vec<&'static str>>()
            .expect("vector should be present")
            .push("one");
        extensions
            .get_or_insert_with::<Vec<&'static str>, _>(Vec::new)
            .push("two");

        assert_eq!(
            extensions
                .get::<Vec<&'static str>>()
                .expect("vector should stay registered"),
            &vec!["one", "two"]
        );
    }
}
