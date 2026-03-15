use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use sea_orm_migration::MigrationTrait;
use serde_json::Value;

use crate::permissions::Permission;

pub struct ModuleContext<'a> {
    pub db: &'a DatabaseConnection,
    pub tenant_id: uuid::Uuid,
    pub config: &'a Value,
}

pub trait EventListener: crate::events::EventHandler {}

/// Legacy trait kept for backward compatibility.
/// New modules should implement `RusToKModule::migrations()` directly.
pub trait MigrationSource: Send + Sync {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>>;
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
/// - `media`   — media library; all modules depend on it for file assets
/// - `storage` — file storage backend; media and platform depend on it
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

/// Core trait for all platform modules.
///
/// A module can be anything: a domain feature with its own DB tables (content, commerce),
/// an infrastructure service without DB (storage, telemetry integrations),
/// or a lightweight extension (webhooks, custom validators).
///
/// Not all modules need migrations, permissions, or event listeners —
/// all methods have sensible defaults. Implement only what your module needs.
#[async_trait]
pub trait RusToKModule: Send + Sync {
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

    /// Database migrations this module provides.
    /// Returns empty vec for modules without their own tables (e.g. storage, webhooks).
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }

    /// Returns the list of permissions this module declares.
    /// Used for dynamic RBAC permission registration.
    fn permissions(&self) -> Vec<Permission> {
        Vec::new()
    }

    fn event_listeners(&self) -> Vec<Box<dyn EventListener>> {
        Vec::new()
    }

    async fn on_enable(&self, _ctx: ModuleContext<'_>) -> crate::Result<()> {
        Ok(())
    }

    async fn on_disable(&self, _ctx: ModuleContext<'_>) -> crate::Result<()> {
        Ok(())
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
