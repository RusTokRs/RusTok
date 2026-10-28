/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::module::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleKind,
    ModuleRuntimeExtensions, RusToKModule,
};
use sea_orm_migration::MigrationTrait;

/// CRM Deals & Pipelines capability module for RusToK.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DealModule;

impl DealModule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl RusToKModule for DealModule {
    fn slug(&self) -> &'static str {
        "deal"
    }

    fn name(&self) -> &'static str {
        "CRM Deals & Pipelines"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn description(&self) -> &'static str {
        "CRM Deals & Pipelines capability module for RusToK"
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Optional
    }

    fn dependencies(&self) -> &[&'static str] {
        &["outbox"]
    }

    fn permissions(&self) -> Vec<Permission> {
        // Standard RBAC permissions for deal.
        // Declare registered permissions here once corresponding Resource
        // variants are added to `rustok_api::permissions::Resource`.
        Vec::new()
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        rustok_core::search_projection::register_search_projection_source(
            extensions,
            crate::integrations::search::DealSearchProjectionSourceFactory,
        )?;
        Ok(())
    }

    fn register_event_listeners(
        &self,
        _registry: &mut ModuleEventListenerRegistry,
        _ctx: &ModuleEventListenerContext<'_>,
    ) {
        // Register event listeners for cross-module integration or outbox event handling:
    }
}

impl MigrationSource for DealModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        crate::migrations::migrations()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_metadata() {
        let module = DealModule;
        assert_eq!(module.slug(), "deal");
        assert_eq!(module.name(), "CRM Deals & Pipelines");
        assert_eq!(module.version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(module.dependencies(), &["outbox"]);
        assert_eq!(module.kind(), ModuleKind::Optional);
    }

    #[test]
    fn test_module_permissions_contract() {
        let module = DealModule;
        let _perms = module.permissions();
    }
}
